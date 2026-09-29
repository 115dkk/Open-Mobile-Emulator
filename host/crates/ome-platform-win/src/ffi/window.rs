// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Window enumeration, child hosting, bounds, focus, and DPI state.
#![allow(unsafe_code)]

use std::io;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering};

use windows::Win32::Foundation::{
    ERROR_SUCCESS, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SetLastError, WPARAM,
};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::HiDpi::{
    DPI_HOSTING_BEHAVIOR, DPI_HOSTING_BEHAVIOR_INVALID, DPI_HOSTING_BEHAVIOR_MIXED,
    GetDpiForWindow, SetThreadDpiHostingBehavior,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CWPSTRUCT, CallNextHookEx, EnumWindows, GA_PARENT, GUITHREADINFO, GW_OWNER, GWL_EXSTYLE,
    GWL_STYLE, GWLP_HWNDPARENT, GetAncestor, GetClassNameW, GetClientRect, GetForegroundWindow,
    GetGUIThreadInfo, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    HC_ACTION, HHOOK, HWND_TOP, IsWindow, IsWindowVisible, SMTO_ABORTIFHUNG, SW_HIDE, SW_SHOW,
    SW_SHOWNA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SendMessageTimeoutW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx, WH_CALLWNDPROC, WM_APP, WS_CAPTION,
    WS_CHILD, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};
use windows::core::BOOL;

use crate::{PreviousHosting, PreviousStyle};

#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "SetParent"]
    fn set_parent_raw(child: HWND, new_parent: HWND) -> HWND;
    #[link_name = "SetActiveWindow"]
    fn set_active_window_raw(window: HWND) -> HWND;
    #[link_name = "SetFocus"]
    fn set_focus_raw(window: HWND) -> HWND;
}

const WM_OME_FOCUS_CHILD: u32 = WM_APP + 0x14f;
static FOCUS_CHILD: AtomicIsize = AtomicIsize::new(0);
static FOCUS_RESULT: AtomicI32 = AtomicI32::new(0);
static FOCUS_LOCK: Mutex<()> = Mutex::new(());

struct Enumeration {
    pid: u32,
    windows: Vec<isize>,
}

pub(crate) fn foreground_window() -> isize {
    // SAFETY: GetForegroundWindow takes no pointers, transfers no ownership, and only returns the
    // current system foreground-window token. A null result is represented as zero.
    unsafe { GetForegroundWindow().0 as isize }
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
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut owner_pid)) };
    if thread_id != 0 && owner_pid == enumeration.pid {
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

pub(crate) fn is_visible(raw: isize) -> io::Result<bool> {
    let window = hwnd(raw)?;
    // SAFETY: window is a borrowed HWND checked non-null. The call reads only
    // window state and takes no pointer, bound, ownership, or privilege.
    Ok(unsafe { IsWindowVisible(window) }.as_bool())
}

pub(crate) fn hide(raw: isize) -> io::Result<()> {
    let window = hwnd(raw)?;
    // SAFETY: window is a borrowed non-null HWND. SW_HIDE changes only its
    // visibility; the BOOL reports prior visibility rather than failure.
    let _ = unsafe { ShowWindow(window, SW_HIDE) };
    Ok(())
}

pub(crate) fn show_inactive(raw: isize) -> io::Result<()> {
    let window = hwnd(raw)?;
    // SAFETY: window is a borrowed non-null HWND. SW_SHOWNA makes it visible
    // without activating it; the BOOL reports prior visibility rather than failure.
    let _ = unsafe { ShowWindow(window, SW_SHOWNA) };
    Ok(())
}

pub(crate) fn belongs_to_process(raw: isize, expected_pid: u32) -> bool {
    let Ok(window) = hwnd(raw) else {
        return false;
    };
    let mut actual_pid = 0_u32;
    // SAFETY: window is a borrowed non-null token and actual_pid is one aligned
    // writable u32. A zero thread id reports an invalid or destroyed window.
    let thread_id = unsafe { GetWindowThreadProcessId(window, Some(&raw mut actual_pid)) };
    thread_id != 0 && actual_pid == expected_pid
}

pub(crate) fn has_keyboard_focus(raw: isize) -> io::Result<bool> {
    let window = hwnd(raw)?;
    let mut pid = 0_u32;
    // SAFETY: window is a borrowed non-null token and pid is one aligned writable
    // u32. The returned thread id identifies the window's own input queue.
    let thread_id = unsafe { GetWindowThreadProcessId(window, Some(&raw mut pid)) };
    if thread_id == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: info is aligned, writable, declares its exact structure size, and
    // remains live for the query. The operation reads only another GUI thread's
    // public queue state and transfers no ownership or privilege.
    unsafe { GetGUIThreadInfo(thread_id, &raw mut info) }.map_err(super::io_error)?;
    Ok(info.hwndFocus == window)
}

pub(crate) fn thread_focus(raw: isize) -> io::Result<Option<isize>> {
    let window = hwnd(raw)?;
    let thread_id = window_thread_id(window)?;
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: info is aligned, writable, declares its exact structure size, and
    // remains live for the query. The call only reads GUI queue state.
    unsafe { GetGUIThreadInfo(thread_id, &raw mut info) }.map_err(super::io_error)?;
    Ok((!info.hwndFocus.0.is_null()).then_some(info.hwndFocus.0 as isize))
}

pub(crate) fn make_child_of_at(
    raw: isize,
    parent_raw: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> io::Result<PreviousStyle> {
    validate_dimensions(width, height)?;
    let child = hwnd(raw)?;
    let parent = hwnd(parent_raw)?;
    let previous = PreviousStyle {
        style: get_window_long(child, GWL_STYLE)?,
        ex_style: get_window_long(child, GWL_EXSTYLE)?,
        parent: get_parent(child),
        owner: get_owner(child),
        hosting: PreviousHosting::Child,
    };
    let removed_style = WS_POPUP.0
        | WS_CAPTION.0
        | WS_THICKFRAME.0
        | WS_MINIMIZEBOX.0
        | WS_MAXIMIZEBOX.0
        | WS_SYSMENU.0;
    let new_style = ((previous.style as u32 & !removed_style) | WS_CHILD.0) as isize;
    let new_ex_style = (previous.ex_style as u32 & !WS_EX_APPWINDOW.0) as isize;

    let result = (|| {
        set_window_long(child, GWL_STYLE, new_style)?;
        set_window_long(child, GWL_EXSTYLE, new_ex_style)?;
        set_parent(child, Some(parent))?;
        set_window_pos(
            child,
            None,
            x,
            y,
            width,
            height,
            SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    })();
    if let Err(error) = result {
        restore_best_effort(child, previous);
        return Err(error);
    }
    Ok(previous)
}

pub(crate) fn make_owned_popup(
    raw: isize,
    owner_raw: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> io::Result<PreviousStyle> {
    validate_dimensions(width, height)?;
    let popup = hwnd(raw)?;
    let owner = hwnd(owner_raw)?;
    let previous = PreviousStyle {
        style: get_window_long(popup, GWL_STYLE)?,
        ex_style: get_window_long(popup, GWL_EXSTYLE)?,
        parent: get_parent(popup),
        owner: get_owner(popup),
        hosting: PreviousHosting::OwnedPopup,
    };
    let removed_style =
        WS_CAPTION.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0;
    let new_style = ((previous.style as u32 & !removed_style) | WS_POPUP.0) as isize;
    let new_ex_style = ((previous.ex_style as u32 & !WS_EX_APPWINDOW.0)
        | WS_EX_NOACTIVATE.0
        | WS_EX_TOOLWINDOW.0) as isize;

    let result = (|| {
        set_window_long(popup, GWL_STYLE, new_style)?;
        set_window_long(popup, GWL_EXSTYLE, new_ex_style)?;
        set_owner(popup, Some(owner))?;
        set_window_pos(
            popup,
            Some(HWND_TOP),
            x,
            y,
            width,
            height,
            SWP_FRAMECHANGED | SWP_SHOWWINDOW | SWP_NOACTIVATE,
        )
    })();
    if let Err(error) = result {
        restore_best_effort(popup, previous);
        return Err(error);
    }
    Ok(previous)
}

pub(crate) fn restore_top_level(raw: isize, previous: PreviousStyle) -> io::Result<()> {
    let window = hwnd(raw)?;
    let mut first_error = None;
    match previous.hosting {
        PreviousHosting::Child => {
            if let Err(error) =
                set_parent(window, previous.parent.map(|value| HWND(value as *mut _)))
            {
                first_error = Some(error);
            }
        }
        PreviousHosting::OwnedPopup => {
            if let Err(error) = set_owner(window, None) {
                first_error = Some(error);
            }
        }
    }
    if let Err(error) = set_window_long(window, GWL_STYLE, previous.style) {
        first_error.get_or_insert(error);
    }
    if let Err(error) = set_window_long(window, GWL_EXSTYLE, previous.ex_style) {
        first_error.get_or_insert(error);
    }
    if previous.hosting == PreviousHosting::OwnedPopup {
        if let Some(parent) = previous.parent {
            if let Err(error) = set_parent(window, Some(HWND(parent as *mut _))) {
                first_error.get_or_insert(error);
            }
        } else if let Err(error) =
            set_owner(window, previous.owner.map(|value| HWND(value as *mut _)))
        {
            first_error.get_or_insert(error);
        }
    }
    if let Err(error) = set_window_pos(
        window,
        None,
        0,
        0,
        0,
        0,
        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
    ) {
        first_error.get_or_insert(error);
    }
    // SAFETY: window remains a borrowed live HWND. SW_SHOW requests a visible
    // top-level presentation; its BOOL reports prior visibility, not failure.
    let _ = unsafe { ShowWindow(window, SW_SHOW) };
    first_error.map_or(Ok(()), Err)
}

pub(crate) fn set_bounds(raw: isize, x: i32, y: i32, width: i32, height: i32) -> io::Result<()> {
    validate_dimensions(width, height)?;
    let window = hwnd(raw)?;
    set_window_pos(
        window,
        None,
        x,
        y,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    )
}

pub(crate) fn place_behind(
    raw: isize,
    insert_after_raw: Option<isize>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> io::Result<()> {
    validate_dimensions(width, height)?;
    let window = hwnd(raw)?;
    let insert_after = insert_after_raw.map(hwnd).transpose()?.or(Some(HWND_TOP));
    set_window_pos(window, insert_after, x, y, width, height, SWP_NOACTIVATE)
}

pub(crate) fn focus_child(raw: isize, parent_raw: isize) -> io::Result<()> {
    let child = hwnd(raw)?;
    let parent = hwnd(parent_raw)?;
    let parent_thread = window_thread_id(parent)?;
    let _serial = FOCUS_LOCK
        .lock()
        .map_err(|_| io::Error::other("focus operation lock is poisoned"))?;
    FOCUS_CHILD.store(raw, Ordering::Release);
    FOCUS_RESULT.store(0, Ordering::Release);
    // SAFETY: parent_thread belongs to this process, so the callback may live in
    // this executable and hMod must be null. The hook is removed before return.
    let hook =
        unsafe { SetWindowsHookExW(WH_CALLWNDPROC, Some(parent_focus_proc), None, parent_thread) }
            .map_err(super::io_error)?;
    let _hook = HookGuard(hook);
    // SAFETY: parent is live. The pointer-free private message is delivered with
    // a bounded wait so a blocked UI thread cannot stall the caller indefinitely.
    let result = unsafe {
        SendMessageTimeoutW(
            parent,
            WM_OME_FOCUS_CHILD,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        )
    };
    if result.0 == 0 {
        return Err(io::Error::last_os_error());
    }
    match FOCUS_RESULT.load(Ordering::Acquire) {
        1 if !focused_window(parent, child)? => Ok(()),
        1 => Err(io::Error::other("hosted child retained keyboard focus")),
        error if error > 1 => Err(io::Error::from_raw_os_error(error)),
        _ => Err(io::Error::other(
            "parent thread did not run the focus callback",
        )),
    }
}

pub(crate) fn to_foreground(raw: isize) -> io::Result<()> {
    let window = hwnd(raw)?;
    // SAFETY: window is a borrowed live top-level HWND. This makes it visible and
    // first among non-topmost windows without changing position or size.
    unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        )
    }
    .map_err(super::io_error)?;
    // SAFETY: the live borrowed HWND is now visible and at the front of its
    // Z-order band. Windows may still deny foreground focus by policy; BOOL false
    // is therefore a normal best-effort result rather than a platform failure.
    let _ = unsafe { SetForegroundWindow(window) };
    Ok(())
}

pub(crate) fn window_rect(raw: isize) -> io::Result<(i32, i32, u32, u32)> {
    let window = hwnd(raw)?;
    let mut rect = RECT::default();
    // SAFETY: window is a borrowed live HWND and rect is aligned and writable for exactly one
    // RECT. The synchronous query retains neither the pointer nor ownership of the window.
    unsafe { GetWindowRect(window, &raw mut rect) }.map_err(super::io_error)?;
    let width = u32::try_from(rect.right - rect.left)
        .map_err(|_| io::Error::other("window width is negative"))?;
    let height = u32::try_from(rect.bottom - rect.top)
        .map_err(|_| io::Error::other("window height is negative"))?;
    Ok((rect.left, rect.top, width, height))
}

pub(crate) fn owner(raw: isize) -> Option<isize> {
    let window = hwnd(raw).ok()?;
    get_owner(window)
}

pub(crate) fn is_top_level(raw: isize) -> bool {
    let Ok(window) = hwnd(raw) else {
        return false;
    };
    // SAFETY: window is a borrowed live HWND. GA_ROOT returns the top-level ancestor without
    // conflating an owned popup's owner with a child-parent relationship; no ownership changes.
    unsafe { GetAncestor(window, windows::Win32::UI::WindowsAndMessaging::GA_ROOT) == window }
}

pub(crate) fn has_no_activate_style(raw: isize) -> io::Result<bool> {
    let window = hwnd(raw)?;
    let ex_style = get_window_long(window, GWL_EXSTYLE)? as u32;
    Ok(ex_style & WS_EX_NOACTIVATE.0 != 0)
}

pub(crate) fn client_size(raw: isize) -> io::Result<(i32, i32)> {
    let window = hwnd(raw)?;
    let mut client = RECT::default();
    // SAFETY: window is a borrowed live HWND; client is aligned and writable
    // for exactly one RECT and no pointer is retained after the synchronous call.
    unsafe { GetClientRect(window, &raw mut client) }.map_err(super::io_error)?;
    Ok((client.right - client.left, client.bottom - client.top))
}

pub(crate) fn client_screen_rect(raw: isize) -> io::Result<(i32, i32, u32, u32)> {
    let window = hwnd(raw)?;
    let mut client = RECT::default();
    // SAFETY: window is a borrowed live HWND; client is aligned and writable for one RECT, and the
    // synchronous call retains neither pointer nor handle ownership.
    unsafe { GetClientRect(window, &raw mut client) }.map_err(super::io_error)?;
    let width = u32::try_from(client.right - client.left)
        .map_err(|_| io::Error::other("client width is negative"))?;
    let height = u32::try_from(client.bottom - client.top)
        .map_err(|_| io::Error::other("client height is negative"))?;
    let mut origin = POINT {
        x: client.left,
        y: client.top,
    };
    // SAFETY: window is a borrowed live HWND and origin is one aligned, writable POINT. The API
    // mutates only that point, performs no allocation, and retains no pointer or handle ownership.
    if !unsafe { ClientToScreen(window, &raw mut origin) }.as_bool() {
        return Err(io::Error::last_os_error());
    }
    Ok((origin.x, origin.y, width, height))
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

fn get_parent(child: HWND) -> Option<isize> {
    // SAFETY: child is a borrowed live HWND. GA_PARENT retrieves only a parent,
    // never a top-level window's owner; null is the normal top-level result.
    let parent = unsafe { GetAncestor(child, GA_PARENT) };
    (!parent.0.is_null()).then_some(parent.0 as isize)
}

fn set_parent(child: HWND, parent: Option<HWND>) -> io::Result<()> {
    clear_last_error();
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

fn set_owner(window: HWND, owner: Option<HWND>) -> io::Result<()> {
    set_window_long(
        window,
        GWLP_HWNDPARENT,
        owner.map_or(0, |owner| owner.0 as isize),
    )
}

fn get_owner(window: HWND) -> Option<isize> {
    // SAFETY: window is a borrowed live HWND. GW_OWNER reads only the owner relationship and
    // returns null when no owner exists; no pointer or ownership is transferred.
    unsafe { GetWindow(window, GW_OWNER) }
        .ok()
        .map(|owner| owner.0 as isize)
}

fn get_window_long(
    window: HWND,
    index: windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX,
) -> io::Result<isize> {
    clear_last_error();
    // SAFETY: window is a borrowed live HWND; GetWindowLongPtrW receives no Rust
    // pointer, retains no ownership, reads one style value, and needs no privilege.
    let value = unsafe { GetWindowLongPtrW(window, index) };
    if value == 0 {
        // SAFETY: reads thread-local state only, with no pointers or privileges.
        let error = unsafe { GetLastError() };
        if error != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(error.0 as i32));
        }
    }
    Ok(value)
}

fn set_window_long(
    window: HWND,
    index: windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX,
    value: isize,
) -> io::Result<()> {
    clear_last_error();
    // SAFETY: window is a borrowed live HWND and value is a complete style or
    // extended-style value. No Rust pointer or ownership is passed or retained.
    let previous = unsafe { SetWindowLongPtrW(window, index, value) };
    if previous == 0 {
        // SAFETY: reads thread-local state only, with no pointers or privileges.
        let error = unsafe { GetLastError() };
        if error != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(error.0 as i32));
        }
    }
    Ok(())
}

fn set_window_pos(
    window: HWND,
    insert_after: Option<HWND>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    flags: windows::Win32::UI::WindowsAndMessaging::SET_WINDOW_POS_FLAGS,
) -> io::Result<()> {
    // SAFETY: window and any insertion predecessor are borrowed live HWND values. Coordinates,
    // dimensions, and flags are plain values; no pointer, array bound, or ownership applies.
    unsafe { SetWindowPos(window, insert_after, x, y, width, height, flags) }
        .map_err(super::io_error)
}

fn restore_best_effort(window: HWND, previous: PreviousStyle) {
    match previous.hosting {
        PreviousHosting::Child => {
            let _ = set_parent(window, previous.parent.map(|value| HWND(value as *mut _)));
        }
        PreviousHosting::OwnedPopup => {
            let _ = set_owner(window, None);
        }
    }
    let _ = set_window_long(window, GWL_STYLE, previous.style);
    let _ = set_window_long(window, GWL_EXSTYLE, previous.ex_style);
    if previous.hosting == PreviousHosting::OwnedPopup {
        if let Some(parent) = previous.parent {
            let _ = set_parent(window, Some(HWND(parent as *mut _)));
        } else {
            let _ = set_owner(window, previous.owner.map(|value| HWND(value as *mut _)));
        }
    }
    let _ = set_window_pos(
        window,
        None,
        0,
        0,
        0,
        0,
        SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

fn window_thread_id(window: HWND) -> io::Result<u32> {
    let mut pid = 0_u32;
    // SAFETY: window is a borrowed non-null HWND and pid is one aligned writable
    // u32. A zero return reports an invalid or destroyed window.
    let thread_id = unsafe { GetWindowThreadProcessId(window, Some(&raw mut pid)) };
    if thread_id == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(thread_id)
    }
}

struct HookGuard(HHOOK);

impl Drop for HookGuard {
    fn drop(&mut self) {
        // SAFETY: this guard solely owns the successful thread-hook registration
        // and releases it exactly once after the synchronous focus message.
        let _ = unsafe { UnhookWindowsHookEx(self.0) };
    }
}

unsafe extern "system" fn parent_focus_proc(code: i32, wparam: WPARAM, data: LPARAM) -> LRESULT {
    if code >= HC_ACTION as i32 && data.0 != 0 {
        // SAFETY: WH_CALLWNDPROC specifies that lParam points to one aligned
        // CWPSTRUCT live for this callback invocation.
        let message = unsafe { &*(data.0 as *const CWPSTRUCT) };
        if message.message == WM_OME_FOCUS_CHILD {
            let child = HWND(FOCUS_CHILD.load(Ordering::Acquire) as *mut _);
            // SAFETY: the value came from the validated child token stored just
            // before this synchronous message; this catches destruction races.
            if !unsafe { IsWindow(Some(child)) }.as_bool() {
                FOCUS_RESULT.store(1400, Ordering::Release);
                // SAFETY: this hook still observes only and must preserve the chain.
                return unsafe { CallNextHookEx(None, code, wparam, data) };
            }
            // SAFETY: the callback runs on the live top-level parent's thread.
            // Raising it without activation makes its embedded child reachable
            // even when foreground policy rejects SetForegroundWindow below.
            let result = unsafe {
                SetWindowPos(
                    message.hwnd,
                    Some(HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
            }
            .map_err(super::io_error);
            if let Err(error) = result {
                FOCUS_RESULT.store(error.raw_os_error().unwrap_or(1).max(2), Ordering::Release);
                // SAFETY: this hook must preserve the chain for every message.
                return unsafe { CallNextHookEx(None, code, wparam, data) };
            }
            // SAFETY: the validated hosted child remains live for this callback.
            // Raising it among siblings does not activate it.
            let result = unsafe {
                SetWindowPos(
                    child,
                    Some(HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
            }
            .map_err(super::io_error);
            if let Err(error) = result {
                FOCUS_RESULT.store(error.raw_os_error().unwrap_or(1).max(2), Ordering::Release);
                // SAFETY: this hook must preserve the chain for every message.
                return unsafe { CallNextHookEx(None, code, wparam, data) };
            }
            // SAFETY: this callback runs on the parent window's own thread and
            // the message target is that live parent HWND.
            let _ = unsafe { set_active_window_raw(message.hwnd) };
            // SAFETY: the same live parent is the foreground target. Windows may
            // reject this by foreground policy; focus correction remains local.
            let _ = unsafe { SetForegroundWindow(message.hwnd) };
            let result = match focused_window(message.hwnd, child) {
                Ok(true) => {
                    clear_last_error();
                    // SAFETY: the callback runs on the parent owner thread and
                    // message.hwnd remains its live top-level window.
                    check_nullable_success(unsafe { set_focus_raw(message.hwnd) })
                }
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            };
            FOCUS_RESULT.store(
                match result {
                    Ok(()) => 1,
                    Err(error) => error.raw_os_error().unwrap_or(1).max(2),
                },
                Ordering::Release,
            );
        }
    }
    // SAFETY: this hook observes one private message and chains every event with
    // the exact values Windows supplied, preserving other hooks in the chain.
    unsafe { CallNextHookEx(None, code, wparam, data) }
}

fn focused_window(thread_window: HWND, candidate: HWND) -> io::Result<bool> {
    let thread_id = window_thread_id(thread_window)?;
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: info is aligned and writable for one synchronous queue-state read.
    unsafe { GetGUIThreadInfo(thread_id, &raw mut info) }.map_err(super::io_error)?;
    Ok(info.hwndFocus == candidate)
}

pub(crate) unsafe fn send_test_window_message(
    window: HWND,
    message: u32,
    wparam: WPARAM,
) -> io::Result<()> {
    // SAFETY: callers pass one live test window and a private pointer-free
    // message. The bounded call retains no caller-owned memory.
    let result = unsafe {
        SendMessageTimeoutW(
            window,
            message,
            wparam,
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        )
    };
    if result.0 == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn check_nullable_success(result: HWND) -> io::Result<()> {
    if result.0.is_null() {
        // SAFETY: reads thread-local last-error state only. SetFocus may validly
        // return null when no window previously owned keyboard focus.
        let error = unsafe { GetLastError() };
        if error != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(error.0 as i32));
        }
    }
    Ok(())
}

fn clear_last_error() {
    // SAFETY: clearing thread-local last error accepts a plain code and has no
    // pointer, handle ownership, bounds, alignment, or privilege assumptions.
    unsafe { SetLastError(ERROR_SUCCESS) };
}

fn validate_dimensions(width: i32, height: i32) -> io::Result<()> {
    if width < 0 || height < 0 {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "window dimensions must not be negative",
        ))
    } else {
        Ok(())
    }
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
