// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Kill-on-close job objects for the QEMU process tree.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;

use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows::core::PCWSTR;

use super::handle::OwnedHandle;
use super::process::Child;

#[derive(Debug)]
pub(crate) struct JobObject(OwnedHandle);

impl JobObject {
    pub(crate) fn kill_on_close() -> io::Result<Self> {
        // SAFETY: no security-attribute pointer and no name are provided. The
        // returned handle is immediately transferred to OwnedHandle; no raw
        // pointer, alignment, bounds, or privilege assumptions are involved.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(super::io_error)
            .and_then(OwnedHandle::from_created)?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the job handle is live and owned. `limits` is aligned and
        // valid for exactly the byte count supplied, remains alive for the
        // call, and only the documented kill-on-close privilege is requested.
        unsafe {
            SetInformationJobObject(
                handle.raw(),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(super::io_error)?;
        Ok(Self(handle))
    }

    pub(crate) fn assign(&self, child: &Child) -> io::Result<()> {
        self.assign_handle(child.raw_handle())
    }

    pub(crate) fn assign_handle(
        &self,
        process: windows::Win32::Foundation::HANDLE,
    ) -> io::Result<()> {
        // SAFETY: both handles are live owned kernel handles. The call retains
        // neither, accepts no memory pointer or bounds, and needs only the
        // assignment right granted by CreateProcessW.
        unsafe { AssignProcessToJobObject(self.0.raw(), process) }.map_err(super::io_error)
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::System::JobObjects::{
        JOB_OBJECT_LIMIT_ACTIVE_PROCESS, QueryInformationJobObject,
    };

    use super::*;

    #[test]
    fn kill_on_close_job_has_no_active_process_limit() {
        let job = JobObject::kill_on_close().expect("create job");
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: job owns a live handle; limits is aligned, writable, and
        // exactly the supplied size. The query reads job metadata only and
        // requires no added privilege or out-of-bounds access.
        unsafe {
            QueryInformationJobObject(
                Some(job.0.raw()),
                JobObjectExtendedLimitInformation,
                (&raw mut limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                None,
            )
        }
        .expect("query job limits");
        assert!(
            limits
                .BasicLimitInformation
                .LimitFlags
                .contains(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)
        );
        assert!(
            !limits
                .BasicLimitInformation
                .LimitFlags
                .contains(JOB_OBJECT_LIMIT_ACTIVE_PROCESS)
        );
    }
}
