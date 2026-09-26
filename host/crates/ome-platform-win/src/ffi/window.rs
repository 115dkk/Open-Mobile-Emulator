// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Window enumeration, child hosting, bounds, and DPI state.
#![allow(unsafe_code)]

use std::io;
use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Foundation::{ERROR_SUCCESS, GetLastError, HWND, LPARAM, SetLastError};
use windows::Win32::UI::HiDpi::{
    DPI_HOSTING_BEHAVIOR, DPI_HOSTING_BEHAVIOR_INVALID, DPI_HOSTING_BEHAVIOR_MIXED,
    GetDpiForWindow, SetThreadDpiHostingBehavior,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_STYLE, GetClassNameW, GetParent, GetWindowLongPtrW, GetWindowThreadProcessId,
    SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
    WS_CHILD, WS_POPUP,
};
use windows::core::BOOL;

use crate::PreviousStyle;

#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "SetParent"]
    fn set_parent_raw(child: HWND, new_parent: HWND) -> HWND;
}

struct Enumeration {
    pid: u32,
    windows: Vec<isize>,
}

pub(crate) fn find_windows_of_process(pid: u32) -> io::Result<Vec<isize>> {
    let mut enumeration = Enumeration {
        pid,
        windows: Vec::new(),
    };
    // SAFETY: LPARAM points to an aligned live Enumeration for the synchronous
    // duration of EnumWindows. The callback checks only HWND ownership by PID,
    // appends within Vec's managed bounds, and requires no privilege.
    unsafe {
        EnumWindows(
            Some(enumerate_window),
            LPARAM((&raw mut enumeration).cast::<()>() as isize),
        )
    }
    .map_err(super::io_error)?;
    Ok(enumeration.windows)
}

unsafe extern "system" fn enumerate_window(hwnd: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: EnumWindows invokes this callback synchronously with the exact
    // aligned pointer supplied above; it remains live for every callback. The
    // callback reads/writes only the Enumeration object within Rust bounds and
    // performs no privileged operation.
    let enumeration = unsafe { &mut *(parameter.0 as *mut Enumeration) };
    let mut owner_pid = 0_u32;
    // SAFETY: hwnd is supplied by EnumWindows and `owner_pid` is an aligned
    // four-byte local out value. The API retains no pointers or privileges.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut owner_pid)) };
    if owner_pid == enumeration.pid {
        enumeration.windows.push(hwnd.0 as isize);
    }
    true.into()
}

pub(crate) fn class_name(raw: isize) -> io::Result<String> {
    let hwnd = hwnd(raw)?;
    let mut buffer = [0_u16; 256];
    // SAFETY: hwnd was checked non-null; buffer is aligned, writable, live, and
    // its exact element count is conveyed by the slice. No privilege is used.
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(String::from_utf16_lossy(&buffer[..length as usize]))
}

pub(crate) fn make_child_of(raw: isize, parent_raw: isize) -> io::Result<PreviousStyle> {
    let child = hwnd(raw)?;
    let parent = hwnd(parent_raw)?;
    let style = get_style(child)?;
    // GetParent returning no window is a normal top-level state; errors are not
    // distinguishable or relevant because restoration accepts no parent.
    let old_parent = unsafe {
        // SAFETY: child is a live borrowed HWND. The call retains nothing,
        // takes no memory pointer, and has no privilege requirement.
        GetParent(child).ok().map(|value| value.0 as isize)
    };
    let new_style = ((style as u32 & !WS_POPUP.0) | WS_CHILD.0) as isize;
    set_style(child, new_style)?;
    if let Err(error) = set_parent(child, Some(parent)) {
        let _ = set_style(child, style);
        return Err(error);
    }
    if let Err(error) = refresh_frame(child) {
        let _ = set_parent(child, old_parent.map(|value| HWND(value as *mut _)));
        let _ = set_style(child, style);
        return Err(error);
    }
    Ok(PreviousStyle {
        style,
        parent: old_parent,
    })
}

pub(crate) fn restore_top_level(raw: isize, previous: PreviousStyle) -> io::Result<()> {
    let child = hwnd(raw)?;
    set_style(child, previous.style)?;
    let parent = previous.parent.map(hwnd).transpose()?;
    set_parent(child, parent)?;
    refresh_frame(child)
}

pub(crate) fn set_bounds(raw: isize, x: i32, y: i32, width: i32, height: i32) -> io::Result<()> {
    if width < 0 || height < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "window dimensions must not be negative",
        ));
    }
    let window = hwnd(raw)?;
    // SAFETY: window is a borrowed live HWND. Coordinates and dimensions are
    // plain values; no pointer, array bound, alignment, or privilege applies.
    unsafe { SetWindowPos(window, None, x, y, width, height, SWP_NOZORDER) }
        .map_err(super::io_error)
}

pub(crate) fn dpi(raw: isize) -> io::Result<u32> {
    let window = hwnd(raw)?;
    // SAFETY: window is a borrowed live HWND; the call has no pointers, bounds,
    // alignment requirements, handle ownership transfer, or privilege change.
    let value = unsafe { GetDpiForWindow(window) };
    if value == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(value)
    }
}

#[derive(Debug)]
pub(crate) struct DpiHostingGuard {
    previous: DPI_HOSTING_BEHAVIOR,
    _thread_affine: PhantomData<Rc<()>>,
}

pub(crate) fn set_thread_dpi_hosting_mixed() -> io::Result<DpiHostingGuard> {
    // SAFETY: this changes only the calling thread's hosting behavior and
    // returns the prior plain enum value; no pointer, handle, bounds, alignment,
    // or process privilege is involved.
    let previous = unsafe { SetThreadDpiHostingBehavior(DPI_HOSTING_BEHAVIOR_MIXED) };
    if previous == DPI_HOSTING_BEHAVIOR_INVALID {
        Err(io::Error::last_os_error())
    } else {
        Ok(DpiHostingGuard {
            previous,
            _thread_affine: PhantomData,
        })
    }
}

impl Drop for DpiHostingGuard {
    fn drop(&mut self) {
        // SAFETY: previous came from a successful call on this same thread and
        // is a valid behavior token. No pointers, handles, bounds, or privilege
        // changes are involved beyond restoring thread-local state.
        let _ = unsafe { SetThreadDpiHostingBehavior(self.previous) };
    }
}

fn set_parent(child: HWND, parent: Option<HWND>) -> io::Result<()> {
    // SetParent may validly return null when the old parent was null, so clear
    // and inspect last error rather than using the generated Result wrapper.
    // SAFETY: clearing thread-local last error accepts a plain code and has no
    // pointer, handle ownership, bounds, alignment, or privilege assumptions.
    unsafe { SetLastError(ERROR_SUCCESS) };
    // SAFETY: child and any parent are borrowed live HWND tokens. The function
    // receives no Rust memory pointer, retains only OS object references, and
    // needs no privilege. A null parent explicitly restores top-level status.
    let old_parent = unsafe { set_parent_raw(child, parent.unwrap_or_default()) };
    if old_parent.0.is_null() {
        // SAFETY: reads thread-local state only, with no pointers or privileges.
        let error = unsafe { GetLastError() };
        if error != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(error.0 as i32));
        }
    }
    Ok(())
}

fn get_style(window: HWND) -> io::Result<isize> {
    // GetWindowLongPtrW may validly return zero, so clear and inspect last error.
    // SAFETY: clearing thread-local last error accepts a plain code and has no
    // pointer, handle ownership, bounds, alignment, or privilege assumptions.
    unsafe { SetLastError(ERROR_SUCCESS) };
    // SAFETY: window is a borrowed live HWND; GetWindowLongPtrW receives no Rust
    // pointer, retains no ownership, reads one style value, and needs no privilege.
    let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) };
    if style == 0 {
        // SAFETY: reads thread-local state only, with no pointers or privileges.
        let error = unsafe { GetLastError() };
        if error != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(error.0 as i32));
        }
    }
    Ok(style)
}

fn set_style(window: HWND, style: isize) -> io::Result<()> {
    // SetWindowLongPtrW may validly return zero, so clear and inspect last error.
    // SAFETY: clearing thread-local last error accepts a plain code and has no
    // pointer, handle ownership, bounds, alignment, or privilege assumptions.
    unsafe { SetLastError(ERROR_SUCCESS) };
    // SAFETY: window is a borrowed live HWND; style is a complete GWL_STYLE
    // value. No Rust pointer is passed or retained and no privilege is elevated.
    let previous = unsafe { SetWindowLongPtrW(window, GWL_STYLE, style) };
    if previous == 0 {
        // SAFETY: reads thread-local state only, with no pointers or privileges.
        let error = unsafe { GetLastError() };
        if error != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(error.0 as i32));
        }
    }
    Ok(())
}

fn refresh_frame(window: HWND) -> io::Result<()> {
    // SAFETY: window is a borrowed live HWND; flags explicitly retain position,
    // size, and Z order while recalculating the frame. No pointer or privilege
    // is involved.
    unsafe {
        SetWindowPos(
            window,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        )
    }
    .map_err(super::io_error)
}

fn hwnd(raw: isize) -> io::Result<HWND> {
    if raw == 0 {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "window handle must not be null",
        ))
    } else {
        Ok(HWND(raw as *mut _))
    }
}
