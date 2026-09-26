// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! `CreateProcessW` with a strict inherited-handle list.
#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;
use std::time::Duration;

use windows::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows::Win32::System::Threading::{
    CREATE_SUSPENDED, CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
    GetCurrentProcess, GetExitCodeProcess, InitializeProcThreadAttributeList,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_CREATION_FLAGS,
    PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR};

use crate::{ProcessLaunch, WaitOutcome};

use super::handle::{OwnedHandle, clamp_timeout};
use super::job::JobObject;

#[derive(Debug)]
pub(crate) struct Child {
    process: OwnedHandle,
    pid: u32,
}

impl Child {
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn raw_handle(&self) -> HANDLE {
        self.process.raw()
    }

    pub(crate) fn try_wait(&self) -> io::Result<Option<u32>> {
        match self.process.wait(Some(Duration::ZERO))? {
            WaitOutcome::TimedOut => Ok(None),
            WaitOutcome::Signaled | WaitOutcome::Abandoned => {
                let mut exit_code = 0_u32;
                // SAFETY: the process handle is live and owned. `exit_code` is an
                // aligned local out value valid for four bytes and the call requires no
                // privilege beyond the query right inherited from CreateProcessW.
                unsafe { GetExitCodeProcess(self.process.raw(), &mut exit_code) }
                    .map_err(super::io_error)?;
                Ok(Some(exit_code))
            }
        }
    }

    pub(crate) fn wait(&self, timeout: Option<Duration>) -> io::Result<WaitOutcome> {
        self.process.wait(timeout)
    }

    pub(crate) fn terminate(&self) -> io::Result<()> {
        // SAFETY: the process handle is live and owned, no pointer or memory
        // bound is passed, and CreateProcessW granted termination privilege.
        unsafe { TerminateProcess(self.process.raw(), 1) }.map_err(super::io_error)
    }
}

pub(crate) fn spawn(launch: ProcessLaunch) -> Result<Child, crate::PlatformError> {
    spawn_with_flags(launch, PROCESS_CREATION_FLAGS(0), None)
}

pub(crate) fn spawn_in_job(
    launch: ProcessLaunch,
    job: &JobObject,
) -> Result<Child, crate::PlatformError> {
    spawn_with_flags(launch, CREATE_SUSPENDED, Some(job))
}

fn spawn_with_flags(
    launch: ProcessLaunch,
    flags: PROCESS_CREATION_FLAGS,
    job: Option<&JobObject>,
) -> Result<Child, crate::PlatformError> {
    let ProcessLaunch {
        executable,
        arguments,
        stdout,
        stderr,
        cwd,
    } = launch;
    let stdin = open_null()?;
    let stdout = duplicate_file_handle(&stdout)?;
    let stderr = duplicate_file_handle(&stderr)?;
    let handles = [stdin.raw(), stdout.raw(), stderr.raw()];
    let mut attributes = AttributeList::with_handles(&handles)?;
    let mut command_line = command_line(&executable, &arguments);
    let application = wide_null(executable.as_os_str());
    let current_directory = cwd.as_ref().map(|path| wide_null(path.as_os_str()));

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin.raw();
    startup.StartupInfo.hStdOutput = stdout.raw();
    startup.StartupInfo.hStdError = stderr.raw();
    startup.lpAttributeList = attributes.pointer();

    let mut information = PROCESS_INFORMATION::default();
    // SAFETY: application and optional cwd are NUL-terminated and live for the
    // call; command_line is a writable NUL-terminated buffer. Startup contains
    // three live owned handles also present in the initialized attribute list.
    // Every pointer is aligned to its Rust type, all lengths were supplied at
    // allocation, output is a local structure, and no elevated token is used.
    unsafe {
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            true,
            EXTENDED_STARTUPINFO_PRESENT | flags,
            None,
            current_directory
                .as_ref()
                .map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr())),
            &raw const startup.StartupInfo,
            &raw mut information,
        )
    }
    .map_err(super::io_error)?;

    // Both returned handles become owned immediately. If wrapping either one
    // fails, the successfully wrapped sibling still drops on the error path.
    let process = OwnedHandle::from_created(information.hProcess);
    let thread = OwnedHandle::from_created(information.hThread);
    let (process, thread) = (process?, thread?);
    if let Some(job) = job {
        if let Err(error) = job.assign_handle(process.raw()) {
            // SAFETY: process is a live owned handle returned by CreateProcessW.
            // The process is still suspended and has spawned no helpers; the
            // call takes no pointer or bounds and uses its termination right.
            let _ = unsafe { TerminateProcess(process.raw(), 1) };
            return Err(error.into());
        }
        // SAFETY: thread is the live owned initial-thread handle and remains
        // suspended. ResumeThread takes no pointer or bounds and requires only
        // the right CreateProcessW granted; a u32::MAX result is failure.
        if unsafe { ResumeThread(thread.raw()) } == u32::MAX {
            let error = io::Error::last_os_error();
            // SAFETY: process remains live and owned; this error path terminates
            // it before either handle drops. No pointers or elevated privilege.
            let _ = unsafe { TerminateProcess(process.raw(), 1) };
            return Err(error.into());
        }
    }
    Ok(Child {
        process,
        pid: information.dwProcessId,
    })
}

fn duplicate_file_handle(file: &File) -> io::Result<OwnedHandle> {
    let source = HANDLE(file.as_raw_handle());
    let current = unsafe {
        // SAFETY: GetCurrentProcess returns a borrowed pseudo-handle requiring
        // no close and carries no pointer, bound, alignment, or privilege risk.
        GetCurrentProcess()
    };
    let mut duplicate = HANDLE::default();
    // SAFETY: `source` remains live through `file`; source and destination
    // process pseudo-handles refer to this process; `duplicate` is an aligned
    // local out value. The duplicate is explicitly inheritable and receives
    // only the source handle's existing rights.
    unsafe {
        DuplicateHandle(
            current,
            source,
            current,
            &raw mut duplicate,
            0,
            true,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .map_err(super::io_error)?;
    OwnedHandle::from_created(duplicate)
}

fn open_null() -> io::Result<OwnedHandle> {
    let name = wide_null(OsStr::new("NUL"));
    let security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: true.into(),
    };
    // SAFETY: name is NUL-terminated; `security` is an aligned live structure
    // for the call and requests inheritable ownership only. No template handle
    // or unbounded buffer is passed, and opening NUL requires no special privilege.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            Some(&raw const security),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(super::io_error)?;
    OwnedHandle::from_created(handle)
}

fn command_line(executable: &std::path::Path, arguments: &[OsString]) -> Vec<u16> {
    let mut command = Vec::new();
    for (index, argument) in std::iter::once(executable.as_os_str())
        .chain(arguments.iter().map(OsString::as_os_str))
        .enumerate()
    {
        if index != 0 {
            command.push(u16::from(b' '));
        }
        command.extend(quote_wide_argument(argument));
    }
    command.push(0);
    command
}

fn quote_wide_argument(value: &OsStr) -> Vec<u16> {
    let units: Vec<u16> = value.encode_wide().collect();
    let requires_quotes = units.is_empty()
        || units
            .iter()
            .any(|unit| matches!(*unit, 0x09..=0x0d | 0x20 | 0x22));
    if !requires_quotes {
        return units;
    }
    let mut quoted = vec![u16::from(b'"')];
    let mut backslashes = 0_usize;
    for unit in units {
        match unit {
            0x5c => backslashes += 1,
            0x22 => {
                quoted.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes * 2 + 1));
                quoted.push(unit);
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes));
                backslashes = 0;
                quoted.push(unit);
            }
        }
    }
    quoted.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes * 2));
    quoted.push(u16::from(b'"'));
    quoted
}

fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

struct AttributeList {
    storage: Vec<usize>,
    initialized: bool,
}

impl AttributeList {
    fn with_handles(handles: &[HANDLE]) -> io::Result<Self> {
        let mut bytes = 0_usize;
        // SAFETY: None is the documented size probe; `bytes` is an aligned
        // local out value. No privilege or caller-owned buffer is involved.
        let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &raw mut bytes) };
        if bytes == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut list = Self {
            storage: vec![0_usize; bytes.div_ceil(size_of::<usize>())],
            initialized: false,
        };
        // SAFETY: storage is word-aligned and its allocation covers at least
        // `bytes`; it stays fixed and live through attribute-list deletion.
        unsafe { InitializeProcThreadAttributeList(Some(list.pointer()), 1, None, &raw mut bytes) }
            .map_err(super::io_error)?;
        list.initialized = true;
        // SAFETY: the list is initialized; `handles` is aligned, live, and
        // exactly `size_of_val(handles)` bytes long. Only those handles gain
        // child inheritance; no additional privilege is introduced.
        unsafe {
            UpdateProcThreadAttribute(
                list.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(handles.as_ptr().cast()),
                std::mem::size_of_val(handles),
                None,
                None,
            )
        }
        .map_err(super::io_error)?;
        Ok(list)
    }

    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.storage.as_mut_ptr().cast())
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: pointer addresses the aligned, live allocation that was
            // successfully initialized once, and this destructor deletes it
            // once before storage is freed. No privileges are involved.
            unsafe { DeleteProcThreadAttributeList(self.pointer()) };
        }
    }
}

#[allow(dead_code)]
fn direct_wait(handle: HANDLE, timeout: Duration) -> io::Result<WaitOutcome> {
    // SAFETY: helper accepts only a live handle from this module and no memory
    // pointers. Timeout is clamped to preserve INFINITE for explicit use.
    match unsafe { WaitForSingleObject(handle, clamp_timeout(timeout)) } {
        WAIT_OBJECT_0 => Ok(WaitOutcome::Signaled),
        WAIT_TIMEOUT => Ok(WaitOutcome::TimedOut),
        _ => Err(io::Error::last_os_error()),
    }
}

#[cfg(test)]
mod tests {
    use std::os::windows::ffi::OsStringExt;

    use super::*;

    #[test]
    fn wide_quoting_preserves_unpaired_surrogates() {
        let value = OsString::from_wide(&[0xd800, u16::from(b' '), u16::from(b'x')]);
        assert_eq!(
            quote_wide_argument(&value),
            [
                u16::from(b'"'),
                0xd800,
                u16::from(b' '),
                u16::from(b'x'),
                u16::from(b'"')
            ]
        );
    }
}
