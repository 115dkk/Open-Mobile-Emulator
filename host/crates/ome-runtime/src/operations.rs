// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Replaceable native-process and elevation seams for runtime orchestration.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use ome_artifacts::{HttpFetch, UreqFetch};

/// Runs one trusted native executable with discrete arguments and no shell.
pub trait NativeProcessRunner: Send + Sync {
    /// Returns the process exit code, or `-1` when Windows supplies no code.
    fn run(&self, program: &Path, args: &[OsString]) -> Result<i32, String>;
}

/// Production native-process adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessCommandRunner;

impl NativeProcessRunner for ProcessCommandRunner {
    fn run(&self, program: &Path, args: &[OsString]) -> Result<i32, String> {
        let mut command = Command::new(program);
        command.args(args);
        // A console child of a windowed application otherwise opens a console window on the
        // desktop and takes the foreground (CI run 36, docs/evidence/M2/dod-ci.md).
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
            .status()
            .map(|status| status.code().unwrap_or(-1))
            .map_err(|error| error.to_string())
    }
}

/// Failure classes from the fixed elevated setup helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ElevationError {
    /// The user declined the Windows elevation prompt.
    Declined,
    /// Launching or waiting for the helper failed.
    Failed(String),
}

/// Starts the fixed-operation setup helper after explicit user consent.
pub trait ElevationLauncher: Send + Sync {
    /// Runs `ome-setup.exe enable-whpx` and returns its process exit code.
    fn enable_whpx(&self) -> Result<u32, ElevationError>;
}

/// Fail-closed elevation adapter for hosts without a native shell.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableElevation;

impl ElevationLauncher for UnavailableElevation {
    fn enable_whpx(&self) -> Result<u32, ElevationError> {
        Err(ElevationError::Failed(
            "elevation integration is unavailable".to_owned(),
        ))
    }
}

/// Shared dependencies used by background workers.
#[derive(Clone)]
pub struct WorkerDeps {
    /// Native process executor.
    pub process: Arc<dyn NativeProcessRunner>,
    /// Elevated fixed-operation launcher.
    pub elevation: Arc<dyn ElevationLauncher>,
    /// HTTPS client restricted by each operation's explicit host policy.
    pub http: Arc<dyn HttpFetch>,
}

impl std::fmt::Debug for WorkerDeps {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("WorkerDeps").finish_non_exhaustive()
    }
}

impl Default for WorkerDeps {
    fn default() -> Self {
        Self {
            process: Arc::new(ProcessCommandRunner),
            elevation: Arc::new(UnavailableElevation),
            http: Arc::new(UreqFetch::new()),
        }
    }
}

/// Timeout used by native elevation adapters while DISM enables the feature.
pub const ELEVATION_TIMEOUT: Duration = Duration::from_secs(15 * 60);
