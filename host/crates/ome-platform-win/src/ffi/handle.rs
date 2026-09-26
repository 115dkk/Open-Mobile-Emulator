// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Owned kernel handles and bounded waits.
#![allow(unsafe_code)]

use std::io;
use std::time::Duration;

use windows::Win32::Foundation::{
    CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::WaitForSingleObject;

use crate::WaitOutcome;

#[derive(Debug)]
pub(crate) struct OwnedHandle(HANDLE);

// SAFETY: a Windows kernel handle is a process-wide token. The kernel
// serializes access to the referenced object, and this wrapper owns the sole
// CloseHandle responsibility while moves merely transfer that responsibility.
unsafe impl Send for OwnedHandle {}
// SAFETY: shared references only issue kernel operations that Windows already
// synchronizes; no pointer into Rust-owned memory is retained by the handle.
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    pub(crate) fn from_created(handle: HANDLE) -> io::Result<Self> {
        if handle.is_invalid() || handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }

    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }

    pub(crate) fn wait(&self, timeout: Option<Duration>) -> io::Result<WaitOutcome> {
        let milliseconds = timeout.map_or(u32::MAX, clamp_timeout);
        // SAFETY: `self.0` remains a live handle owned by this wrapper for the
        // call. The API receives no pointers, lengths, or elevated privilege.
        match unsafe { WaitForSingleObject(self.0, milliseconds) } {
            WAIT_OBJECT_0 => Ok(WaitOutcome::Signaled),
            WAIT_TIMEOUT => Ok(WaitOutcome::TimedOut),
            WAIT_ABANDONED => Ok(WaitOutcome::Abandoned),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            other => Err(io::Error::other(format!(
                "unexpected wait result {:#x}",
                other.0
            ))),
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from a successful creation call, has not been
        // closed elsewhere, and this destructor performs its one close. There
        // are no pointers, bounds, alignment, or privilege assumptions.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn clamp_timeout(timeout: Duration) -> u32 {
    timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::{ERROR_INVALID_HANDLE, GetLastError, SetLastError};
    use windows::Win32::System::Threading::CreateEventW;
    use windows::core::PCWSTR;

    use super::*;

    #[test]
    fn owned_handle_closes_exactly_once() {
        // SAFETY: no security attributes or name pointers are supplied. The new
        // event handle is transferred immediately to one OwnedHandle and needs
        // no privilege, buffer bounds, or alignment assumptions.
        let raw = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .expect("create event handle");
        let owner = OwnedHandle::from_created(raw).expect("own event handle");
        drop(owner);
        // SAFETY: this intentionally probes the stale copied token after its
        // sole owner dropped. CloseHandle accepts no pointer or buffer; failure
        // is expected and proves the wrapper already closed the kernel handle.
        assert!(unsafe { CloseHandle(raw) }.is_err());
        // SAFETY: reads only thread-local last-error state; no pointer, handle
        // ownership, bound, alignment, or privilege assumption is involved.
        assert_eq!(unsafe { GetLastError() }, ERROR_INVALID_HANDLE);
        // SAFETY: keep the test from leaking its expected error into later
        // thread-local calls. No pointer, ownership, bounds, or privilege applies.
        unsafe { SetLastError(windows::Win32::Foundation::ERROR_SUCCESS) };
    }

    #[test]
    fn finite_timeout_does_not_become_infinite() {
        assert_eq!(clamp_timeout(Duration::ZERO), 0);
        assert_eq!(clamp_timeout(Duration::from_millis(250)), 250);
        assert_eq!(clamp_timeout(Duration::from_secs(u64::MAX)), u32::MAX - 1);
    }
}
