// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Synchronous QMP client with request correlation and lossless event queuing.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

const POLL_TIMEOUT: Duration = Duration::from_millis(1);

/// A QMP transport or protocol failure.
#[derive(Debug, Error)]
pub enum QmpError {
    /// Opening or configuring the TCP stream failed.
    #[error("QMP I/O failed: {0}")]
    Io(#[from] io::Error),
    /// The peer did not send a complete message within the configured timeout.
    #[error("QMP operation timed out")]
    Timeout,
    /// The peer closed the stream before the expected response arrived.
    #[error("QMP peer closed the stream")]
    Closed,
    /// A line was not valid JSON.
    #[error("QMP JSON was invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// The peer violated greeting, response, or request-ID rules.
    #[error("QMP protocol error: {0}")]
    Protocol(String),
    /// QEMU rejected the command correlated with the caller's request ID.
    #[error("QMP command failed ({class}): {desc}")]
    Command {
        /// QEMU error class.
        class: String,
        /// QEMU error description.
        desc: String,
    },
}

/// A QMP asynchronous event retained while commands are in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct QmpEvent {
    /// QEMU event name, such as `SHUTDOWN`.
    pub event: String,
    /// Event-specific object, or JSON null when QEMU omitted it.
    pub data: Value,
    /// QEMU timestamp object, or JSON null when QEMU omitted it.
    pub timestamp: Value,
}

/// Result returned by QEMU's `query-status` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunState {
    /// QEMU status token, such as `running`, `paused`, or `shutdown`.
    pub status: String,
    /// Whether QEMU reports active vCPU execution.
    pub running: bool,
    /// Whether single-step execution is enabled.
    pub singlestep: bool,
}

/// A negotiated QMP connection over one TCP stream.
///
/// The channel owns request IDs and queues every event observed while waiting
/// for a command response. Callers must serialize access through this value.
#[derive(Debug)]
pub struct QmpChannel {
    reader: BufReader<TcpStream>,
    timeout: Duration,
    next_id: u64,
    events: VecDeque<QmpEvent>,
    pending_line: Vec<u8>,
}

#[derive(Serialize)]
struct Request<'a> {
    execute: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<&'a Value>,
    id: &'a str,
}

impl QmpChannel {
    /// Connects within `timeout`, validates the QMP greeting, and negotiates
    /// `qmp_capabilities` before returning.
    pub fn connect(addr: SocketAddr, timeout: Duration) -> Result<Self, QmpError> {
        if !addr.ip().is_loopback() {
            return Err(QmpError::Protocol(
                "QMP address must be on the loopback interface".to_owned(),
            ));
        }
        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(map_io)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let mut channel = Self {
            reader: BufReader::new(stream),
            timeout,
            next_id: 1,
            events: VecDeque::new(),
            pending_line: Vec::new(),
        };
        let greeting = channel.read_message()?;
        if !greeting
            .as_object()
            .is_some_and(|object| object.contains_key("QMP"))
        {
            return Err(QmpError::Protocol(
                "initial message does not contain a QMP greeting".to_owned(),
            ));
        }
        channel.execute_with_id("qmp_capabilities", None, "ome-capabilities")?;
        Ok(channel)
    }

    /// Executes one QMP command and returns its `return` value.
    ///
    /// Requests are compact UTF-8 JSON terminated by CRLF. Events that arrive
    /// before the matching response remain available through [`Self::poll_event`].
    pub fn execute(&mut self, command: &str, arguments: Option<Value>) -> Result<Value, QmpError> {
        let id = format!("ome-{}", self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.execute_with_id(command, arguments.as_ref(), &id)
    }

    /// Returns the oldest queued event, or briefly checks the stream for one.
    ///
    /// Timeout, closure, malformed data, and non-event messages produce `None`;
    /// command methods remain the error-reporting interface for the channel.
    pub fn poll_event(&mut self) -> Option<QmpEvent> {
        if let Some(event) = self.events.pop_front() {
            return Some(event);
        }
        if self
            .reader
            .get_ref()
            .set_read_timeout(Some(POLL_TIMEOUT))
            .is_err()
        {
            return None;
        }
        let message = self.read_message().ok();
        let _ = self.reader.get_ref().set_read_timeout(Some(self.timeout));
        message.and_then(|value| parse_event(&value))
    }

    /// Runs `query-status` and validates its three standard fields.
    pub fn query_status(&mut self) -> Result<RunState, QmpError> {
        let value = self.execute("query-status", None)?;
        let object = value
            .as_object()
            .ok_or_else(|| QmpError::Protocol("query-status return is not an object".to_owned()))?;
        let status = object
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| QmpError::Protocol("query-status omitted string status".to_owned()))?
            .to_owned();
        let running = object
            .get("running")
            .and_then(Value::as_bool)
            .ok_or_else(|| QmpError::Protocol("query-status omitted boolean running".to_owned()))?;
        let singlestep = object
            .get("singlestep")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                QmpError::Protocol("query-status omitted boolean singlestep".to_owned())
            })?;
        Ok(RunState {
            status,
            running,
            singlestep,
        })
    }

    /// Requests an ACPI guest powerdown.
    pub fn system_powerdown(&mut self) -> Result<(), QmpError> {
        self.execute_empty("system_powerdown", None)
    }

    /// Requests immediate QEMU process termination through QMP.
    pub fn quit(&mut self) -> Result<(), QmpError> {
        self.execute_empty("quit", None)
    }

    /// Writes a PPM screenshot to `path` on the QEMU host.
    pub fn screendump(&mut self, path: &Path) -> Result<(), QmpError> {
        self.execute_empty(
            "screendump",
            Some(json!({ "filename": path.to_string_lossy() })),
        )
    }

    /// Sends a complete QMP `input-send-event` event array.
    pub fn input_send_event(&mut self, events: &[Value]) -> Result<(), QmpError> {
        self.execute_empty("input-send-event", Some(json!({ "events": events })))
    }

    /// Sends QEMU key names through the `send-key` command.
    pub fn send_key(&mut self, keys: &[&str]) -> Result<(), QmpError> {
        let keys: Vec<Value> = keys
            .iter()
            .map(|key| json!({ "type": "qcode", "data": key }))
            .collect();
        self.execute_empty("send-key", Some(json!({ "keys": keys })))
    }

    /// Returns QEMU's unmodified `query-display-options` result.
    pub fn query_display_options(&mut self) -> Result<Value, QmpError> {
        self.execute("query-display-options", None)
    }

    fn execute_empty(&mut self, command: &str, arguments: Option<Value>) -> Result<(), QmpError> {
        self.execute(command, arguments).map(|_| ())
    }

    fn execute_with_id(
        &mut self,
        command: &str,
        arguments: Option<&Value>,
        id: &str,
    ) -> Result<Value, QmpError> {
        let request = Request {
            execute: command,
            arguments,
            id,
        };
        let mut bytes = serde_json::to_vec(&request)?;
        bytes.extend_from_slice(b"\r\n");
        self.reader.get_mut().write_all(&bytes).map_err(map_io)?;
        self.reader.get_mut().flush().map_err(map_io)?;

        loop {
            let message = self.read_message()?;
            if let Some(event) = parse_event(&message) {
                self.events.push_back(event);
                continue;
            }
            let object = message.as_object().ok_or_else(|| {
                QmpError::Protocol("response message is not a JSON object".to_owned())
            })?;
            let response_id = object
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| QmpError::Protocol("response omitted string id".to_owned()))?;
            if response_id != id {
                return Err(QmpError::Protocol(format!(
                    "response id '{response_id}' does not match request id '{id}'"
                )));
            }
            if let Some(value) = object.get("return") {
                return Ok(value.clone());
            }
            if let Some(error) = object.get("error").and_then(Value::as_object) {
                let class = error
                    .get("class")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown")
                    .to_owned();
                let desc = error
                    .get("desc")
                    .and_then(Value::as_str)
                    .unwrap_or("QEMU returned an error without a description")
                    .to_owned();
                return Err(QmpError::Command { class, desc });
            }
            return Err(QmpError::Protocol(
                "response has neither return nor error".to_owned(),
            ));
        }
    }

    fn read_message(&mut self) -> Result<Value, QmpError> {
        loop {
            let available = self.reader.fill_buf().map_err(map_io)?;
            if available.is_empty() {
                return Err(QmpError::Closed);
            }
            if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
                self.pending_line.extend_from_slice(&available[..=newline]);
                self.reader.consume(newline + 1);
                break;
            }
            let length = available.len();
            self.pending_line.extend_from_slice(available);
            self.reader.consume(length);
        }
        while self
            .pending_line
            .last()
            .is_some_and(|byte| matches!(byte, b'\r' | b'\n'))
        {
            self.pending_line.pop();
        }
        if self.pending_line.is_empty() {
            return Err(QmpError::Protocol("QMP sent an empty line".to_owned()));
        }
        let result = serde_json::from_slice(&self.pending_line).map_err(QmpError::Json);
        self.pending_line.clear();
        result
    }
}

fn parse_event(value: &Value) -> Option<QmpEvent> {
    let object = value.as_object()?;
    let event = object.get("event")?.as_str()?.to_owned();
    Some(QmpEvent {
        event,
        data: object.get("data").cloned().unwrap_or(Value::Null),
        timestamp: object.get("timestamp").cloned().unwrap_or(Value::Null),
    })
}

fn map_io(error: io::Error) -> QmpError {
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => QmpError::Timeout,
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::UnexpectedEof => QmpError::Closed,
        _ => QmpError::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

    use super::*;

    fn server<F>(script: F) -> (SocketAddr, thread::JoinHandle<()>)
    where
        F: FnOnce(TcpStream) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake QMP");
        let address = listener.local_addr().expect("fake QMP address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept fake QMP client");
            script(stream);
        });
        (address, handle)
    }

    fn read_request(reader: &mut BufReader<TcpStream>) -> Value {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request");
        serde_json::from_str(line.trim()).expect("request JSON")
    }

    fn handshake(mut stream: TcpStream) -> BufReader<TcpStream> {
        stream
            .write_all(b"{\"QMP\":{\"version\":{},\"capabilities\":[]}}\r\n")
            .expect("write greeting");
        let read_stream = stream.try_clone().expect("clone stream");
        let mut reader = BufReader::new(read_stream);
        let request = read_request(&mut reader);
        assert_eq!(request["execute"], "qmp_capabilities");
        assert_eq!(request["id"], "ome-capabilities");
        stream
            .write_all(b"{\"return\":{},\"id\":\"ome-capabilities\"}\r\n")
            .expect("write capabilities response");
        reader
    }

    #[test]
    fn handshake_command_order_and_interleaved_event() {
        let (address, server) = server(|stream| {
            let mut write = stream.try_clone().expect("clone writer");
            let mut reader = handshake(stream);
            let request = read_request(&mut reader);
            assert_eq!(request["execute"], "query-status");
            assert_eq!(request["id"], "ome-1");
            write
                .write_all(
                    b"{\"event\":\"SHUTDOWN\",\"data\":{\"guest\":true},\"timestamp\":{\"seconds\":1}}\r\n\
                      {\"return\":{\"status\":\"running\",\"running\":true,\"singlestep\":false},\"id\":\"ome-1\"}\r\n",
                )
                .expect("write event and response");
            keep_response_alive();
        });
        let mut channel = QmpChannel::connect(address, Duration::from_secs(1)).expect("connect");
        assert_eq!(
            channel.query_status().expect("query status"),
            RunState {
                status: "running".into(),
                running: true,
                singlestep: false,
            }
        );
        let event = channel.poll_event().expect("queued event");
        assert_eq!(event.event, "SHUTDOWN");
        assert_eq!(event.data["guest"], true);
        server.join().expect("server thread");
    }

    fn keep_response_alive() {
        thread::sleep(Duration::from_millis(20));
    }

    #[test]
    fn partial_event_line_survives_poll_timeout() {
        let (address, server) = server(|stream| {
            let mut write = stream.try_clone().expect("clone writer");
            let _reader = handshake(stream);
            write
                .write_all(b"{\"event\":\"RESET\"")
                .expect("write partial event");
            thread::sleep(Duration::from_millis(25));
            write.write_all(b",\"data\":{}}\r\n").expect("finish event");
            keep_response_alive();
        });
        let mut channel = QmpChannel::connect(address, Duration::from_secs(1)).expect("connect");
        assert_eq!(channel.poll_event(), None);
        // The server finishes the line after 25 ms; under load the thread may be late, so poll
        // until a deadline instead of sleeping a fixed 30 ms (flaked once on 2026-09-27).
        let deadline = Instant::now() + Duration::from_secs(2);
        let event = loop {
            if let Some(event) = channel.poll_event() {
                break event;
            }
            assert!(Instant::now() < deadline, "assembled event");
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(event.event, "RESET");
        server.join().expect("server thread");
    }

    #[test]
    fn command_error_is_typed() {
        let (address, server) = server(|stream| {
            let mut write = stream.try_clone().expect("clone writer");
            let mut reader = handshake(stream);
            let request = read_request(&mut reader);
            let id = request["id"].as_str().expect("request id");
            write
                .write_all(
                    format!(
                        "{{\"error\":{{\"class\":\"CommandNotFound\",\"desc\":\"no such command\"}},\"id\":\"{id}\"}}\r\n"
                    )
                    .as_bytes(),
                )
                .expect("write error");
            keep_response_alive();
        });
        let mut channel = QmpChannel::connect(address, Duration::from_secs(1)).expect("connect");
        let error = channel.quit().expect_err("command must fail");
        assert!(matches!(
            error,
            QmpError::Command { class, desc }
                if class == "CommandNotFound" && desc == "no such command"
        ));
        server.join().expect("server thread");
    }

    #[test]
    fn mismatched_id_is_a_protocol_error() {
        let (address, server) = server(|stream| {
            let mut write = stream.try_clone().expect("clone writer");
            let mut reader = handshake(stream);
            let _ = read_request(&mut reader);
            write
                .write_all(b"{\"return\":{},\"id\":\"someone-else\"}\r\n")
                .expect("write mismatch");
            keep_response_alive();
        });
        let mut channel = QmpChannel::connect(address, Duration::from_secs(1)).expect("connect");
        assert!(matches!(channel.quit(), Err(QmpError::Protocol(_))));
        server.join().expect("server thread");
    }

    #[test]
    fn read_timeout_is_typed() {
        let (address, server) = server(|stream| {
            let mut reader = handshake(stream);
            let _ = read_request(&mut reader);
            thread::sleep(Duration::from_millis(150));
        });
        let mut channel =
            QmpChannel::connect(address, Duration::from_millis(100)).expect("connect");
        channel.timeout = Duration::from_millis(30);
        channel
            .reader
            .get_ref()
            .set_read_timeout(Some(channel.timeout))
            .expect("set command timeout");
        assert!(matches!(channel.quit(), Err(QmpError::Timeout)));
        server.join().expect("server thread");
    }

    #[test]
    fn closed_stream_is_typed() {
        let (address, server) = server(|stream| {
            let mut reader = handshake(stream);
            let _ = read_request(&mut reader);
        });
        let mut channel = QmpChannel::connect(address, Duration::from_secs(1)).expect("connect");
        assert!(matches!(channel.quit(), Err(QmpError::Closed)));
        server.join().expect("server thread");
    }

    #[test]
    fn outgoing_json_uses_required_key_order_and_crlf() {
        let (address, server) = server(|stream| {
            let mut write = stream.try_clone().expect("clone writer");
            let mut reader = handshake(stream);
            let mut line = String::new();
            reader.read_line(&mut line).expect("read request line");
            assert_eq!(
                line,
                "{\"execute\":\"screendump\",\"arguments\":{\"filename\":\"shot.ppm\"},\"id\":\"ome-1\"}\r\n"
            );
            write
                .write_all(b"{\"return\":{},\"id\":\"ome-1\"}\r\n")
                .expect("write response");
            keep_response_alive();
        });
        let mut channel = QmpChannel::connect(address, Duration::from_secs(1)).expect("connect");
        channel
            .screendump(Path::new("shot.ppm"))
            .expect("screendump");
        server.join().expect("server thread");
    }
}
