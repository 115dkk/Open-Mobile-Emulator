// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Android debugging bridge sessions, output parsing, and package archive handling.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip::ZipArchive;

const MAX_PACKAGE_ARCHIVE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_APK_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);
const POLL_TIMEOUT: Duration = Duration::from_secs(15);

/// Captured process output from one adb invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// Native process exit code; processes terminated by a signal use `-1`.
    pub exit_code: i32,
    /// Unmodified standard output bytes.
    pub stdout: Vec<u8>,
    /// Unmodified standard error bytes.
    pub stderr: Vec<u8>,
}

/// Errors while spawning, waiting for, or terminating an external command.
#[derive(Debug, Error)]
pub enum RunError {
    /// Process creation failed.
    #[error("command could not start")]
    Spawn(#[source] io::Error),
    /// Process output could not be collected.
    #[error("command output could not be read")]
    Io(#[source] io::Error),
    /// The deadline elapsed and the process was terminated.
    #[error("command timed out")]
    Timeout,
    /// A deterministic recorded runner had no matching result.
    #[error("recorded runner has no output")]
    Unwired,
    /// A requested output pipe was unexpectedly unavailable.
    #[error("command output pipe is unavailable")]
    MissingPipe,
    /// An output-reader thread terminated unexpectedly.
    #[error("command output reader failed")]
    ReaderFailed,
}

/// Seam for all adb process execution.
pub trait CommandRunner: Send + Sync {
    /// Runs one program with already-separated arguments and a hard timeout.
    ///
    /// Implementations must not invoke a shell. Timeout is an error after terminating the child;
    /// nonzero process exit is returned as ordinary [`Output`] for the session to classify.
    fn run(&self, program: &Path, args: &[OsString], timeout: Duration)
    -> Result<Output, RunError>;
}

/// Keeps a console child from opening its own console window.
///
/// The product is a windowed application, so every `adb` it runs would otherwise get a fresh
/// console window that appears on the desktop, takes the foreground for a moment, and covers the
/// operating-system window; the boot poll runs adb every few seconds (CI run 36,
/// docs/evidence/M2/dod-ci.md).
fn hide_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

/// Production runner using `std::process::Command` without a shell.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn run(
        &self,
        program: &Path,
        args: &[OsString],
        timeout: Duration,
    ) -> Result<Output, RunError> {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_console_window(&mut command);
        let mut child = command.spawn().map_err(RunError::Spawn)?;
        let stdout = child.stdout.take().ok_or(RunError::MissingPipe)?;
        let stderr = child.stderr.take().ok_or(RunError::MissingPipe)?;
        let stdout_reader = thread::spawn(move || read_pipe(stdout));
        let stderr_reader = thread::spawn(move || read_pipe(stderr));
        let start = Instant::now();
        loop {
            match child.try_wait().map_err(RunError::Io)? {
                Some(status) => {
                    return Ok(Output {
                        exit_code: status.code().unwrap_or(-1),
                        stdout: join_reader(stdout_reader)?,
                        stderr: join_reader(stderr_reader)?,
                    });
                }
                None if start.elapsed() >= timeout => {
                    child.kill().map_err(RunError::Io)?;
                    let _ = child.wait();
                    let _ = join_reader(stdout_reader);
                    let _ = join_reader(stderr_reader);
                    return Err(RunError::Timeout);
                }
                None => thread::sleep(Duration::from_millis(25)),
            }
        }
    }
}

fn read_pipe(mut pipe: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join_reader(reader: thread::JoinHandle<io::Result<Vec<u8>>>) -> Result<Vec<u8>, RunError> {
    reader
        .join()
        .map_err(|_| RunError::ReaderFailed)?
        .map_err(RunError::Io)
}

/// One recorded invocation for assertions in deterministic tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedCall {
    /// Executable passed by the session.
    pub program: PathBuf,
    /// Exact, separate argument elements.
    pub args: Vec<OsString>,
    /// Requested hard timeout.
    pub timeout: Duration,
}

/// Queue-backed test runner that records each invocation.
#[derive(Clone, Debug, Default)]
pub struct RecordedRunner {
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    outputs: Arc<Mutex<VecDeque<Result<Output, RecordedError>>>>,
}

/// Cloneable error supplied to [`RecordedRunner`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordedError {
    /// Produce [`RunError::Timeout`].
    Timeout,
    /// Produce [`RunError::Unwired`].
    Unwired,
}

impl RecordedRunner {
    /// Appends outputs that future recorded invocations consume in order.
    pub fn extend(&self, outputs: impl IntoIterator<Item = Result<Output, RecordedError>>) {
        self.outputs
            .lock()
            .expect("recorded outputs lock")
            .extend(outputs);
    }

    /// Creates a runner whose queued outputs are consumed in order.
    pub fn new(outputs: impl IntoIterator<Item = Result<Output, RecordedError>>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            outputs: Arc::new(Mutex::new(outputs.into_iter().collect())),
        }
    }

    /// Returns a copy of every invocation observed so far.
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().expect("recorded calls lock").clone()
    }
}

impl CommandRunner for RecordedRunner {
    fn run(
        &self,
        program: &Path,
        args: &[OsString],
        timeout: Duration,
    ) -> Result<Output, RunError> {
        self.calls
            .lock()
            .expect("recorded calls lock")
            .push(RecordedCall {
                program: program.to_path_buf(),
                args: args.to_vec(),
                timeout,
            });
        match self
            .outputs
            .lock()
            .expect("recorded outputs lock")
            .pop_front()
            .ok_or(RunError::Unwired)?
        {
            Ok(output) => Ok(output),
            Err(RecordedError::Timeout) => Err(RunError::Timeout),
            Err(RecordedError::Unwired) => Err(RunError::Unwired),
        }
    }
}

/// Injectable monotonic clock and sleeper used by boot polling.
pub trait Clock: Send + Sync {
    /// Returns elapsed monotonic time in a clock-specific epoch.
    fn now(&self) -> Duration;
    /// Sleeps or advances by the requested duration.
    fn sleep(&self, duration: Duration);
}

#[derive(Debug)]
struct SystemClock {
    start: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

/// One adb session pinned to a validated executable and one serial.
#[derive(Clone)]
pub struct AdbSession {
    adb_exe: PathBuf,
    serial: String,
    runner: Arc<dyn CommandRunner>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for AdbSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdbSession")
            .field("adb_exe", &self.adb_exe)
            .field("serial", &self.serial)
            .finish_non_exhaustive()
    }
}

impl AdbSession {
    /// Creates a session without starting adb.
    ///
    /// An empty serial is rejected. The executable and runner remain fixed for the session lifetime.
    pub fn new(
        adb_exe: impl Into<PathBuf>,
        serial: String,
        runner: Box<dyn CommandRunner>,
    ) -> Result<Self, AdbError> {
        Self::with_clock(adb_exe, serial, runner, Box::<SystemClock>::default())
    }

    /// Creates a session with an injectable polling clock.
    ///
    /// This has the same invariants as [`AdbSession::new`] and is intended for deterministic tests.
    pub fn with_clock(
        adb_exe: impl Into<PathBuf>,
        serial: String,
        runner: Box<dyn CommandRunner>,
        clock: Box<dyn Clock>,
    ) -> Result<Self, AdbError> {
        if serial.trim().is_empty() {
            return Err(AdbError::InvalidSerial);
        }
        Ok(Self {
            adb_exe: adb_exe.into(),
            serial,
            runner: runner.into(),
            clock: clock.into(),
        })
    }

    /// Starts the adb server so the client key pair exists, then returns the one-line public key read
    /// from `key_path` (the product passes `%USERPROFILE%\.android\adbkey.pub`), without the trailing newline.
    pub fn host_public_key(&self, key_path: &Path) -> Result<String, AdbError> {
        self.expect_success(&[OsString::from("start-server")], COMMAND_TIMEOUT)?;
        let contents = fs::read_to_string(key_path).map_err(|_| AdbError::HostKeyUnavailable)?;
        let line = contents
            .lines()
            .next()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .ok_or(AdbError::HostKeyUnavailable)?;
        let token = line
            .split_whitespace()
            .next()
            .ok_or(AdbError::HostKeyUnavailable)?;
        if token.is_empty()
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
        {
            return Err(AdbError::HostKeyUnavailable);
        }
        Ok(line.to_owned())
    }

    /// Connects only when the serial has the `host:port` form used by network adb.
    ///
    /// Exit code zero still fails when adb writes `failed to connect` to stdout.
    pub fn connect(&self) -> Result<Option<String>, AdbError> {
        if !is_host_port(&self.serial) {
            return Ok(None);
        }
        let output = self.run(
            &[OsString::from("connect"), OsString::from(&self.serial)],
            COMMAND_TIMEOUT,
        )?;
        let stdout = text(&output.stdout);
        if output.exit_code != 0 || stdout.to_ascii_lowercase().contains("failed to connect") {
            return Err(command_failed(output));
        }
        Ok(Some(stdout.trim().to_owned()))
    }

    /// Waits for the serial and polls `sys.boot_completed` every two seconds.
    ///
    /// Each property poll is capped at 15 seconds and the overall deadline includes connection and
    /// wait-for-device work. A deadline overrun returns [`AdbError::BootTimeout`].
    pub fn wait_for_boot(&self, timeout: Duration) -> Result<(), AdbError> {
        let start = self.clock.now();
        let _ = self.connect()?;
        let remaining = timeout.saturating_sub(self.clock.now().saturating_sub(start));
        if remaining.is_zero() {
            return Err(AdbError::BootTimeout);
        }
        self.expect_success(&self.serial_args(["wait-for-device"]), remaining)?;
        loop {
            let elapsed = self.clock.now().saturating_sub(start);
            if elapsed >= timeout {
                return Err(AdbError::BootTimeout);
            }
            let remaining = timeout - elapsed;
            let output = self.run(
                &self.serial_args(["shell", "getprop", "sys.boot_completed"]),
                remaining.min(POLL_TIMEOUT),
            );
            if let Ok(output) = output
                && output.exit_code == 0
                && text(&output.stdout).trim() == "1"
            {
                return Ok(());
            }
            let elapsed = self.clock.now().saturating_sub(start);
            if elapsed >= timeout {
                return Err(AdbError::BootTimeout);
            }
            self.clock
                .sleep(Duration::from_secs(2).min(timeout - elapsed));
        }
    }

    /// Runs one raw operating-system shell command and preserves stdout and the remote exit code.
    ///
    /// Arguments remain separate all the way to `adb`; callers must not join them into a shell
    /// string. A nonzero remote exit code is returned in [`Output`] so generation adapters can
    /// decide whether an unavailable command is a capability result or an error.
    pub fn shell(&self, args: &[String]) -> Result<Output, AdbError> {
        if args.is_empty() || args.iter().any(|argument| argument.is_empty()) {
            return Err(AdbError::InvalidArgument);
        }
        let mut command = self.serial_prefix();
        command.push(OsString::from("shell"));
        command.extend(args.iter().map(OsString::from));
        self.run(&command, COMMAND_TIMEOUT)
    }

    /// Restarts adbd as root and waits briefly for the selected device to return.
    pub fn root(&self) -> Result<(), AdbError> {
        self.expect_success(&self.serial_args(["root"]), COMMAND_TIMEOUT)?;
        self.wait_for_device(COMMAND_TIMEOUT)
    }

    /// Pushes one trusted local file to an absolute operating-system path.
    pub fn push(&self, local: &Path, remote: &str) -> Result<(), AdbError> {
        if !local.is_file() || !remote.starts_with('/') || remote.chars().any(char::is_whitespace) {
            return Err(AdbError::InvalidArgument);
        }
        let mut command = self.serial_prefix();
        command.extend([
            OsString::from("push"),
            local.as_os_str().to_owned(),
            OsString::from(remote),
        ]);
        self.expect_success(&command, INSTALL_TIMEOUT)?;
        Ok(())
    }

    /// Reads one Android system property.
    pub fn getprop(&self, property: &str) -> Result<String, AdbError> {
        if property.is_empty() || property.chars().any(char::is_whitespace) {
            return Err(AdbError::InvalidArgument);
        }
        let output = self.expect_success(
            &self.serial_args(["shell", "getprop", property]),
            COMMAND_TIMEOUT,
        )?;
        Ok(text(&output.stdout).trim().to_owned())
    }

    /// Installs a previously opened package using single or split adb syntax.
    ///
    /// Archive entries are extracted to a unique temporary directory immediately before invocation
    /// and removed afterward. A package can never be installed before [`AppPackage::open`] succeeds.
    pub fn install(&self, package: &AppPackage) -> Result<(), AdbError> {
        let prepared = package.prepare_install()?;
        let mut args = self.serial_prefix();
        if prepared.paths.len() == 1 {
            args.extend([OsString::from("install"), OsString::from("-r")]);
        } else {
            args.extend([OsString::from("install-multiple"), OsString::from("-r")]);
        }
        args.extend(
            prepared
                .paths
                .iter()
                .map(|path| OsString::from(path.as_os_str())),
        );
        self.expect_success(&args, INSTALL_TIMEOUT)?;
        Ok(())
    }

    /// Launches the package's main launcher activity through Android's monkey command.
    pub fn launch(&self, package: &str) -> Result<(), AdbError> {
        validate_package_name(package)?;
        self.expect_success(
            &self.serial_args([
                "shell",
                "monkey",
                "-p",
                package,
                "-c",
                "android.intent.category.LAUNCHER",
                "1",
            ]),
            COMMAND_TIMEOUT,
        )?;
        Ok(())
    }

    /// Captures an unmodified PNG byte stream from the guest.
    pub fn screencap(&self) -> Result<Vec<u8>, AdbError> {
        let output = self.expect_success(
            &self.serial_args(["exec-out", "screencap", "-p"]),
            COMMAND_TIMEOUT,
        )?;
        if output.stdout.is_empty() {
            return Err(AdbError::EmptyOutput);
        }
        Ok(output.stdout)
    }

    /// Returns the most recent 2000 logcat lines as bytes.
    pub fn logcat_tail(&self) -> Result<Vec<u8>, AdbError> {
        let output = self.expect_success(
            &self.serial_args(["logcat", "-d", "-t", "2000"]),
            COMMAND_TIMEOUT,
        )?;
        Ok(output.stdout)
    }

    /// Lists third-party packages and parsed version codes.
    pub fn packages(&self) -> Result<Vec<InstalledPackage>, AdbError> {
        let output = self.expect_success(
            &self.serial_args([
                "shell",
                "pm",
                "list",
                "packages",
                "-3",
                "--show-versioncode",
            ]),
            COMMAND_TIMEOUT,
        )?;
        Ok(parse_package_list(&text(&output.stdout)))
    }

    /// Returns all installed APK paths for one package.
    pub fn package_paths(&self, package: &str) -> Result<Vec<String>, AdbError> {
        validate_package_name(package)?;
        let output = self.expect_success(
            &self.serial_args(["shell", "pm", "path", package]),
            COMMAND_TIMEOUT,
        )?;
        Ok(parse_package_paths(&text(&output.stdout)))
    }

    /// Returns the installed package version code, or `None` when the package is absent.
    pub fn package_version_code(&self, package: &str) -> Result<Option<u64>, AdbError> {
        validate_package_name(package)?;
        let output = self.expect_success(
            &self.serial_args([
                "shell",
                "pm",
                "list",
                "packages",
                "--show-versioncode",
                package,
            ]),
            COMMAND_TIMEOUT,
        )?;
        Ok(parse_package_list(&text(&output.stdout))
            .into_iter()
            .find(|installed| installed.package == package)
            .and_then(|installed| installed.version_code))
    }

    /// Creates an ephemeral localhost forward to one validated Android abstract socket.
    pub fn forward_localabstract(&self, name: &str) -> Result<u16, AdbError> {
        validate_localabstract_name(name)?;
        let remote = format!("localabstract:{name}");
        let output = self.expect_success(
            &self.serial_args(["forward", "tcp:0", remote.as_str()]),
            COMMAND_TIMEOUT,
        )?;
        let port = text(&output.stdout)
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(AdbError::InvalidForwardPort)?;
        Ok(port)
    }

    /// Removes one previously allocated localhost forward.
    pub fn forward_remove(&self, port: u16) -> Result<(), AdbError> {
        if port == 0 {
            return Err(AdbError::InvalidArgument);
        }
        let local = format!("tcp:{port}");
        self.expect_success(
            &self.serial_args(["forward", "--remove", local.as_str()]),
            COMMAND_TIMEOUT,
        )?;
        Ok(())
    }

    /// Enables one validated Android input-method service.
    pub fn ime_enable(&self, id: &str) -> Result<(), AdbError> {
        validate_ime_id(id)?;
        self.expect_success(
            &self.serial_args(["shell", "ime", "enable", id]),
            COMMAND_TIMEOUT,
        )?;
        Ok(())
    }

    /// Selects one validated Android input-method service.
    pub fn ime_set(&self, id: &str) -> Result<(), AdbError> {
        validate_ime_id(id)?;
        self.expect_success(
            &self.serial_args(["shell", "ime", "set", id]),
            COMMAND_TIMEOUT,
        )?;
        Ok(())
    }

    /// Waits until this serial appears to adb without polling boot properties.
    pub fn wait_for_device(&self, timeout: Duration) -> Result<(), AdbError> {
        self.expect_success(&self.serial_args(["wait-for-device"]), timeout)?;
        Ok(())
    }

    /// Reads `sys.boot_completed` once and returns true only for trimmed stdout `1`.
    pub fn boot_completed(&self) -> Result<bool, AdbError> {
        let output = self.expect_success(
            &self.serial_args(["shell", "getprop", "sys.boot_completed"]),
            POLL_TIMEOUT,
        )?;
        Ok(text(&output.stdout).trim() == "1")
    }

    /// Returns the transport state such as `device`.
    pub fn state(&self) -> Result<String, AdbError> {
        let output = self.expect_success(&self.serial_args(["get-state"]), COMMAND_TIMEOUT)?;
        Ok(text(&output.stdout).trim().to_owned())
    }

    /// Requests Android power-off using `adb reboot -p`.
    pub fn power_off(&self) -> Result<(), AdbError> {
        self.expect_success(&self.serial_args(["reboot", "-p"]), COMMAND_TIMEOUT)?;
        Ok(())
    }

    /// Applies guest display size in `WIDTHxHEIGHT` form.
    pub fn set_display_size(&self, width: u32, height: u32) -> Result<(), AdbError> {
        if width == 0 || height == 0 {
            return Err(AdbError::InvalidArgument);
        }
        let size = format!("{width}x{height}");
        self.expect_success(
            &self.serial_args(["shell", "wm", "size", size.as_str()]),
            COMMAND_TIMEOUT,
        )?;
        Ok(())
    }

    /// Applies guest display density in DPI.
    pub fn set_display_density(&self, density_dpi: u32) -> Result<(), AdbError> {
        if density_dpi == 0 {
            return Err(AdbError::InvalidArgument);
        }
        let density = density_dpi.to_string();
        self.expect_success(
            &self.serial_args(["shell", "wm", "density", density.as_str()]),
            COMMAND_TIMEOUT,
        )?;
        Ok(())
    }

    /// Sets Android media stream volume to the M1-tested value 15.
    pub fn set_media_volume(&self) -> Result<(), AdbError> {
        self.expect_success(
            &self.serial_args([
                "shell",
                "cmd",
                "media_session",
                "volume",
                "--stream",
                "3",
                "--set",
                "15",
            ]),
            COMMAND_TIMEOUT,
        )?;
        Ok(())
    }

    /// Uninstalls one validated Android package name.
    pub fn uninstall(&self, package: &str) -> Result<(), AdbError> {
        validate_package_name(package)?;
        self.expect_success(&self.serial_args(["uninstall", package]), INSTALL_TIMEOUT)?;
        Ok(())
    }

    fn serial_prefix(&self) -> Vec<OsString> {
        vec![OsString::from("-s"), OsString::from(&self.serial)]
    }

    fn serial_args<const N: usize>(&self, tail: [&str; N]) -> Vec<OsString> {
        let mut args = self.serial_prefix();
        args.extend(tail.into_iter().map(OsString::from));
        args
    }

    fn run(&self, args: &[OsString], timeout: Duration) -> Result<Output, AdbError> {
        self.runner
            .run(&self.adb_exe, args, timeout)
            .map_err(AdbError::Run)
    }

    fn expect_success(&self, args: &[OsString], timeout: Duration) -> Result<Output, AdbError> {
        let output = self.run(args, timeout)?;
        if output.exit_code == 0 {
            Ok(output)
        } else {
            Err(command_failed(output))
        }
    }
}

/// Returns the default Windows adb public-key path derived from `USERPROFILE`.
#[must_use]
pub fn default_host_public_key_path() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE")
            .filter(|profile| !profile.is_empty())
            .map(PathBuf::from)
            .map(|profile| profile.join(".android").join("adbkey.pub"))
    } else {
        None
    }
}

fn command_failed(output: Output) -> AdbError {
    let stderr = text(&output.stderr).trim().to_owned();
    let stdout = text(&output.stdout).trim().to_owned();
    AdbError::CommandFailed {
        exit_code: output.exit_code,
        detail: if stderr.is_empty() { stdout } else { stderr },
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn is_host_port(serial: &str) -> bool {
    if let Some(rest) = serial.strip_prefix('[') {
        let Some((_, port)) = rest.rsplit_once("]:") else {
            return false;
        };
        return !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit());
    }
    let Some((host, port)) = serial.rsplit_once(':') else {
        return false;
    };
    !host.is_empty() && !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
}

fn validate_package_name(package: &str) -> Result<(), AdbError> {
    let valid = package.split('.').count() >= 2
        && package.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic())
                && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
        });
    if valid {
        Ok(())
    } else {
        Err(AdbError::InvalidArgument)
    }
}

fn validate_localabstract_name(name: &str) -> Result<(), AdbError> {
    if !name.is_empty()
        && name.len() <= 108
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        Ok(())
    } else {
        Err(AdbError::InvalidArgument)
    }
}

fn validate_ime_id(id: &str) -> Result<(), AdbError> {
    let Some((package, service)) = id.split_once('/') else {
        return Err(AdbError::InvalidArgument);
    };
    validate_package_name(package)?;
    let service = service.strip_prefix('.').unwrap_or(service);
    if service.is_empty()
        || !service.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic())
                && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
    {
        return Err(AdbError::InvalidArgument);
    }
    Ok(())
}

/// Errors from a session command or package preparation.
#[derive(Debug, Error)]
pub enum AdbError {
    /// Empty session serial.
    #[error("adb serial is invalid")]
    InvalidSerial,
    /// A package, property, resolution, or density argument violates its format.
    #[error("adb argument is invalid")]
    InvalidArgument,
    /// Process execution failed or timed out.
    #[error("adb runner failed")]
    Run(#[source] RunError),
    /// adb returned a nonzero exit code or a textual connection failure.
    #[error("adb command failed with exit code {exit_code}: {detail}")]
    CommandFailed {
        /// Native adb exit code.
        exit_code: i32,
        /// Trimmed stderr, or stdout when stderr was empty.
        detail: String,
    },
    /// Guest boot did not complete before the overall deadline.
    #[error("guest boot timed out")]
    BootTimeout,
    /// A binary command unexpectedly returned no bytes.
    #[error("adb returned empty output")]
    EmptyOutput,
    /// `adb forward tcp:0` did not return one nonzero TCP port.
    #[error("adb returned an invalid forward port")]
    InvalidForwardPort,
    /// The host adb public key is missing or malformed.
    #[error("adb host public key is unavailable")]
    HostKeyUnavailable,
    /// A package archive could not be safely prepared.
    #[error("app package could not be prepared")]
    Package(#[from] PackageError),
}

impl AdbError {
    /// Returns true when package installation failed because the installed package uses another signer.
    #[must_use]
    pub fn is_update_incompatible(&self) -> bool {
        matches!(
            self,
            Self::CommandFailed { detail, .. }
                if detail.contains("INSTALL_FAILED_UPDATE_INCOMPATIBLE")
        )
    }
}

/// One parsed third-party package entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackage {
    /// Android application identifier.
    pub package: String,
    /// Version code when adb included it.
    pub version_code: Option<u64>,
}

/// Parses `adb devices -l`, retaining only authorized device rows.
///
/// Daemon chatter, headers, offline devices, and unauthorized devices are ignored.
pub fn parse_devices(output: &str) -> Vec<Device> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            (fields.next()? == "device").then(|| Device {
                serial: serial.to_owned(),
                description: line.trim().to_owned(),
            })
        })
        .collect()
}

/// One authorized adb device row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// adb serial.
    pub serial: String,
    /// Original trimmed row for display or diagnostics.
    pub description: String,
}

/// Parses `pm list packages -3 --show-versioncode` output.
pub fn parse_package_list(output: &str) -> Vec<InstalledPackage> {
    output
        .lines()
        .filter_map(|line| {
            let body = line.trim().strip_prefix("package:")?;
            let mut fields = body.split_whitespace();
            let package = fields.next()?.to_owned();
            let version_code = fields.find_map(|field| {
                field
                    .strip_prefix("versionCode:")
                    .or_else(|| field.strip_prefix("versionCode="))
                    .and_then(|value| value.parse().ok())
            });
            Some(InstalledPackage {
                package,
                version_code,
            })
        })
        .collect()
}

/// Parses `pm path` output and strips the `package:` marker.
pub fn parse_package_paths(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("package:"))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Opened app package whose install members have already been inspected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppPackage {
    /// One ordinary APK file.
    Apk {
        /// Existing APK path checked by [`AppPackage::open`].
        path: PathBuf,
    },
    /// Existing loose APK files that make up one base-and-splits package.
    SplitSet {
        /// Base APK installed first.
        base: PathBuf,
        /// Split APKs in lexical order.
        splits: Vec<PathBuf>,
    },
    /// A ZIP-based XAPK or APKS archive and ordered safe entry names.
    Archive {
        /// Existing archive path.
        path: PathBuf,
        /// Base APK entry, installed first.
        base: String,
        /// Split entries in lexical order.
        splits: Vec<String>,
    },
}

impl AppPackage {
    /// Opens and validates an APK, XAPK, or APKS path before installation.
    ///
    /// APK input must be an existing file. Archive input must contain at least one safe regular APK
    /// entry. A member without `config.` or a `split_` prefix is preferred as base; otherwise the
    /// largest member becomes base. Unsafe archive paths and duplicate names are rejected.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PackageError> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(PackageError::Missing);
        }
        let metadata = fs::metadata(path).map_err(PackageError::Io)?;
        if metadata.len() > MAX_PACKAGE_ARCHIVE_BYTES {
            return Err(PackageError::ArchiveTooLarge);
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .ok_or(PackageError::UnsupportedExtension)?;
        match extension.as_str() {
            "apk" => Ok(Self::Apk {
                path: path.to_path_buf(),
            }),
            "xapk" | "apks" => Self::open_archive(path),
            _ => Err(PackageError::UnsupportedExtension),
        }
    }

    /// Opens and validates loose base and split APK files selected together.
    pub fn open_split_set(paths: &[PathBuf]) -> Result<Self, PackageError> {
        if paths.len() < 2 {
            return Err(PackageError::SplitSetTooSmall);
        }
        let mut names = Vec::with_capacity(paths.len());
        let mut parent = None;
        for path in paths {
            if !path.is_file() {
                return Err(PackageError::Missing);
            }
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .ok_or(PackageError::UnsupportedExtension)?;
            if extension != "apk" {
                return Err(PackageError::UnsupportedExtension);
            }
            let current_parent = path.parent().ok_or(PackageError::InvalidSplitSet)?;
            if let Some(expected_parent) = parent {
                if current_parent != expected_parent {
                    return Err(PackageError::SplitSetDifferentDirectories);
                }
            } else {
                parent = Some(current_parent);
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or(PackageError::InvalidSplitSet)?;
            names.push(name.to_ascii_lowercase());
        }

        let explicit_bases = names
            .iter()
            .enumerate()
            .filter_map(|(index, name)| (name == "base.apk").then_some(index))
            .collect::<Vec<_>>();
        let base_index = match explicit_bases.as_slice() {
            [index] => *index,
            [] => {
                let candidates = names
                    .iter()
                    .enumerate()
                    .filter_map(|(index, name)| is_base_candidate(name).then_some(index))
                    .collect::<Vec<_>>();
                match candidates.as_slice() {
                    [index] => *index,
                    _ => return Err(PackageError::InvalidSplitSet),
                }
            }
            _ => return Err(PackageError::InvalidSplitSet),
        };
        if names
            .iter()
            .enumerate()
            .any(|(index, name)| index != base_index && !is_split_name(name))
        {
            return Err(PackageError::InvalidSplitSet);
        }

        let base = paths[base_index].clone();
        let mut splits = paths
            .iter()
            .enumerate()
            .filter_map(|(index, path)| (index != base_index).then_some(path.clone()))
            .collect::<Vec<_>>();
        splits.sort();
        Ok(Self::SplitSet { base, splits })
    }

    /// Returns install order as displayable source paths or archive member names.
    pub fn members(&self) -> Vec<String> {
        match self {
            Self::Apk { path } => vec![path.to_string_lossy().into_owned()],
            Self::SplitSet { base, splits } => std::iter::once(base)
                .chain(splits)
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            Self::Archive { base, splits, .. } => std::iter::once(base.clone())
                .chain(splits.iter().cloned())
                .collect(),
        }
    }

    fn open_archive(path: &Path) -> Result<Self, PackageError> {
        let file = File::open(path).map_err(PackageError::Io)?;
        let mut archive = ZipArchive::new(file).map_err(PackageError::Zip)?;
        if archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err(PackageError::TooManyEntries);
        }
        let mut entries = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(PackageError::Zip)?;
            if !entry.is_file() || !entry.name().to_ascii_lowercase().ends_with(".apk") {
                continue;
            }
            let enclosed = entry
                .enclosed_name()
                .ok_or(PackageError::UnsafeArchiveEntry)?;
            let name = enclosed
                .to_str()
                .ok_or(PackageError::UnsafeArchiveEntry)?
                .replace('\\', "/");
            if enclosed.components().count() != 1 {
                return Err(PackageError::UnsafeArchiveEntry);
            }
            if entry.size() > MAX_APK_UNCOMPRESSED_BYTES {
                return Err(PackageError::ArchiveTooLarge);
            }
            if !seen.insert(name.clone()) {
                return Err(PackageError::DuplicateArchiveEntry);
            }
            entries.push((name, entry.size()));
        }
        if entries.is_empty() {
            return Err(PackageError::NoApkEntries);
        }
        let base_index = entries
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| is_base_candidate(name))
            .max_by_key(|(_, (_, size))| *size)
            .or_else(|| {
                entries
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, (_, size))| *size)
            })
            .map(|(index, _)| index)
            .expect("entries is nonempty");
        let base = entries.remove(base_index).0;
        let mut splits: Vec<String> = entries.into_iter().map(|(name, _)| name).collect();
        splits.sort();
        Ok(Self::Archive {
            path: path.to_path_buf(),
            base,
            splits,
        })
    }

    fn prepare_install(&self) -> Result<PreparedInstall, PackageError> {
        match self {
            Self::Apk { path } => Ok(PreparedInstall {
                paths: vec![path.clone()],
                temporary: None,
            }),
            Self::SplitSet { base, splits } => Ok(PreparedInstall {
                paths: std::iter::once(base.clone())
                    .chain(splits.iter().cloned())
                    .collect(),
                temporary: None,
            }),
            Self::Archive { path, base, splits } => {
                let directory = unique_extract_directory()?;
                fs::create_dir(&directory).map_err(PackageError::Io)?;
                let result = extract_members(path, &directory, base, splits);
                match result {
                    Ok(paths) => Ok(PreparedInstall {
                        paths,
                        temporary: Some(directory),
                    }),
                    Err(error) => {
                        let _ = fs::remove_dir_all(&directory);
                        Err(error)
                    }
                }
            }
        }
    }
}

fn is_base_candidate(name: &str) -> bool {
    !is_split_name(name)
}

fn is_split_name(name: &str) -> bool {
    let leaf = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(name)
        .to_ascii_lowercase();
    leaf.contains("config.") || leaf.starts_with("split_")
}

fn unique_extract_directory() -> Result<PathBuf, PackageError> {
    let mut candidate = std::env::temp_dir();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| PackageError::TemporaryDirectory)?
        .as_nanos();
    candidate.push(format!("ome-apk-{}-{nonce}", std::process::id()));
    Ok(candidate)
}

fn extract_members(
    archive_path: &Path,
    directory: &Path,
    base: &str,
    splits: &[String],
) -> Result<Vec<PathBuf>, PackageError> {
    let file = File::open(archive_path).map_err(PackageError::Io)?;
    let mut archive = ZipArchive::new(file).map_err(PackageError::Zip)?;
    let mut paths = Vec::with_capacity(splits.len() + 1);
    for (position, member) in std::iter::once(base)
        .chain(splits.iter().map(String::as_str))
        .enumerate()
    {
        let mut entry = archive.by_name(member).map_err(PackageError::Zip)?;
        if !entry.is_file() || entry.enclosed_name().is_none() {
            return Err(PackageError::UnsafeArchiveEntry);
        }
        let leaf = Path::new(member)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(PackageError::UnsafeArchiveEntry)?;
        let target = directory.join(format!("{position:04}-{leaf}"));
        let mut output = File::create(&target).map_err(PackageError::Io)?;
        io::copy(&mut entry, &mut output).map_err(PackageError::Io)?;
        output.flush().map_err(PackageError::Io)?;
        paths.push(target);
    }
    Ok(paths)
}

struct PreparedInstall {
    paths: Vec<PathBuf>,
    temporary: Option<PathBuf>,
}

impl Drop for PreparedInstall {
    fn drop(&mut self) {
        if let Some(directory) = &self.temporary {
            let _ = fs::remove_dir_all(directory);
        }
    }
}

/// Errors while opening or extracting an app package.
#[derive(Debug, Error)]
pub enum PackageError {
    /// Input does not exist as a regular file.
    #[error("package file is missing")]
    Missing,
    /// Only APK, XAPK, and APKS inputs are supported.
    #[error("package extension is unsupported")]
    UnsupportedExtension,
    /// The archive or one expanded APK exceeds its fixed byte bound.
    #[error("package archive is too large")]
    ArchiveTooLarge,
    /// The archive has more than the fixed entry bound.
    #[error("package archive contains too many entries")]
    TooManyEntries,
    /// File-system I/O failed.
    #[error("package I/O failed")]
    Io(#[source] io::Error),
    /// ZIP structure or decompression failed.
    #[error("package archive is invalid")]
    Zip(#[source] zip::result::ZipError),
    /// An archive entry is absolute, escapes the extraction root, or is not UTF-8.
    #[error("package archive contains an invalid entry path")]
    UnsafeArchiveEntry,
    /// An archive repeats an APK member name.
    #[error("package archive repeats an APK entry")]
    DuplicateArchiveEntry,
    /// The archive contains no installable APK member.
    #[error("package archive contains no APK")]
    NoApkEntries,
    /// A loose split set has fewer than two APKs.
    #[error("split APK set must contain at least two files")]
    SplitSetTooSmall,
    /// Loose split APKs are not all in one directory.
    #[error("split APK set spans multiple directories")]
    SplitSetDifferentDirectories,
    /// A loose split set has no unique base or contains a non-split member.
    #[error("split APK set is invalid")]
    InvalidSplitSet,
    /// A unique extraction directory could not be selected.
    #[error("temporary package directory is unavailable")]
    TemporaryDirectory,
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::*;

    fn success(stdout: &str) -> Result<Output, RecordedError> {
        Ok(Output {
            exit_code: 0,
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    #[derive(Debug, Default)]
    struct FakeClock(AtomicU64);

    impl Clock for Arc<FakeClock> {
        fn now(&self) -> Duration {
            Duration::from_millis(self.0.load(Ordering::SeqCst))
        }

        fn sleep(&self, duration: Duration) {
            self.0.fetch_add(
                u64::try_from(duration.as_millis()).expect("test duration fits u64"),
                Ordering::SeqCst,
            );
        }
    }

    #[test]
    fn command_lines_match_launcher_forms() {
        let runner = Box::new(RecordedRunner::new([
            success("connected"),
            success("value\n"),
            success("Success"),
            success("Events injected: 1"),
            success(""),
            success("1\n"),
            success("device"),
            success(""),
            success(""),
            success(""),
            success("Success"),
        ]));
        let session =
            AdbSession::new("adb.exe", "127.0.0.1:5555".to_owned(), runner).expect("session");
        session.connect().expect("connect");
        assert_eq!(
            session.getprop("ro.product.model").expect("property"),
            "value"
        );
        let directory = tempfile::tempdir().expect("temp directory");
        let apk = directory.path().join("one.apk");
        fs::write(&apk, b"apk").expect("write apk");
        session
            .install(&AppPackage::open(&apk).expect("open apk"))
            .expect("install");
        session.launch("com.example.app").expect("launch");
        session
            .wait_for_device(Duration::from_secs(10))
            .expect("wait for device");
        assert!(session.boot_completed().expect("boot completed"));
        assert_eq!(session.state().expect("state"), "device");
        session.power_off().expect("power off");
        session.set_display_size(1280, 720).expect("size");
        session.set_display_density(160).expect("density");
        session.uninstall("com.example.app").expect("uninstall");
    }

    #[test]
    fn host_public_key_starts_server_without_a_serial_and_parses_one_line() {
        let directory = tempfile::tempdir().expect("temp directory");
        let key_path = directory.path().join("adbkey.pub");
        fs::write(&key_path, "QUJDREVGRw== user@host\r\nignored\n").expect("write key");
        let runner = RecordedRunner::new([success("")]);
        let calls = runner.clone();
        let session = AdbSession::new("adb.exe", "127.0.0.1:5555".to_owned(), Box::new(runner))
            .expect("session");

        assert_eq!(
            session.host_public_key(&key_path).expect("host key"),
            "QUJDREVGRw== user@host"
        );
        assert_eq!(calls.calls()[0].args, ["start-server"]);
    }

    #[test]
    fn host_public_key_rejects_empty_and_malformed_tokens() {
        for contents in ["", "not_valid! user@host\n"] {
            let directory = tempfile::tempdir().expect("temp directory");
            let key_path = directory.path().join("adbkey.pub");
            fs::write(&key_path, contents).expect("write key");
            let session = AdbSession::new(
                "adb.exe",
                "serial".to_owned(),
                Box::new(RecordedRunner::new([success("")])),
            )
            .expect("session");
            assert!(matches!(
                session.host_public_key(&key_path),
                Err(AdbError::HostKeyUnavailable)
            ));
        }
    }

    #[test]
    fn host_public_key_maps_start_server_failure_to_adb_error() {
        let directory = tempfile::tempdir().expect("temp directory");
        let key_path = directory.path().join("adbkey.pub");
        fs::write(
            &key_path,
            "QUJD user@host
",
        )
        .expect("write key");
        let session = AdbSession::new(
            "adb.exe",
            "serial".to_owned(),
            Box::new(RecordedRunner::new([Ok(Output {
                exit_code: 1,
                stdout: Vec::new(),
                stderr: b"server failed".to_vec(),
            })])),
        )
        .expect("session");
        assert!(matches!(
            session.host_public_key(&key_path),
            Err(AdbError::CommandFailed { exit_code: 1, .. })
        ));
    }

    #[test]
    fn failed_connect_text_is_an_error_even_with_zero_exit() {
        let runner = Box::new(RecordedRunner::new([success(
            "failed to connect to 127.0.0.1:5555",
        )]));
        let session =
            AdbSession::new("adb.exe", "127.0.0.1:5555".to_owned(), runner).expect("session");
        assert!(matches!(
            session.connect(),
            Err(AdbError::CommandFailed {
                exit_code: 0,
                detail
            }) if detail == "failed to connect to 127.0.0.1:5555"
        ));
    }

    #[test]
    fn parsers_ignore_headers_and_unavailable_devices() {
        let devices = parse_devices(
            "List of devices attached\nabc device product:p model:m\ndef offline\nghi unauthorized\n",
        );
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "abc");

        let packages = parse_package_list(
            "package:com.example.one versionCode:42\npackage:com.example.two versionCode=7\n",
        );
        assert_eq!(
            packages,
            vec![
                InstalledPackage {
                    package: "com.example.one".to_owned(),
                    version_code: Some(42),
                },
                InstalledPackage {
                    package: "com.example.two".to_owned(),
                    version_code: Some(7),
                },
            ]
        );
    }

    #[test]
    fn update_incompatible_is_recognized_without_accepting_other_install_failures() {
        let incompatible = command_failed(Output {
            exit_code: 1,
            stdout: b"Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE: signatures differ]".to_vec(),
            stderr: Vec::new(),
        });
        assert!(incompatible.is_update_incompatible());
        let storage = command_failed(Output {
            exit_code: 1,
            stdout: b"Failure [INSTALL_FAILED_INSUFFICIENT_STORAGE]".to_vec(),
            stderr: Vec::new(),
        });
        assert!(!storage.is_update_incompatible());
    }

    #[test]
    fn ime_and_forward_commands_validate_and_keep_arguments_separate() {
        let runner = RecordedRunner::new([
            success("43219\n"),
            success(""),
            success("Input method org.openmobileemulator.ime/.OmeInputMethodService: now enabled"),
            success("Input method org.openmobileemulator.ime/.OmeInputMethodService selected"),
            success("package:org.openmobileemulator.ime versionCode:17\n"),
        ]);
        let calls = runner.clone();
        let session =
            AdbSession::new("adb.exe", "serial".to_owned(), Box::new(runner)).expect("session");
        assert_eq!(
            session.forward_localabstract("ome-ime").expect("forward"),
            43219
        );
        session.forward_remove(43219).expect("remove forward");
        session
            .ime_enable("org.openmobileemulator.ime/.OmeInputMethodService")
            .expect("enable IME");
        session
            .ime_set("org.openmobileemulator.ime/.OmeInputMethodService")
            .expect("set IME");
        assert_eq!(
            session
                .package_version_code("org.openmobileemulator.ime")
                .expect("package version query"),
            Some(17)
        );
        let calls = calls.calls();
        assert_eq!(
            calls[0].args,
            ["-s", "serial", "forward", "tcp:0", "localabstract:ome-ime"]
        );
        assert_eq!(
            calls[1].args,
            ["-s", "serial", "forward", "--remove", "tcp:43219"]
        );
        assert_eq!(
            calls[2].args,
            [
                "-s",
                "serial",
                "shell",
                "ime",
                "enable",
                "org.openmobileemulator.ime/.OmeInputMethodService"
            ]
        );
        assert_eq!(
            calls[3].args,
            [
                "-s",
                "serial",
                "shell",
                "ime",
                "set",
                "org.openmobileemulator.ime/.OmeInputMethodService"
            ]
        );
    }

    #[test]
    fn ime_and_forward_commands_reject_untrusted_arguments() {
        let runner = RecordedRunner::default();
        let calls = runner.clone();
        let session =
            AdbSession::new("adb.exe", "serial".to_owned(), Box::new(runner)).expect("session");
        assert!(matches!(
            session.forward_localabstract("ome-ime shell"),
            Err(AdbError::InvalidArgument)
        ));
        assert!(matches!(
            session.forward_remove(0),
            Err(AdbError::InvalidArgument)
        ));
        assert!(matches!(
            session.ime_enable("org.openmobileemulator.ime"),
            Err(AdbError::InvalidArgument)
        ));
        assert!(matches!(
            session.ime_set("org.example/app;stop"),
            Err(AdbError::InvalidArgument)
        ));
        assert!(calls.calls().is_empty());
    }

    #[test]
    fn invalid_forward_output_is_rejected() {
        let session = AdbSession::new(
            "adb.exe",
            "serial".to_owned(),
            Box::new(RecordedRunner::new([success("not-a-port\n")])),
        )
        .expect("session");
        assert!(matches!(
            session.forward_localabstract("ome-ime"),
            Err(AdbError::InvalidForwardPort)
        ));
    }

    #[test]
    fn boot_polling_uses_two_second_interval_and_fifteen_second_cap() {
        let runner = Box::new(RecordedRunner::new([
            success("connected"),
            success(""),
            success("0"),
            success(""),
            success("1\n"),
        ]));
        let clock = Arc::new(FakeClock::default());
        let session = AdbSession::with_clock(
            "adb.exe",
            "127.0.0.1:5555".to_owned(),
            runner,
            Box::new(Arc::clone(&clock)),
        )
        .expect("session");
        session
            .wait_for_boot(Duration::from_secs(20))
            .expect("boot succeeds");
        assert_eq!(clock.now(), Duration::from_secs(4));
    }

    fn write_apks(directory: &Path, names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|name| {
                let path = directory.join(name);
                fs::write(&path, b"apk").expect("write APK fixture");
                path
            })
            .collect()
    }

    #[test]
    fn opens_loose_split_set_and_orders_base_first() {
        let directory = tempfile::tempdir().expect("temp directory");
        let paths = write_apks(
            directory.path(),
            &[
                "split_gpdeku.config.arm64_v8a.apk",
                "split_gpdeku.apk",
                "base.apk",
                "split_config.arm64_v8a.apk",
            ],
        );
        let package = AppPackage::open_split_set(&paths).expect("open split set");
        let prepared = package.prepare_install().expect("prepare split set");
        assert_eq!(
            prepared.paths,
            vec![
                directory.path().join("base.apk"),
                directory.path().join("split_config.arm64_v8a.apk"),
                directory.path().join("split_gpdeku.apk"),
                directory.path().join("split_gpdeku.config.arm64_v8a.apk"),
            ]
        );
        assert!(prepared.temporary.is_none());
    }

    #[test]
    fn rejects_loose_split_set_spanning_directories() {
        let first = tempfile::tempdir().expect("first directory");
        let second = tempfile::tempdir().expect("second directory");
        let base = write_apks(first.path(), &["base.apk"]).remove(0);
        let split = write_apks(second.path(), &["split_config.en.apk"]).remove(0);
        assert!(matches!(
            AppPackage::open_split_set(&[base, split]),
            Err(PackageError::SplitSetDifferentDirectories)
        ));
    }

    #[test]
    fn rejects_loose_split_set_with_two_bases() {
        let directory = tempfile::tempdir().expect("temp directory");
        let paths = write_apks(directory.path(), &["first.apk", "second.apk"]);
        assert!(matches!(
            AppPackage::open_split_set(&paths),
            Err(PackageError::InvalidSplitSet)
        ));
    }

    #[test]
    fn rejects_loose_split_set_without_base() {
        let directory = tempfile::tempdir().expect("temp directory");
        let paths = write_apks(
            directory.path(),
            &["split_config.en.apk", "split_config.arm64_v8a.apk"],
        );
        assert!(matches!(
            AppPackage::open_split_set(&paths),
            Err(PackageError::InvalidSplitSet)
        ));
    }

    #[test]
    fn rejects_single_loose_apk_as_split_set() {
        let directory = tempfile::tempdir().expect("temp directory");
        let paths = write_apks(directory.path(), &["base.apk"]);
        assert!(matches!(
            AppPackage::open_split_set(&paths),
            Err(PackageError::SplitSetTooSmall)
        ));
    }

    #[test]
    fn opens_synthetic_archive_and_orders_base_first() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("bundle.apks");
        write_zip(
            &path,
            &[
                ("split_config.en.apk", b"split"),
                ("base.apk", b"base"),
                ("split_config.arm64_v8a.apk", b"arch"),
            ],
        );
        let package = AppPackage::open(&path).expect("open archive");
        assert_eq!(
            package.members(),
            vec![
                "base.apk".to_owned(),
                "split_config.arm64_v8a.apk".to_owned(),
                "split_config.en.apk".to_owned(),
            ]
        );
    }

    #[test]
    fn largest_entry_is_base_when_all_names_are_splits() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("bundle.xapk");
        write_zip(
            &path,
            &[("split_small.apk", b"1"), ("config.large.apk", b"12345")],
        );
        let package = AppPackage::open(&path).expect("open archive");
        assert_eq!(package.members()[0], "config.large.apk");
    }

    #[test]
    fn split_install_extracts_before_invocation() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("bundle.xapk");
        write_zip(&path, &[("base.apk", b"base"), ("split_a.apk", b"split")]);
        let runner = Box::new(RecordedRunner::new([success("Success")]));
        let session = AdbSession::new("adb.exe", "serial".to_owned(), runner).expect("session");
        session
            .install(&AppPackage::open(path).expect("open archive"))
            .expect("install archive");
    }

    #[test]
    fn raw_shell_root_and_push_keep_arguments_separate() {
        let directory = tempfile::tempdir().expect("temp directory");
        let local = directory.path().join("query.sql");
        fs::write(&local, b"select 1;").expect("fixture");
        let runner = RecordedRunner::new([
            success(
                "value
",
            ),
            success(
                "restarting adbd as root
",
            ),
            success(""),
            success(
                "1 file pushed
",
            ),
        ]);
        let calls = runner.clone();
        let session =
            AdbSession::new("adb.exe", "serial".to_owned(), Box::new(runner)).expect("session");
        let output = session
            .shell(&["content".to_owned(), "query".to_owned()])
            .expect("shell output");
        assert_eq!(text(&output.stdout).trim(), "value");
        session.root().expect("root and wait");
        session
            .push(&local, "/data/local/tmp/query.sql")
            .expect("push");
        let calls = calls.calls();
        assert_eq!(calls[0].args, ["-s", "serial", "shell", "content", "query"]);
        assert_eq!(calls[1].args, ["-s", "serial", "root"]);
        assert_eq!(calls[2].args, ["-s", "serial", "wait-for-device"]);
        assert_eq!(calls[3].args[0..3], ["-s", "serial", "push"]);
        assert_eq!(calls[3].args[4], "/data/local/tmp/query.sql");
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let bytes = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(bytes);
        for (name, body) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("start zip entry");
            writer.write_all(body).expect("write zip entry");
        }
        let bytes = writer.finish().expect("finish zip").into_inner();
        fs::write(path, bytes).expect("write zip");
    }
}
