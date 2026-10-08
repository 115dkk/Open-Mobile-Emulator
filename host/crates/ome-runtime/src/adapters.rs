// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Runtime adapters over the raw adb session.
#![forbid(unsafe_code)]

use ome_adb::AdbSession;
use ome_guest_image::{RunnerError, ShellCommand, ShellOutput, ShellRunner};
use ome_supervisor::PowerOffHook;

/// Graceful Android shutdown used by the guest-process supervisor.
#[derive(Clone, Debug)]
pub struct AdbPowerOff {
    session: AdbSession,
}

impl AdbPowerOff {
    /// Creates an independent shutdown owner over one pinned adb session.
    #[must_use]
    pub fn new(session: AdbSession) -> Self {
        Self { session }
    }
}

impl PowerOffHook for AdbPowerOff {
    fn request_power_off(&self) -> bool {
        self.session.connect().is_ok()
            && self
                .session
                .boot_completed()
                .is_ok_and(|completed| completed)
            && self.session.power_off().is_ok()
    }
}

/// Local newtype required because both `AdbSession` and `ShellRunner` belong to sibling crates.
#[derive(Clone, Copy, Debug)]
pub struct AdbShellRunner<'a>(pub &'a AdbSession);

impl ShellRunner for AdbShellRunner<'_> {
    fn shell(&self, command: &ShellCommand) -> Result<ShellOutput, RunnerError> {
        // CapabilityProbe calls `root` once before dispatching commands marked `needs_root`.
        // Other callers must send only commands supplied by a FamilyAdapter method whose
        // contract does not require root.
        let output = self.0.shell(&command.args).map_err(runner_error)?;
        Ok(ShellOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            exit_code: output.exit_code,
        })
    }

    fn root(&self) -> Result<(), RunnerError> {
        self.0.root().map_err(runner_error)
    }
}

fn runner_error(error: ome_adb::AdbError) -> RunnerError {
    RunnerError {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use ome_adb::{Output, RecordedRunner};

    use super::*;

    fn success(stdout: &str) -> Result<Output, ome_adb::RecordedError> {
        Ok(Output {
            exit_code: 0,
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    #[test]
    fn probe_owned_root_step_runs_only_once_before_privileged_shell() {
        let recorded = RecordedRunner::new([
            success("restarting adbd as root"),
            success(""),
            success("value"),
        ]);
        let calls = recorded.clone();
        let session =
            AdbSession::new("adb.exe", "serial".to_owned(), Box::new(recorded)).expect("session");
        let runner = AdbShellRunner(&session);
        runner.root().expect("root");
        let output = runner
            .shell(&ShellCommand::new(["privileged"]).as_root())
            .expect("shell");
        assert_eq!(output.stdout, "value");
        assert_eq!(
            calls
                .calls()
                .into_iter()
                .map(|call| call.args)
                .collect::<Vec<_>>(),
            vec![
                ["-s", "serial", "root"].map(OsString::from).to_vec(),
                ["-s", "serial", "wait-for-device"]
                    .map(OsString::from)
                    .to_vec(),
                ["-s", "serial", "shell", "privileged"]
                    .map(OsString::from)
                    .to_vec(),
            ]
        );
    }

    #[test]
    fn power_off_returns_true_only_after_connected_booted_guest_accepts_request() {
        let recorded = RecordedRunner::new([
            success("connected to 127.0.0.1:5555"),
            success("1"),
            success(""),
        ]);
        let calls = recorded.clone();
        let session = AdbSession::new("adb.exe", "127.0.0.1:5555".to_owned(), Box::new(recorded))
            .expect("session");

        assert!(AdbPowerOff::new(session).request_power_off());
        assert_eq!(
            calls
                .calls()
                .into_iter()
                .map(|call| call.args)
                .collect::<Vec<_>>(),
            vec![
                ["connect", "127.0.0.1:5555"].map(OsString::from).to_vec(),
                [
                    "-s",
                    "127.0.0.1:5555",
                    "shell",
                    "getprop",
                    "sys.boot_completed",
                ]
                .map(OsString::from)
                .to_vec(),
                ["-s", "127.0.0.1:5555", "reboot", "-p"]
                    .map(OsString::from)
                    .to_vec(),
            ]
        );
    }

    #[test]
    fn power_off_returns_false_when_connect_fails() {
        let recorded = RecordedRunner::new([Ok(Output {
            exit_code: 1,
            stdout: b"failed to connect to 127.0.0.1:5555".to_vec(),
            stderr: Vec::new(),
        })]);
        let calls = recorded.clone();
        let session = AdbSession::new("adb.exe", "127.0.0.1:5555".to_owned(), Box::new(recorded))
            .expect("session");

        assert!(!AdbPowerOff::new(session).request_power_off());
        assert_eq!(calls.calls().len(), 1);
    }

    #[test]
    fn power_off_returns_false_when_reboot_command_fails() {
        let recorded = RecordedRunner::new([
            success("already connected to 127.0.0.1:5555"),
            success("1"),
            Ok(Output {
                exit_code: 1,
                stdout: Vec::new(),
                stderr: b"reboot rejected".to_vec(),
            }),
        ]);
        let session = AdbSession::new("adb.exe", "127.0.0.1:5555".to_owned(), Box::new(recorded))
            .expect("session");

        assert!(!AdbPowerOff::new(session).request_power_off());
    }
}
