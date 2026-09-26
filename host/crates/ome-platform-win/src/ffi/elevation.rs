// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Shell elevation with an owned process handle.
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

use windows::Win32::Foundation::ERROR_CANCELLED;
use windows::Win32::System::Threading::GetExitCodeProcess;
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::core::PCWSTR;

use crate::{PlatformError, WaitOutcome};

use super::handle::OwnedHandle;

#[derive(Debug)]
pub(crate) struct ElevatedChild {
    process: OwnedHandle,
}

impl ElevatedChild {
    pub(crate) fn wait(&self, timeout: Duration) -> Result<u32, PlatformError> {
        match self.process.wait(Some(timeout))? {
            WaitOutcome::TimedOut => Err(PlatformError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "elevated process did not exit before the deadline",
            ))),
            WaitOutcome::Signaled | WaitOutcome::Abandoned => {
                let mut exit_code = 0_u32;
                // SAFETY: process is a live owned handle and exit_code is an
                // aligned local four-byte out value. No buffer bounds or
                // privilege beyond the shell-created handle's query right applies.
                unsafe { GetExitCodeProcess(self.process.raw(), &raw mut exit_code) }
                    .map_err(super::io_error)?;
                Ok(exit_code)
            }
        }
    }
}

pub(crate) fn launch_elevated(
    executable: &Path,
    parameters: &OsStr,
) -> Result<ElevatedChild, PlatformError> {
    let verb = wide_null(OsStr::new("runas"));
    let executable = wide_null(executable.as_os_str());
    let parameters = wide_null(parameters);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(executable.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        ..Default::default()
    };
    // SAFETY: info is fully initialized; all three strings are NUL-terminated,
    // aligned u16 buffers live through the call. The shell writes only within
    // the documented structure bounds. Elevation is explicit via `runas` and
    // no caller-provided executable or parameters outlive this synchronous call.
    if let Err(error) = unsafe { ShellExecuteExW(&raw mut info) } {
        let hresult = error.code().0 as u32;
        if hresult == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0).0 as u32 {
            return Err(PlatformError::UserDeclinedElevation);
        }
        return Err(PlatformError::Io(super::io_error(error)));
    }
    let process = OwnedHandle::from_created(info.hProcess)?;
    Ok(ElevatedChild { process })
}

fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}
