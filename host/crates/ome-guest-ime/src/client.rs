// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::protocol::{self, FocusInfo, GuestMessage, HostMessage};

const READ_TIMEOUT: Duration = Duration::from_millis(50);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// Connection, bounded line write, and timeout-aware line read seam.
pub trait ImeTransport: Send + Sync {
    /// Connects to one adb-forwarded loopback port.
    fn connect(&self, port: u16) -> io::Result<()>;
    /// Writes one already encoded line.
    fn write_line(&self, line: &[u8]) -> io::Result<()>;
    /// Reads one complete line, or `None` when the timeout expires without a line.
    fn read_line_timeout(&self, timeout: Duration) -> io::Result<Option<Vec<u8>>>;
    /// Closes the current connection. Calling this while disconnected succeeds.
    fn close(&self) -> io::Result<()>;
}

#[derive(Debug)]
struct TcpReader {
    stream: TcpStream,
    pending: Vec<u8>,
}

/// Standard loopback TCP transport used with `adb forward tcp:0 localabstract:ome-ime`.
#[derive(Debug, Default)]
pub struct TcpImeTransport {
    writer: Mutex<Option<TcpStream>>,
    reader: Mutex<Option<TcpReader>>,
}

impl TcpImeTransport {
    fn shutdown(stream: &TcpStream) {
        let _ = stream.shutdown(Shutdown::Both);
    }
}

impl ImeTransport for TcpImeTransport {
    fn connect(&self, port: u16) -> io::Result<()> {
        if port == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "port is zero"));
        }
        self.close()?;
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let stream = TcpStream::connect_timeout(&address.into(), CONNECT_TIMEOUT)?;
        stream.set_nodelay(true)?;
        let reader_stream = stream.try_clone()?;
        *self.writer.lock().map_err(poisoned_lock)? = Some(stream);
        *self.reader.lock().map_err(poisoned_lock)? = Some(TcpReader {
            stream: reader_stream,
            pending: Vec::new(),
        });
        Ok(())
    }

    fn write_line(&self, line: &[u8]) -> io::Result<()> {
        if line.len() > protocol::MAX_LINE_BYTES + 1 || !line.ends_with(b"\n") {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid line"));
        }
        let mut writer = self.writer.lock().map_err(poisoned_lock)?;
        let stream = writer
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "IME is disconnected"))?;
        stream.write_all(line)?;
        stream.flush()
    }

    fn read_line_timeout(&self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let mut reader = self.reader.lock().map_err(poisoned_lock)?;
        let reader = reader
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "IME is disconnected"))?;
        reader.stream.set_read_timeout(Some(timeout))?;
        loop {
            if let Some(newline) = reader.pending.iter().position(|byte| *byte == b'\n') {
                let line = reader.pending.drain(..=newline).collect::<Vec<_>>();
                let without_newline = line.strip_suffix(b"\n").unwrap_or(&line);
                let body_len = without_newline
                    .strip_suffix(b"\r")
                    .unwrap_or(without_newline)
                    .len();
                if body_len > protocol::MAX_LINE_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "IME line is too long",
                    ));
                }
                return Ok(Some(line));
            }
            if reader.pending.len() > protocol::MAX_LINE_BYTES + 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "IME line is too long",
                ));
            }
            let mut chunk = [0_u8; 4096];
            match reader.stream.read(&mut chunk) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "IME socket closed",
                    ));
                }
                Ok(read) => reader.pending.extend_from_slice(&chunk[..read]),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn close(&self) -> io::Result<()> {
        if let Some(stream) = self.writer.lock().map_err(poisoned_lock)?.take() {
            Self::shutdown(&stream);
        }
        if let Some(reader) = self.reader.lock().map_err(poisoned_lock)?.take() {
            Self::shutdown(&reader.stream);
        }
        Ok(())
    }
}

fn poisoned_lock<T>(_: std::sync::PoisonError<T>) -> io::Error {
    io::Error::other("IME transport lock is poisoned")
}

/// Public connection state projected by the runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeLinkState {
    Disconnected,
    Connecting,
    Ready {
        guest_version: u32,
        focus: Option<FocusInfo>,
    },
}

/// Messages observed during one non-blocking link tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeLinkEvent {
    Hello {
        version: u32,
        package: String,
        version_code: u64,
    },
    Focus(FocusInfo),
    Blur,
    Unknown {
        op: String,
    },
    Disconnected,
}

#[derive(Debug)]
enum ReaderEvent {
    Message {
        generation: u64,
        message: GuestMessage,
    },
    Disconnected {
        generation: u64,
    },
}

/// Errors while sending a frame through the current link.
#[derive(Debug, Error)]
pub enum ImeLinkError {
    #[error("IME link is not ready")]
    NotReady,
    #[error("IME frame could not be encoded")]
    Protocol(#[from] protocol::ProtocolError),
    #[error("IME frame could not be written")]
    Io(#[from] io::Error),
}

/// Reconnecting state machine around one adb-forwarded port.
pub struct ImeLink {
    port: u16,
    transport: Arc<dyn ImeTransport>,
    state: ImeLinkState,
    reader_tx: mpsc::Sender<ReaderEvent>,
    reader_rx: mpsc::Receiver<ReaderEvent>,
    reconnect_at: Instant,
    reconnect_delay: Duration,
    hello_deadline: Option<Instant>,
    generation: u64,
}

impl std::fmt::Debug for ImeLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImeLink")
            .field("port", &self.port)
            .field("state", &self.state)
            .field("reconnect_delay", &self.reconnect_delay)
            .finish_non_exhaustive()
    }
}

impl ImeLink {
    /// Creates a disconnected link whose first tick attempts connection immediately.
    pub fn new(port: u16, transport: Arc<dyn ImeTransport>) -> Self {
        let (reader_tx, reader_rx) = mpsc::channel();
        Self {
            port,
            transport,
            state: ImeLinkState::Disconnected,
            reader_tx,
            reader_rx,
            reconnect_at: Instant::now(),
            reconnect_delay: INITIAL_RECONNECT_DELAY,
            hello_deadline: None,
            generation: 0,
        }
    }

    /// Creates a production link using the standard TCP transport.
    pub fn tcp(port: u16) -> Self {
        Self::new(port, Arc::new(TcpImeTransport::default()))
    }

    /// Returns the current connection state.
    pub fn state(&self) -> &ImeLinkState {
        &self.state
    }

    /// Advances reads and reconnects without waiting for an inbound frame.
    pub fn tick(&mut self) -> Vec<ImeLinkEvent> {
        self.tick_at(Instant::now())
    }

    /// Advances the link at an injected monotonic time.
    pub fn tick_at(&mut self, now: Instant) -> Vec<ImeLinkEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.reader_rx.try_recv() {
            match event {
                ReaderEvent::Message {
                    generation,
                    message,
                } if generation == self.generation => match message {
                    GuestMessage::Hello {
                        version,
                        package,
                        version_code,
                    } => {
                        self.state = ImeLinkState::Ready {
                            guest_version: version,
                            focus: None,
                        };
                        self.hello_deadline = None;
                        self.reconnect_delay = INITIAL_RECONNECT_DELAY;
                        events.push(ImeLinkEvent::Hello {
                            version,
                            package,
                            version_code,
                        });
                    }
                    GuestMessage::Focus(focus) => {
                        if let ImeLinkState::Ready { focus: current, .. } = &mut self.state
                            && current.as_ref() != Some(&focus)
                        {
                            *current = Some(focus.clone());
                            events.push(ImeLinkEvent::Focus(focus));
                        }
                    }
                    GuestMessage::Blur => {
                        if let ImeLinkState::Ready { focus, .. } = &mut self.state
                            && focus.take().is_some()
                        {
                            events.push(ImeLinkEvent::Blur);
                        }
                    }
                    GuestMessage::Unknown { op } => events.push(ImeLinkEvent::Unknown { op }),
                },
                ReaderEvent::Disconnected { generation } if generation == self.generation => {
                    self.mark_disconnected(now);
                    events.push(ImeLinkEvent::Disconnected);
                }
                ReaderEvent::Message { .. } | ReaderEvent::Disconnected { .. } => {}
            }
        }
        if matches!(self.state, ImeLinkState::Disconnected) && now >= self.reconnect_at {
            self.state = ImeLinkState::Connecting;
            if self.connect().is_err() {
                self.mark_disconnected(now);
            } else {
                self.hello_deadline = Some(now + CONNECT_TIMEOUT);
            }
        } else if matches!(self.state, ImeLinkState::Connecting)
            && self.hello_deadline.is_some_and(|deadline| now >= deadline)
        {
            self.mark_disconnected(now);
            events.push(ImeLinkEvent::Disconnected);
        }
        events
    }

    /// Sends one text frame. A write failure disconnects the link and schedules a retry.
    pub fn send(&mut self, message: &HostMessage) -> Result<(), ImeLinkError> {
        if !matches!(self.state, ImeLinkState::Ready { .. }) {
            return Err(ImeLinkError::NotReady);
        }
        let line = protocol::encode(message)?;
        if let Err(error) = self.transport.write_line(&line) {
            self.mark_disconnected(Instant::now());
            return Err(ImeLinkError::Io(error));
        }
        Ok(())
    }

    /// Closes the socket and returns to the disconnected state.
    pub fn disconnect(&mut self) {
        let _ = self.transport.close();
        self.generation = self.generation.wrapping_add(1);
        self.state = ImeLinkState::Disconnected;
        self.hello_deadline = None;
        self.reconnect_at = Instant::now();
        self.reconnect_delay = INITIAL_RECONNECT_DELAY;
    }

    fn connect(&mut self) -> io::Result<()> {
        self.transport.connect(self.port)?;
        let hello = protocol::encode(&HostMessage::Hello {
            version: protocol::PROTOCOL_VERSION,
        })
        .map_err(io::Error::other)?;
        self.transport.write_line(&hello)?;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let transport = Arc::clone(&self.transport);
        let sender = self.reader_tx.clone();
        thread::Builder::new()
            .name("ome-ime-reader".to_owned())
            .spawn(move || {
                loop {
                    match transport.read_line_timeout(READ_TIMEOUT) {
                        Ok(Some(line)) => match protocol::decode(&line) {
                            Ok(message) => {
                                if sender
                                    .send(ReaderEvent::Message {
                                        generation,
                                        message,
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(_) => {
                                let _ = sender.send(ReaderEvent::Disconnected { generation });
                                break;
                            }
                        },
                        Ok(None) => continue,
                        Err(_) => {
                            let _ = sender.send(ReaderEvent::Disconnected { generation });
                            break;
                        }
                    }
                }
            })
            .map(|_| ())
            .map_err(io::Error::other)
    }

    fn mark_disconnected(&mut self, now: Instant) {
        let _ = self.transport.close();
        self.generation = self.generation.wrapping_add(1);
        self.state = ImeLinkState::Disconnected;
        self.hello_deadline = None;
        self.reconnect_at = now + self.reconnect_delay;
        self.reconnect_delay = (self.reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
    }
}

impl Drop for ImeLink {
    fn drop(&mut self) {
        let _ = self.transport.close();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[derive(Debug, Default)]
    struct FakeTransport {
        connect_results: Mutex<VecDeque<io::Result<()>>>,
        reads: Mutex<VecDeque<io::Result<Option<Vec<u8>>>>>,
        writes: Mutex<Vec<Vec<u8>>>,
        write_failures: AtomicUsize,
        connections: AtomicUsize,
    }

    impl FakeTransport {
        fn with_connects(results: impl IntoIterator<Item = io::Result<()>>) -> Arc<Self> {
            Arc::new(Self {
                connect_results: Mutex::new(results.into_iter().collect()),
                ..Self::default()
            })
        }

        fn push_line(&self, line: &str) {
            self.reads
                .lock()
                .expect("reads")
                .push_back(Ok(Some(format!("{line}\n").into_bytes())));
        }
    }

    impl ImeTransport for FakeTransport {
        fn connect(&self, _port: u16) -> io::Result<()> {
            self.connections.fetch_add(1, Ordering::SeqCst);
            self.connect_results
                .lock()
                .expect("connects")
                .pop_front()
                .unwrap_or(Ok(()))
        }

        fn write_line(&self, line: &[u8]) -> io::Result<()> {
            if self.write_failures.load(Ordering::SeqCst) > 0 {
                self.write_failures.fetch_sub(1, Ordering::SeqCst);
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed"));
            }
            self.writes.lock().expect("writes").push(line.to_vec());
            Ok(())
        }

        fn read_line_timeout(&self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            self.reads
                .lock()
                .expect("reads")
                .pop_front()
                .unwrap_or_else(|| {
                    thread::sleep(Duration::from_millis(1));
                    Ok(None)
                })
        }

        fn close(&self) -> io::Result<()> {
            Ok(())
        }
    }

    fn wait_for(link: &mut ImeLink, predicate: impl Fn(&ImeLinkState) -> bool) {
        for _ in 0..200 {
            link.tick();
            if predicate(link.state()) {
                return;
            }
            thread::sleep(Duration::from_millis(1));
        }
        panic!("link did not reach expected state: {:?}", link.state());
    }

    #[test]
    fn fake_transport_reaches_ready_and_tracks_focus() {
        let transport = FakeTransport::with_connects([Ok(())]);
        transport.push_line(
            r#"{"op":"hello","version":1,"package":"org.openmobileemulator.ime","versionCode":3}"#,
        );
        transport
            .push_line(r#"{"op":"focus","inputType":1,"imeAction":6,"package":"com.example.app"}"#);
        let mut link = ImeLink::new(12345, transport.clone());
        link.tick();
        assert_eq!(link.state(), &ImeLinkState::Connecting);
        wait_for(&mut link, |state| {
            matches!(state, ImeLinkState::Ready { focus: Some(_), .. })
        });
        assert!(matches!(
            link.state(),
            ImeLinkState::Ready {
                guest_version: 1,
                focus: Some(FocusInfo { package, .. }),
            } if package == "com.example.app"
        ));
        assert_eq!(transport.connections.load(Ordering::SeqCst), 1);
        assert_eq!(
            transport.writes.lock().expect("writes")[0],
            b"{\"op\":\"hello\",\"version\":1}\n"
        );
    }

    #[test]
    fn duplicate_focus_and_idle_blur_do_not_repeat_state_events() {
        let transport = FakeTransport::with_connects([Ok(())]);
        transport.push_line(
            r#"{"op":"hello","version":1,"package":"org.openmobileemulator.ime","versionCode":1}"#,
        );
        let focus = r#"{"op":"focus","inputType":1,"imeAction":6,"package":"com.example.app"}"#;
        transport.push_line(focus);
        transport.push_line(focus);
        transport.push_line(r#"{"op":"blur"}"#);
        transport.push_line(r#"{"op":"blur"}"#);
        let mut link = ImeLink::new(12345, transport);
        link.tick();
        let mut focus_events = 0;
        let mut blur_events = 0;
        for _ in 0..200 {
            for event in link.tick() {
                match event {
                    ImeLinkEvent::Focus(_) => focus_events += 1,
                    ImeLinkEvent::Blur => blur_events += 1,
                    ImeLinkEvent::Hello { .. }
                    | ImeLinkEvent::Unknown { .. }
                    | ImeLinkEvent::Disconnected => {}
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(focus_events, 1);
        assert_eq!(blur_events, 1);
    }

    #[test]
    fn write_failure_disconnects_and_reconnects() {
        let transport = FakeTransport::with_connects([Ok(()), Ok(())]);
        transport.push_line(
            r#"{"op":"hello","version":1,"package":"org.openmobileemulator.ime","versionCode":1}"#,
        );
        let mut link = ImeLink::new(12345, transport.clone());
        link.tick();
        wait_for(&mut link, |state| {
            matches!(state, ImeLinkState::Ready { .. })
        });
        transport.write_failures.store(1, Ordering::SeqCst);
        assert!(
            link.send(&HostMessage::Commit {
                text: "한글".to_owned()
            })
            .is_err()
        );
        assert_eq!(link.state(), &ImeLinkState::Disconnected);
        link.tick_at(Instant::now() + MAX_RECONNECT_DELAY);
        assert_eq!(transport.connections.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn failed_connection_retries_with_bounded_backoff() {
        let transport = FakeTransport::with_connects([
            Err(io::Error::new(io::ErrorKind::ConnectionRefused, "first")),
            Err(io::Error::new(io::ErrorKind::ConnectionRefused, "second")),
            Ok(()),
        ]);
        transport.push_line(
            r#"{"op":"hello","version":1,"package":"org.openmobileemulator.ime","versionCode":1}"#,
        );
        let mut link = ImeLink::new(12345, transport.clone());
        let start = Instant::now();
        link.tick_at(start);
        assert_eq!(transport.connections.load(Ordering::SeqCst), 1);
        link.tick_at(start + Duration::from_millis(999));
        assert_eq!(transport.connections.load(Ordering::SeqCst), 1);
        link.tick_at(start + Duration::from_secs(1));
        assert_eq!(transport.connections.load(Ordering::SeqCst), 2);
        link.tick_at(start + Duration::from_secs(2));
        assert_eq!(transport.connections.load(Ordering::SeqCst), 2);
        link.tick_at(start + Duration::from_secs(3));
        assert_eq!(transport.connections.load(Ordering::SeqCst), 3);
    }
}
