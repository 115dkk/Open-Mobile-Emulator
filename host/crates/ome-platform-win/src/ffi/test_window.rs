// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Plain top-level host window used by native integration tests.
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GetClientRect, GetMessageW, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos, ShowWindow,
    TranslateMessage, UnregisterClassW, WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY, WNDCLASSW,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::PCWSTR;

const START_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_TIMEOUT: Duration = Duration::from_secs(2);
static NEXT_CLASS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(crate) struct TestHostWindow {
    window: isize,
    done: Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl TestHostWindow {
    pub(crate) fn create(title: &str, width: i32, height: i32) -> io::Result<Self> {
        if width <= 0 || height <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "test-window dimensions must be positive",
            ));
        }
        let title = title.to_owned();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name(String::from("ome-test-host-window"))
            .spawn(move || {
                let result = window_thread(&title, width, height, &ready_tx);
                if let Err(error) = result {
                    let _ = ready_tx.send(Err(error));
                }
                let _ = done_tx.send(());
            })?;
        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(window)) => Ok(Self {
                window,
                done: done_rx,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(error) => {
                let _ = thread.join();
                let kind = match error {
                    mpsc::RecvTimeoutError::Timeout => io::ErrorKind::TimedOut,
                    mpsc::RecvTimeoutError::Disconnected => io::ErrorKind::BrokenPipe,
                };
                Err(io::Error::new(
                    kind,
                    "test-window thread did not initialize",
                ))
            }
        }
    }

    pub(crate) fn raw(&self) -> isize {
        self.window
    }

    pub(crate) fn client_size(&self) -> io::Result<(i32, i32)> {
        let window = hwnd(self.window)?;
        let mut client = RECT::default();
        // SAFETY: window remains live while self owns the message-loop thread;
        // client is aligned and writable for exactly one RECT.
        unsafe { GetClientRect(window, &raw mut client) }.map_err(super::io_error)?;
        Ok((client.right - client.left, client.bottom - client.top))
    }

    pub(crate) fn resize(&self, width: i32, height: i32) -> io::Result<()> {
        if width <= 0 || height <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "test-window dimensions must be positive",
            ));
        }
        let window = hwnd(self.window)?;
        let mut outer = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        // SAFETY: outer is one aligned writable RECT and the style/ex-style
        // values exactly match the test window's registration and creation.
        unsafe {
            AdjustWindowRectEx(
                &raw mut outer,
                WS_OVERLAPPEDWINDOW,
                false,
                WINDOW_EX_STYLE(0),
            )
        }
        .map_err(super::io_error)?;
        // SAFETY: window is live; dimensions are validated positive and flags
        // retain its position and Z-order without activating another window.
        unsafe {
            SetWindowPos(
                window,
                None,
                0,
                0,
                outer.right - outer.left,
                outer.bottom - outer.top,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(super::io_error)
    }
}

impl Drop for TestHostWindow {
    fn drop(&mut self) {
        if let Ok(window) = hwnd(self.window) {
            // SAFETY: this posts the pointer-free WM_CLOSE system message to the
            // live test window. Its owner thread performs destruction.
            let _ = unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
        if self.done.recv_timeout(STOP_TIMEOUT).is_ok()
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

fn window_thread(
    title: &str,
    width: i32,
    height: i32,
    ready: &mpsc::SyncSender<io::Result<isize>>,
) -> io::Result<()> {
    let _dpi_guard = super::window::set_thread_dpi_hosting_mixed()?;
    // SAFETY: a null module name returns the already loaded executable module;
    // no caller-owned pointer or new privilege is involved.
    let module = unsafe { GetModuleHandleW(None) }.map_err(super::io_error)?;
    let class_name = wide_null(OsStr::new(&format!(
        "OME_TestHost_{}_{}",
        std::process::id(),
        NEXT_CLASS.fetch_add(1, Ordering::Relaxed)
    )));
    let title = wide_null(OsStr::new(title));
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: HINSTANCE(module.0),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    // SAFETY: class and its NUL-terminated class name remain live until this
    // thread unregisters the class after destroying its sole window.
    if unsafe { RegisterClassW(&raw const class) } == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut outer = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    // SAFETY: outer is one aligned writable RECT. The declared style and zero
    // extended style are the same values supplied to CreateWindowExW below.
    let adjusted = unsafe {
        AdjustWindowRectEx(
            &raw mut outer,
            WS_OVERLAPPEDWINDOW,
            false,
            WINDOW_EX_STYLE(0),
        )
    }
    .map_err(super::io_error);
    let window = adjusted.and_then(|()| {
        // SAFETY: class and title are NUL-terminated and live for the call;
        // the registered procedure is static, and no raw application pointer,
        // menu, owner, or transferred handle is supplied.
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100,
                100,
                outer.right - outer.left,
                outer.bottom - outer.top,
                None,
                None,
                Some(HINSTANCE(module.0)),
                None,
            )
        }
        .map_err(super::io_error)
    });
    let window = match window {
        Ok(window) => window,
        Err(error) => {
            // SAFETY: this thread registered class_name with this module and no
            // window was created, so it is eligible for immediate unregister.
            let _ =
                unsafe { UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(HINSTANCE(module.0))) };
            return Err(error);
        }
    };
    // SAFETY: window is the newly created live top-level window. SW_SHOW has no
    // ownership, pointer, bounds, alignment, or privilege implications.
    let _ = unsafe { ShowWindow(window, windows::Win32::UI::WindowsAndMessaging::SW_SHOW) };
    if ready.send(Ok(window.0 as isize)).is_err() {
        // SAFETY: this thread owns the live window and the receiver has gone;
        // synchronous destruction is valid on its creating thread.
        let _ = unsafe { DestroyWindow(window) };
    } else {
        let mut message = MSG::default();
        loop {
            // SAFETY: message is aligned and writable. This creating thread owns
            // the queue and pumps all messages until WM_QUIT.
            let result = unsafe { GetMessageW(&raw mut message, None, 0, 0) };
            if result.0 <= 0 {
                break;
            }
            // SAFETY: GetMessageW initialized message; neither function retains
            // its pointer, and dispatch reaches the registered static procedure.
            unsafe {
                let _ = TranslateMessage(&raw const message);
                DispatchMessageW(&raw const message);
            }
        }
    }
    // SAFETY: the only window of this class has been destroyed on this thread;
    // the class name and module remain the exact registered pair.
    let _ = unsafe { UnregisterClassW(PCWSTR(class_name.as_ptr()), Some(HINSTANCE(module.0))) };
    Ok(())
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE => {
            // SAFETY: Windows supplied this live HWND to its registered owner
            // procedure, which runs on the creating thread.
            let _ = unsafe { DestroyWindow(window) };
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: called on the window thread; posting WM_QUIT ends only this
            // thread's message loop and carries no pointer or ownership transfer.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => {
            // SAFETY: forwarding every unhandled message with the exact values
            // supplied by Windows satisfies the registered window-proc contract.
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
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

fn wide_null(value: &OsStr) -> Vec<u16> {
    let mut result: Vec<u16> = value.encode_wide().collect();
    result.push(0);
    result
}

const _: () = assert!(std::mem::size_of::<isize>() <= std::mem::size_of::<u64>());
