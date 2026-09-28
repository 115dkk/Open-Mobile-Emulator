// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Mouse injection used by native integration checks.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEINPUT, SendInput,
};
use windows::Win32::UI::WindowsAndMessaging::{SetCursorPos, WindowFromPoint};

pub(crate) fn window_at(x: i32, y: i32) -> isize {
    // SAFETY: WindowFromPoint accepts one fully initialized screen coordinate.
    unsafe { WindowFromPoint(POINT { x, y }).0 as isize }
}

pub(crate) fn click_primary_at(x: i32, y: i32) -> io::Result<()> {
    // SAFETY: coordinates are plain screen-pixel values; no pointer or ownership is involved.
    unsafe { SetCursorPos(x.saturating_sub(16), y.saturating_sub(16)) }.map_err(super::io_error)?;
    std::thread::sleep(std::time::Duration::from_millis(120));
    // SAFETY: the target coordinates have the same plain-value contract.
    unsafe { SetCursorPos(x, y) }.map_err(super::io_error)?;
    std::thread::sleep(std::time::Duration::from_millis(120));
    let event = |flags: MOUSE_EVENT_FLAGS| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let down = [event(MOUSEEVENTF_LEFTDOWN)];
    // SAFETY: down is a live contiguous array of initialized INPUT values and
    // cbSize is exactly the ABI size Windows requires for its element.
    let down_sent = unsafe { SendInput(&down, size_of::<INPUT>() as i32) };
    if down_sent != down.len() as u32 {
        return Err(io::Error::last_os_error());
    }
    std::thread::sleep(std::time::Duration::from_millis(120));
    let up = [event(MOUSEEVENTF_LEFTUP)];
    // SAFETY: up has the same valid representation and lifetime as down.
    let up_sent = unsafe { SendInput(&up, size_of::<INPUT>() as i32) };
    if up_sent == up.len() as u32 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
