// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! QEMU process supervision, pure lifecycle transitions, and restart policy.
#![forbid(unsafe_code)]

use std::fs;
#[cfg(windows)]
use std::fs::File;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ome_guest_config::{GuestConfig, GuestPaths, QemuInstall, QemuInvocation};
use ome_qmp::{QmpChannel, QmpError, QmpEvent};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// QMP-ready deadline inherited from `Start-Guest.ps1`.
pub const QMP_READY_DEADLINE: Duration = Duration::from_secs(10);
/// QMP polling interval inherited from `Start-Guest.ps1`.
pub const QMP_POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Grace period after adb/QMP power-off before process termination.
pub const STOP_DEADLINE: Duration = Duration::from_secs(30);

/// External guest states defined by `CONTEXT.md`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GuestState {
    /// No QEMU process is owned.
    Stopped,
    /// QEMU was spawned but QMP has not responded yet.
    Starting,
    /// QMP responded; Android may still be booting.
    Running,
    /// An explicit stop request is in progress.
    Stopping,
    /// A guest-reset shutdown was observed and a replacement is being started.
    Restarting,
    /// Startup or process lifetime ended unexpectedly.
    Failed,
}

/// Internal lifecycle state with timestamps needed for pure timeout decisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    /// No child exists.
    Stopped,
    /// Process creation was requested.
    Starting,
    /// QMP is connected.
    Running,
    /// Graceful stop is in progress. `since` is present only for explicit stops
    /// that escalate to forced termination after the policy deadline.
    Stopping { since: Option<Instant> },
    /// A guest reset requires a replacement process.
    Restarting,
    /// An unrecoverable failure occurred.
    Failed,
}

impl Lifecycle {
    /// Returns the public [`GuestState`] represented by this lifecycle value.
    pub fn state(self) -> GuestState {
        match self {
            Self::Stopped => GuestState::Stopped,
            Self::Starting => GuestState::Starting,
            Self::Running => GuestState::Running,
            Self::Stopping { .. } => GuestState::Stopping,
            Self::Restarting => GuestState::Restarting,
            Self::Failed => GuestState::Failed,
        }
    }

    /// Applies one event and returns the next immutable state plus side effects.
    ///
    /// Invalid or stale events are ignored. Restart occurs only when QMP first
    /// identified a guest-reset shutdown and the child then exits.
    pub fn on(&self, event: LifecycleEvent) -> (Lifecycle, Vec<Action>) {
        match (*self, event) {
            (Self::Starting, LifecycleEvent::Spawned) => (Self::Starting, vec![Action::ConnectQmp]),
            (Self::Starting, LifecycleEvent::QmpReady) => {
                (Self::Running, vec![Action::EmitState(GuestState::Running)])
            }
            (Self::Starting, LifecycleEvent::QmpUnreachable(elapsed))
                if elapsed >= QMP_READY_DEADLINE =>
            {
                (
                    Self::Failed,
                    vec![
                        Action::KillProcess,
                        Action::KeepLogsAsFailure,
                        Action::EmitState(GuestState::Failed),
                    ],
                )
            }
            (Self::Running, LifecycleEvent::GuestResetShutdown) => (
                Self::Restarting,
                vec![Action::EmitState(GuestState::Restarting)],
            ),
            (Self::Running, LifecycleEvent::GuestPowerdownShutdown) => (
                Self::Stopping { since: None },
                vec![Action::EmitState(GuestState::Stopping)],
            ),
            (Self::Restarting, LifecycleEvent::ProcessExited(_)) => (
                Self::Starting,
                vec![Action::Spawn, Action::EmitState(GuestState::Starting)],
            ),
            (Self::Stopping { .. }, LifecycleEvent::ProcessExited(_)) => {
                (Self::Stopped, vec![Action::EmitState(GuestState::Stopped)])
            }
            (Self::Starting, LifecycleEvent::ProcessExited(code))
            | (Self::Running, LifecycleEvent::ProcessExited(code)) => {
                let _ = code;
                (
                    Self::Failed,
                    vec![
                        Action::KeepLogsAsFailure,
                        Action::EmitState(GuestState::Failed),
                    ],
                )
            }
            (Self::Running | Self::Starting | Self::Restarting, LifecycleEvent::StopRequested) => (
                Self::Stopping { since: None },
                vec![
                    Action::SendPowerdown,
                    Action::EmitState(GuestState::Stopping),
                ],
            ),
            (Self::Stopping { since: None }, LifecycleEvent::Tick(now)) => {
                (Self::Stopping { since: Some(now) }, Vec::new())
            }
            (Self::Stopping { since: Some(since) }, LifecycleEvent::Tick(now))
                if now.saturating_duration_since(since) >= STOP_DEADLINE =>
            {
                (
                    Self::Stopping { since: Some(since) },
                    vec![Action::KillProcess],
                )
            }
            (state, _) => (state, Vec::new()),
        }
    }
}

/// An input to the pure [`Lifecycle`] state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleEvent {
    /// A child process was created.
    Spawned,
    /// QMP handshake and capability negotiation succeeded.
    QmpReady,
    /// QMP remained unreachable for this duration.
    QmpUnreachable(Duration),
    /// QMP emitted `SHUTDOWN` with `guest=true` and reason `guest-reset`.
    GuestResetShutdown,
    /// QMP emitted a non-reset guest shutdown.
    GuestPowerdownShutdown,
    /// The owned QEMU process exited with the supplied code.
    ProcessExited(i32),
    /// A caller requested graceful stop.
    StopRequested,
    /// Periodic monotonic time update.
    Tick(Instant),
}

/// Side effects emitted by the pure lifecycle state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Spawn the retained QEMU invocation.
    Spawn,
    /// Attempt QMP connection and capability negotiation.
    ConnectQmp,
    /// Run the power-off hook, then use QMP if the hook could not request shutdown.
    SendPowerdown,
    /// Terminate the owned QEMU process.
    KillProcess,
    /// Notify subscribers of a public state change.
    EmitState(GuestState),
    /// Mark the current log set as evidence for a failed run.
    KeepLogsAsFailure,
}

/// Paths for one QEMU process's redirected output and printable command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogPaths {
    /// Redirected standard output.
    pub stdout: PathBuf,
    /// Redirected standard error.
    pub stderr: PathBuf,
    /// Printable command line ending in `.cmd.log`.
    pub command: PathBuf,
}

/// Process creation failure surfaced by a [`ProcessAdapter`].
#[derive(Debug, Error)]
pub enum SpawnError {
    /// File or platform process creation failed.
    #[error("QEMU spawn failed: {0}")]
    Io(#[from] io::Error),
    /// A platform adapter exists but is not wired for this host.
    #[error("QEMU process adapter is not wired")]
    Unwired,
}

/// Child wait or termination failure.
#[derive(Debug, Error)]
pub enum WaitError {
    /// Waiting for or terminating the process failed.
    #[error("QEMU child operation failed: {0}")]
    Io(#[from] io::Error),
}

/// Adapter that starts QEMU while honoring the provided redirection paths.
pub trait ProcessAdapter: Send + 'static {
    /// Starts `invocation`, redirecting output into `logs`, and returns the owned child.
    fn spawn(
        &mut self,
        invocation: &QemuInvocation,
        logs: &LogPaths,
    ) -> Result<Box<dyn ChildProcess>, SpawnError>;
}

/// Owned child operations needed by the supervisor loop.
pub trait ChildProcess: Send {
    /// Immutable process identifier.
    fn pid(&self) -> u32;
    /// Non-blocking exit check, returning a signed representation of the exit code.
    fn try_wait(&mut self) -> Result<Option<i32>, WaitError>;
    /// Forcefully terminates the child.
    fn kill(&mut self) -> Result<(), WaitError>;
}

/// A negotiated QMP session used only by the supervisor worker.
pub trait QmpSession: Send {
    /// Sends QMP `system_powerdown`.
    fn system_powerdown(&mut self) -> Result<(), QmpError>;
    /// Returns the next queued QMP event, if one is immediately available.
    fn poll_event(&mut self) -> Option<QmpEvent>;
}

impl QmpSession for QmpChannel {
    fn system_powerdown(&mut self) -> Result<(), QmpError> {
        QmpChannel::system_powerdown(self)
    }

    fn poll_event(&mut self) -> Option<QmpEvent> {
        QmpChannel::poll_event(self)
    }
}

/// Factory for the single QMP session owned by a supervisor.
pub trait QmpFactory: Send + Sync + 'static {
    /// Connects and negotiates QMP within `timeout`.
    fn connect(
        &self,
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<Box<dyn QmpSession>, QmpError>;
}

/// Production QMP factory backed by [`QmpChannel`].
#[derive(Clone, Copy, Debug, Default)]
pub struct TcpQmpFactory;

impl QmpFactory for TcpQmpFactory {
    fn connect(
        &self,
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<Box<dyn QmpSession>, QmpError> {
        QmpChannel::connect(address, timeout)
            .map(|channel| Box::new(channel) as Box<dyn QmpSession>)
    }
}

/// Graceful guest power-off hook, normally backed by `adb reboot -p`.
pub trait PowerOffHook: Send + Sync + 'static {
    /// Returns `true` only when the hook successfully submitted a power-off request.
    fn request_power_off(&self) -> bool;
}

impl<F> PowerOffHook for F
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    fn request_power_off(&self) -> bool {
        self()
    }
}

/// Timing and graceful-stop dependencies for [`Supervisor`].
#[derive(Clone)]
pub struct SupervisorPolicy {
    /// Maximum wait for initial QMP readiness.
    pub qmp_ready_deadline: Duration,
    /// Interval between process and QMP checks.
    pub poll_interval: Duration,
    /// Wait after power-off before forced termination.
    pub stop_deadline: Duration,
    /// First graceful stop attempt; QMP follows only when this returns false.
    pub power_off_hook: Arc<dyn PowerOffHook>,
}

impl std::fmt::Debug for SupervisorPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SupervisorPolicy")
            .field("qmp_ready_deadline", &self.qmp_ready_deadline)
            .field("poll_interval", &self.poll_interval)
            .field("stop_deadline", &self.stop_deadline)
            .finish_non_exhaustive()
    }
}

impl Default for SupervisorPolicy {
    fn default() -> Self {
        Self {
            qmp_ready_deadline: QMP_READY_DEADLINE,
            poll_interval: QMP_POLL_INTERVAL,
            stop_deadline: STOP_DEADLINE,
            power_off_hook: Arc::new(|| false),
        }
    }
}

/// Event emitted to every active supervisor subscriber.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestEvent {
    /// New public guest state.
    pub state: GuestState,
    /// Process identifier when a child is currently known.
    pub pid: Option<u32>,
    /// Log paths for the current or most recent process.
    pub logs: Option<LogPaths>,
}

/// Failure returned synchronously by supervisor admission and setup.
#[derive(Debug, Error)]
pub enum SupervisorError {
    /// Start was requested from a state that already owns or is stopping a guest.
    #[error("guest cannot start while supervisor state is {0:?}")]
    InvalidState(GuestState),
    /// A worker is already active despite a state that should admit a start.
    #[error("supervisor worker is already active")]
    WorkerActive,
    /// The log directory or command log could not be created.
    #[error("supervisor log setup failed: {0}")]
    Io(#[from] io::Error),
    /// The supervisor worker could not be created.
    #[error("supervisor thread could not be created: {0}")]
    Thread(io::Error),
}

/// Owner of one QEMU invocation and its worker-thread lifecycle.
pub struct Supervisor<A, Q>
where
    A: ProcessAdapter,
    Q: QmpFactory,
{
    adapter: Arc<Mutex<Option<A>>>,
    qmp_factory: Arc<Q>,
    log_dir: PathBuf,
    policy: SupervisorPolicy,
    shared: Arc<Shared>,
    worker_active: Arc<AtomicBool>,
}

impl<A, Q> std::fmt::Debug for Supervisor<A, Q>
where
    A: ProcessAdapter,
    Q: QmpFactory,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Supervisor")
            .field("log_dir", &self.log_dir)
            .field("policy", &self.policy)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

struct Shared {
    state: Mutex<GuestState>,
    subscribers: Mutex<Vec<mpsc::Sender<GuestEvent>>>,
    stop_requested: AtomicBool,
}

impl<A, Q> Supervisor<A, Q>
where
    A: ProcessAdapter,
    Q: QmpFactory,
{
    /// Creates a stopped supervisor; no process or thread starts until [`Self::start`].
    pub fn new(adapter: A, qmp_factory: Q, log_dir: PathBuf, policy: SupervisorPolicy) -> Self {
        Self {
            adapter: Arc::new(Mutex::new(Some(adapter))),
            qmp_factory: Arc::new(qmp_factory),
            log_dir,
            policy,
            shared: Arc::new(Shared {
                state: Mutex::new(GuestState::Stopped),
                subscribers: Mutex::new(Vec::new()),
                stop_requested: AtomicBool::new(false),
            }),
            worker_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Starts a background worker for a validated config and persistent boot invocation.
    ///
    /// Calls are accepted only from `Stopped` or `Failed`. Runtime spawn and
    /// QMP failures are reported through state events because they happen after
    /// this method returns.
    pub fn start(
        &mut self,
        config: GuestConfig,
        paths: GuestPaths,
        install: QemuInstall,
    ) -> Result<(), SupervisorError> {
        let current = self.state();
        if !matches!(current, GuestState::Stopped | GuestState::Failed) {
            return Err(SupervisorError::InvalidState(current));
        }
        if self.worker_active.swap(true, Ordering::AcqRel) {
            return Err(SupervisorError::WorkerActive);
        }
        if let Err(error) = fs::create_dir_all(&self.log_dir) {
            self.worker_active.store(false, Ordering::Release);
            return Err(SupervisorError::Io(error));
        }
        let invocation = QemuInvocation::for_boot(&config, &paths, &install);
        self.shared.stop_requested.store(false, Ordering::Release);
        emit(&self.shared, GuestState::Starting, None, None);

        let context = WorkerContext {
            adapter: Arc::clone(&self.adapter),
            qmp_factory: Arc::clone(&self.qmp_factory),
            shared: Arc::clone(&self.shared),
            worker_active: Arc::clone(&self.worker_active),
            log_dir: self.log_dir.clone(),
            policy: self.policy.clone(),
        };
        thread::Builder::new()
            .name(format!("ome-supervisor-{}", config.name()))
            .spawn(move || run_worker(context, config, invocation))
            .map_err(|error| {
                self.worker_active.store(false, Ordering::Release);
                emit(&self.shared, GuestState::Failed, None, None);
                SupervisorError::Thread(error)
            })?;
        Ok(())
    }

    /// Requests the launcher stop order: power-off hook, QMP fallback, 30-second
    /// wait by default, then forced process termination.
    pub fn request_stop(&self) -> Result<(), SupervisorError> {
        let state = self.state();
        if state == GuestState::Stopped {
            return Ok(());
        }
        self.shared.stop_requested.store(true, Ordering::Release);
        Ok(())
    }

    /// Returns the latest public state.
    pub fn state(&self) -> GuestState {
        *lock_unpoisoned(&self.shared.state)
    }

    /// Registers a subscriber and immediately sends the current state.
    ///
    /// Dropped receivers are removed on the next emitted event.
    pub fn subscribe(&self) -> mpsc::Receiver<GuestEvent> {
        let (sender, receiver) = mpsc::channel();
        let state = self.state();
        let _ = sender.send(GuestEvent {
            state,
            pid: None,
            logs: None,
        });
        lock_unpoisoned(&self.shared.subscribers).push(sender);
        receiver
    }
}

struct WorkerContext<A, Q> {
    adapter: Arc<Mutex<Option<A>>>,
    qmp_factory: Arc<Q>,
    shared: Arc<Shared>,
    worker_active: Arc<AtomicBool>,
    log_dir: PathBuf,
    policy: SupervisorPolicy,
}

fn run_worker<A, Q>(context: WorkerContext<A, Q>, config: GuestConfig, invocation: QemuInvocation)
where
    A: ProcessAdapter,
    Q: QmpFactory,
{
    let WorkerContext {
        adapter,
        qmp_factory,
        shared,
        worker_active,
        log_dir,
        policy,
    } = context;
    let mut lifecycle = Lifecycle::Starting;
    let mut replacement = false;
    loop {
        let logs = match create_log_paths(&log_dir, config.name()) {
            Ok(paths) => paths,
            Err(_) => {
                emit(&shared, GuestState::Failed, None, None);
                break;
            }
        };
        if fs::write(&logs.command, format!("{}\n", invocation.printable())).is_err() {
            emit(&shared, GuestState::Failed, None, Some(logs));
            break;
        }
        let spawn_result = {
            let mut guard = lock_unpoisoned(&adapter);
            guard
                .as_mut()
                .expect("process adapter retained for supervisor lifetime")
                .spawn(&invocation, &logs)
        };
        let mut child = match spawn_result {
            Ok(child) => child,
            Err(_) => {
                emit(&shared, GuestState::Failed, None, Some(logs));
                break;
            }
        };
        let pid = child.pid();
        let (next, _) = lifecycle.on(LifecycleEvent::Spawned);
        lifecycle = next;
        let qmp_address = SocketAddr::from(([127, 0, 0, 1], config.qmp_port()));
        let connect_start = Instant::now();
        let mut qmp = loop {
            match child.try_wait() {
                Ok(Some(code)) => {
                    let (next, _) = lifecycle.on(LifecycleEvent::ProcessExited(code));
                    lifecycle = next;
                    emit(&shared, GuestState::Failed, Some(pid), Some(logs.clone()));
                    break None;
                }
                Err(_) => {
                    emit(&shared, GuestState::Failed, Some(pid), Some(logs.clone()));
                    lifecycle = Lifecycle::Failed;
                    break None;
                }
                Ok(None) => {}
            }
            match qmp_factory.connect(qmp_address, policy.poll_interval) {
                Ok(channel) => break Some(channel),
                Err(_) if connect_start.elapsed() < policy.qmp_ready_deadline => {
                    thread::sleep(policy.poll_interval);
                }
                Err(_) => {
                    let _ = child.kill();
                    let (next, _) =
                        lifecycle.on(LifecycleEvent::QmpUnreachable(connect_start.elapsed()));
                    lifecycle = next;
                    emit(&shared, GuestState::Failed, Some(pid), Some(logs.clone()));
                    break None;
                }
            }
        };
        let Some(mut qmp) = qmp.take() else {
            let _ = lifecycle;
            break;
        };
        let (next, _) = lifecycle.on(LifecycleEvent::QmpReady);
        lifecycle = next;
        emit(&shared, GuestState::Running, Some(pid), Some(logs.clone()));

        let mut reset_seen = false;
        let mut stop_started = None;
        let mut kill_sent = false;
        loop {
            if shared.stop_requested.swap(false, Ordering::AcqRel) && stop_started.is_none() {
                let now = Instant::now();
                let (next, _) = lifecycle.on(LifecycleEvent::StopRequested);
                lifecycle = next;
                emit(&shared, GuestState::Stopping, Some(pid), Some(logs.clone()));
                if !policy.power_off_hook.request_power_off() {
                    let _ = qmp.system_powerdown();
                }
                stop_started = Some(now);
            }
            while let Some(event) = qmp.poll_event() {
                match classify_shutdown(&event) {
                    Some(LifecycleEvent::GuestResetShutdown) if stop_started.is_none() => {
                        reset_seen = true;
                        let (next, _) = lifecycle.on(LifecycleEvent::GuestResetShutdown);
                        lifecycle = next;
                        emit(
                            &shared,
                            GuestState::Restarting,
                            Some(pid),
                            Some(logs.clone()),
                        );
                    }
                    Some(LifecycleEvent::GuestPowerdownShutdown) => {
                        let (next, actions) = lifecycle.on(LifecycleEvent::GuestPowerdownShutdown);
                        lifecycle = next;
                        if actions.contains(&Action::EmitState(GuestState::Stopping)) {
                            emit(&shared, GuestState::Stopping, Some(pid), Some(logs.clone()));
                        }
                    }
                    _ => {}
                }
            }
            match child.try_wait() {
                Ok(Some(code)) => {
                    let (next, _) = lifecycle.on(LifecycleEvent::ProcessExited(code));
                    lifecycle = next;
                    if reset_seen && stop_started.is_none() {
                        replacement = true;
                    } else if lifecycle.state() == GuestState::Stopped {
                        emit(&shared, GuestState::Stopped, None, Some(logs.clone()));
                    } else {
                        emit(&shared, GuestState::Failed, None, Some(logs.clone()));
                    }
                    break;
                }
                Err(_) => {
                    emit(&shared, GuestState::Failed, None, Some(logs.clone()));
                    lifecycle = Lifecycle::Failed;
                    break;
                }
                Ok(None) => {}
            }
            if !kill_sent
                && stop_started.is_some_and(|since| since.elapsed() >= policy.stop_deadline)
            {
                if child.kill().is_err() {
                    emit(&shared, GuestState::Failed, Some(pid), Some(logs.clone()));
                    lifecycle = Lifecycle::Failed;
                    break;
                }
                kill_sent = true;
            }
            thread::sleep(policy.poll_interval);
        }
        if replacement {
            replacement = false;
            lifecycle = Lifecycle::Starting;
            continue;
        }
        let _ = lifecycle;
        break;
    }
    worker_active.store(false, Ordering::Release);
}

fn classify_shutdown(event: &QmpEvent) -> Option<LifecycleEvent> {
    if event.event != "SHUTDOWN" {
        return None;
    }
    let guest = event
        .data
        .get("guest")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let reason = event.data.get("reason").and_then(serde_json::Value::as_str);
    if guest && reason == Some("guest-reset") {
        Some(LifecycleEvent::GuestResetShutdown)
    } else {
        Some(LifecycleEvent::GuestPowerdownShutdown)
    }
}

fn create_log_paths(directory: &Path, name: &str) -> io::Result<LogPaths> {
    fs::create_dir_all(directory)?;
    let timestamp = utc_compact_timestamp(SystemTime::now());
    let stem = format!("qemu-{name}-{timestamp}");
    Ok(LogPaths {
        stdout: directory.join(format!("{stem}.stdout.log")),
        stderr: directory.join(format!("{stem}.stderr.log")),
        command: directory.join(format!("{stem}.cmd.log")),
    })
}

fn utc_compact_timestamp(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_seconds = duration.as_secs();
    let milliseconds = duration.subsec_millis();
    let days = (total_seconds / 86_400) as i64;
    let seconds_of_day = total_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3600;
    let minute = (seconds_of_day % 3600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}{milliseconds:03}")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month as u32, day as u32)
}

fn emit(shared: &Shared, state: GuestState, pid: Option<u32>, logs: Option<LogPaths>) {
    *lock_unpoisoned(&shared.state) = state;
    let event = GuestEvent { state, pid, logs };
    lock_unpoisoned(&shared.subscribers).retain(|sender| sender.send(event.clone()).is_ok());
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Real Windows process adapter using `ProcessLaunch` and a kill-on-close job.
#[cfg(windows)]
pub mod windows_adapter {
    use ome_platform_win::{Child, JobObject, ProcessLaunch};

    use super::*;

    /// QEMU adapter that creates one child-owned kill-on-close job per launch.
    #[derive(Debug, Default)]
    pub struct WindowsProcessAdapter;

    impl ProcessAdapter for WindowsProcessAdapter {
        fn spawn(
            &mut self,
            invocation: &QemuInvocation,
            logs: &LogPaths,
        ) -> Result<Box<dyn ChildProcess>, SpawnError> {
            let stdout = File::create(&logs.stdout)?;
            let stderr = File::create(&logs.stderr)?;
            let job = JobObject::kill_on_close()?;
            let child = ProcessLaunch {
                executable: invocation.program().to_path_buf(),
                arguments: invocation.args().to_vec(),
                stdout,
                stderr,
                cwd: invocation.program().parent().map(Path::to_path_buf),
            }
            .spawn_in_job(&job)
            .map_err(|error| SpawnError::Io(io::Error::other(error)))?;
            Ok(Box::new(WindowsChild { child, _job: job }))
        }
    }

    #[derive(Debug)]
    struct WindowsChild {
        child: Child,
        _job: JobObject,
    }

    impl ChildProcess for WindowsChild {
        fn pid(&self) -> u32 {
            self.child.pid()
        }

        fn try_wait(&mut self) -> Result<Option<i32>, WaitError> {
            self.child
                .try_wait()
                .map(|code| code.map(|value| value as i32))
                .map_err(WaitError::Io)
        }

        fn kill(&mut self) -> Result<(), WaitError> {
            self.child.terminate().map_err(WaitError::Io)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn lifecycle_covers_start_ready_and_stop_escalation() {
        let start = now();
        let state = Lifecycle::Starting;
        let (state, actions) = state.on(LifecycleEvent::Spawned);
        assert_eq!(actions, [Action::ConnectQmp]);
        let (state, actions) = state.on(LifecycleEvent::QmpReady);
        assert_eq!(state, Lifecycle::Running);
        assert_eq!(actions, [Action::EmitState(GuestState::Running)]);
        let stop = start + Duration::from_secs(2);
        let (state, actions) = state.on(LifecycleEvent::StopRequested);
        assert_eq!(
            actions,
            [
                Action::SendPowerdown,
                Action::EmitState(GuestState::Stopping)
            ]
        );
        let (state, actions) = state.on(LifecycleEvent::Tick(stop));
        assert!(actions.is_empty());
        let (_, actions) = state.on(LifecycleEvent::Tick(stop + STOP_DEADLINE));
        assert_eq!(actions, [Action::KillProcess]);
        let (state, actions) = state.on(LifecycleEvent::ProcessExited(0));
        assert_eq!(state, Lifecycle::Stopped);
        assert_eq!(actions, [Action::EmitState(GuestState::Stopped)]);
    }

    #[test]
    fn qmp_timeout_kills_and_keeps_failure_logs() {
        let state = Lifecycle::Starting;
        let (state, actions) = state.on(LifecycleEvent::QmpUnreachable(QMP_READY_DEADLINE));
        assert_eq!(state, Lifecycle::Failed);
        assert_eq!(
            actions,
            [
                Action::KillProcess,
                Action::KeepLogsAsFailure,
                Action::EmitState(GuestState::Failed)
            ]
        );
    }

    #[test]
    fn only_guest_reset_causes_restart() {
        let (state, _) = Lifecycle::Running.on(LifecycleEvent::GuestResetShutdown);
        assert_eq!(state, Lifecycle::Restarting);
        let (state, actions) = state.on(LifecycleEvent::ProcessExited(0));
        assert!(state == Lifecycle::Starting);
        assert_eq!(actions[0], Action::Spawn);

        let (state, _) = Lifecycle::Running.on(LifecycleEvent::GuestPowerdownShutdown);
        let (state, actions) = state.on(LifecycleEvent::ProcessExited(0));
        assert_eq!(state, Lifecycle::Stopped);
        assert!(!actions.contains(&Action::Spawn));

        let (state, actions) = Lifecycle::Running.on(LifecycleEvent::ProcessExited(17));
        assert_eq!(state, Lifecycle::Failed);
        assert!(!actions.contains(&Action::Spawn));
    }

    #[test]
    fn invalid_transitions_are_no_ops() {
        let instant = now();
        let cases = [
            (Lifecycle::Stopped, LifecycleEvent::QmpReady),
            (Lifecycle::Stopped, LifecycleEvent::ProcessExited(0)),
            (Lifecycle::Running, LifecycleEvent::Spawned),
            (Lifecycle::Failed, LifecycleEvent::Tick(instant)),
            (
                Lifecycle::Stopping {
                    since: Some(instant),
                },
                LifecycleEvent::QmpReady,
            ),
        ];
        for (state, event) in cases {
            let (next, actions) = state.on(event);
            assert_eq!(next, state);
            assert!(actions.is_empty());
        }
    }

    #[test]
    fn shutdown_event_classification_requires_guest_reset_fields() {
        let reset = QmpEvent {
            event: "SHUTDOWN".into(),
            data: serde_json::json!({ "guest": true, "reason": "guest-reset" }),
            timestamp: serde_json::Value::Null,
        };
        assert_eq!(
            classify_shutdown(&reset),
            Some(LifecycleEvent::GuestResetShutdown)
        );
        let powerdown = QmpEvent {
            event: "SHUTDOWN".into(),
            data: serde_json::json!({ "guest": true, "reason": "guest-shutdown" }),
            timestamp: serde_json::Value::Null,
        };
        assert_eq!(
            classify_shutdown(&powerdown),
            Some(LifecycleEvent::GuestPowerdownShutdown)
        );
    }

    #[test]
    fn timestamp_has_launcher_shape() {
        let value = utc_compact_timestamp(UNIX_EPOCH + Duration::from_millis(1_700_000_000_123));
        assert_eq!(value.len(), 18);
        assert_eq!(&value[8..9], "-");
        assert!(
            value[..8]
                .chars()
                .all(|character| character.is_ascii_digit())
        );
        assert!(
            value[9..]
                .chars()
                .all(|character| character.is_ascii_digit())
        );
    }

    #[derive(Debug)]
    struct BlockingSpawn {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }

    impl ProcessAdapter for BlockingSpawn {
        fn spawn(
            &mut self,
            _invocation: &QemuInvocation,
            _logs: &LogPaths,
        ) -> Result<Box<dyn ChildProcess>, SpawnError> {
            let _ = self.entered.send(());
            let _ = self.release.recv();
            Err(SpawnError::Unwired)
        }
    }

    #[derive(Debug)]
    struct NeverQmp;

    impl QmpFactory for NeverQmp {
        fn connect(
            &self,
            _address: SocketAddr,
            _timeout: Duration,
        ) -> Result<Box<dyn QmpSession>, QmpError> {
            Err(QmpError::Closed)
        }
    }

    #[test]
    fn supervisor_rejects_second_start() {
        let directory = std::env::temp_dir().join(format!(
            "ome-supervisor-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let mut supervisor = Supervisor::new(
            BlockingSpawn {
                entered: entered_sender,
                release: release_receiver,
            },
            NeverQmp,
            directory.clone(),
            SupervisorPolicy::default(),
        );
        let config = GuestConfig::validate(Default::default()).expect("config");
        let paths = GuestPaths {
            disk: "disk.qcow2".into(),
            firmware_code: "code.fd".into(),
            firmware_vars: "vars.fd".into(),
            iso: None,
        };
        let install = QemuInstall {
            system_exe: "qemu.exe".into(),
        };
        supervisor
            .start(config.clone(), paths.clone(), install.clone())
            .expect("first start admitted");
        entered_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("worker reached spawn");
        assert!(matches!(
            supervisor.start(config, paths, install),
            Err(SupervisorError::InvalidState(GuestState::Starting))
        ));
        release_sender.send(()).expect("release worker");
        for _ in 0..100 {
            if supervisor.state() == GuestState::Failed {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(supervisor.state(), GuestState::Failed);
        let _ = fs::remove_dir_all(directory);
    }
}
