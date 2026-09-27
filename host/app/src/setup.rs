// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
#![forbid(unsafe_code)]

use std::ffi::OsStr;
use std::path::PathBuf;
#[cfg(not(windows))]
use std::time::Duration;

use ome_runtime::{ELEVATION_TIMEOUT, ElevationError, ElevationLauncher};

/// Launches the bundled fixed-operation helper through the Windows `runas` verb.
#[derive(Clone, Debug)]
pub(crate) struct WindowsElevation {
    helper: PathBuf,
}

impl WindowsElevation {
    pub(crate) fn new(helper: PathBuf) -> Self {
        Self { helper }
    }
}

impl ElevationLauncher for WindowsElevation {
    fn enable_whpx(&self) -> Result<u32, ElevationError> {
        #[cfg(windows)]
        {
            match ome_platform_win::launch_elevated(&self.helper, OsStr::new("enable-whpx")) {
                Ok(child) => child
                    .wait(ELEVATION_TIMEOUT)
                    .map_err(|error| ElevationError::Failed(error.to_string())),
                Err(ome_platform_win::PlatformError::UserDeclinedElevation) => {
                    Err(ElevationError::Declined)
                }
                Err(error) => Err(ElevationError::Failed(error.to_string())),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (OsStr::new("enable-whpx"), Duration::ZERO);
            Err(ElevationError::Failed(
                "elevation is unavailable on this platform".to_owned(),
            ))
        }
    }
}
