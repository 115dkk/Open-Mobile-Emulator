// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Per-thread execution state that keeps the display on while a guest runs.
#![allow(unsafe_code)]

use std::io;

const ES_CONTINUOUS: u32 = 0x8000_0000;
const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "SetThreadExecutionState"]
    fn set_thread_execution_state_raw(flags: u32) -> u32;
}

/// Asks Windows to keep the display on for as long as the calling thread holds the state.
pub(crate) fn require_display() -> io::Result<()> {
    // SAFETY: SetThreadExecutionState takes one plain flag value, has no pointers, and changes
    // only the calling thread's execution state. A zero result reports failure.
    let previous = unsafe { set_thread_execution_state_raw(ES_CONTINUOUS | ES_DISPLAY_REQUIRED) };
    if previous == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Clears the calling thread's display requirement.
pub(crate) fn release_display() {
    // SAFETY: the same plain-value call as above; ES_CONTINUOUS alone clears this thread's
    // requirement. A failure leaves nothing to undo, so the result is ignored.
    let _ = unsafe { set_thread_execution_state_raw(ES_CONTINUOUS) };
}
