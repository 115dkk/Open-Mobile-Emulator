// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Private Win32 FFI modules. Raw handles and pointers do not leave this tree.
#![allow(unsafe_code)]

pub(crate) mod audio;
pub(crate) mod elevation;
pub(crate) mod handle;
pub(crate) mod host;
pub(crate) mod hypervisor;
pub(crate) mod job;
pub(crate) mod mouse;
pub(crate) mod opengl;
pub(crate) mod power;
pub(crate) mod process;
pub(crate) mod screen;
pub(crate) mod test_window;
pub(crate) mod window;

pub(crate) fn io_error(error: windows::core::Error) -> std::io::Error {
    let hresult = error.code().0 as u32;
    let code = if hresult & 0xffff_0000 == 0x8007_0000 {
        hresult & 0xffff
    } else {
        hresult
    };
    std::io::Error::from_raw_os_error(code as i32)
}
