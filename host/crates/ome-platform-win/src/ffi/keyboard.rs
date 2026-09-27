// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Dedicated-thread `WH_KEYBOARD_LL` observation.
#![allow(unsafe_code)]

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED,
    MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_USER,
};

use crate::KeyEvent;

const START_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

static EVENT_SINK: AtomicPtr<Sender<KeyEvent>> = AtomicPtr::new(std::ptr::null_mut());

#[derive(Debug)]
pub(crate) struct KeyboardHook {
    thread_id: u32,
    done: Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl KeyboardHook {
    pub(crate) fn install(sink: Sender<KeyEvent>) -> io::Result<Self> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let thread_id = Arc::new(AtomicU32::new(0));
        let worker_thread_id = Arc::clone(&thread_id);
        let thread = thread::Builder::new()
            .name(String::from("ome-keyboard-hook"))
            .spawn(move || {
                hook_thread(sink, ready_tx, done_tx, worker_thread_id);
            })?;

        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                thread_id: thread_id.load(Ordering::Acquire),
                done: done_rx,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(error) => {
                let _ = post_quit_thread_id(thread_id.load(Ordering::Acquire));
                let _ = thread.join();
                let kind = match error {
                    mpsc::RecvTimeoutError::Timeout => io::ErrorKind::TimedOut,
                    mpsc::RecvTimeoutError::Disconnected => io::ErrorKind::BrokenPipe,
                };
                Err(io::Error::new(
                    kind,
                    "keyboard-hook thread did not initialize",
                ))
            }
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        let _ = post_quit_thread_id(self.thread_id);
        if self.done.recv_timeout(STOP_TIMEOUT).is_ok()
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

fn hook_thread(
    sink: Sender<KeyEvent>,
    ready: mpsc::SyncSender<io::Result<()>>,
    done: mpsc::SyncSender<()>,
    thread_id: Arc<AtomicU32>,
) {
    let sink = Box::new(sink);
    let sink = Box::into_raw(sink);
    if EVENT_SINK
        .compare_exchange(
            std::ptr::null_mut(),
            sink,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        // SAFETY: compare_exchange did not publish this pointer, so this thread
        // retains sole ownership of the Box allocated immediately above.
        drop(unsafe { Box::from_raw(sink) });
        finish_failed_start(
            &ready,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "only one keyboard hook may be installed per process",
            ),
            &done,
        );
        return;
    }
    // SAFETY: this returns the numeric identifier of the current thread and
    // carries no pointers, bounds, ownership transfer, or privilege change.
    thread_id.store(unsafe { GetCurrentThreadId() }, Ordering::Release);

    let mut message = MSG::default();
    // SAFETY: a null-window PM_NOREMOVE probe creates this thread's message
    // queue. `message` is aligned and writable for the call and remains local.
    unsafe {
        let _ = PeekMessageW(&raw mut message, None, WM_USER, WM_USER, PM_NOREMOVE);
    }

    // SAFETY: a null module name asks for the executable module already loaded
    // in this process; no caller-owned pointer or new privilege is involved.
    let module = match unsafe { GetModuleHandleW(None) } {
        Ok(module) => module,
        Err(error) => {
            finish_failed_start(&ready, super::io_error(error), &done);
            return;
        }
    };
    // SAFETY: low_level_keyboard_proc is a static callback in this executable,
    // module identifies that executable, and thread id zero is the documented
    // global scope required by WH_KEYBOARD_LL. The hook remains owned until the
    // owner or this thread unhooks it.
    let hook = match unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            Some(HINSTANCE(module.0)),
            0,
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            finish_failed_start(&ready, super::io_error(error), &done);
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        // SAFETY: this dedicated thread solely owns the successful hook and
        // no callback can remain after the synchronous unhook completes.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
        finish_thread(&done);
        return;
    }

    loop {
        // SAFETY: `message` is aligned, writable, and live for each synchronous
        // call. This dedicated thread owns its queue and pumps every message.
        let result = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetMessageW(&raw mut message, None, 0, 0)
        };
        if result.0 <= 0 {
            break;
        }
        // SAFETY: GetMessageW initialized `message`; neither call retains its
        // pointer. Dispatch invokes only procedures registered with Windows.
        unsafe {
            let _ = TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }

    // SAFETY: this dedicated thread solely owns the successful hook and the
    // message loop has stopped, so unhooking before sink cleanup prevents any
    // callback from observing freed storage.
    let _ = unsafe { UnhookWindowsHookEx(hook) };
    finish_thread(&done);
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    message: WPARAM,
    data: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 && data.0 != 0 {
        // SAFETY: for HC_ACTION on WH_KEYBOARD_LL, Windows specifies that
        // lParam points to one aligned KBDLLHOOKSTRUCT live for this callback.
        let event = unsafe { &*(data.0 as *const KBDLLHOOKSTRUCT) };
        let pressed = match message.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some(true),
            WM_KEYUP | WM_SYSKEYUP => Some(false),
            _ => None,
        };
        if let Some(pressed) = pressed {
            let sink = EVENT_SINK.load(Ordering::Acquire);
            if !sink.is_null() {
                // SAFETY: the hook thread stores one boxed Sender before the
                // hook is installed and clears it only after the message loop
                // and unhook complete. This callback executes on that thread.
                let _ = unsafe { &*sink }.send(KeyEvent {
                    vk: event.vkCode,
                    scan: event.scanCode,
                    pressed,
                    extended: event.flags.contains(LLKHF_EXTENDED),
                    injected: event.flags.contains(LLKHF_INJECTED),
                });
            }
        }
    }
    // SAFETY: chaining every event is required for an observing hook. None is
    // valid because CallNextHookEx ignores its hook parameter on current Windows.
    unsafe { CallNextHookEx(None, code, message, data) }
}

fn finish_failed_start(
    ready: &mpsc::SyncSender<io::Result<()>>,
    error: io::Error,
    done: &mpsc::SyncSender<()>,
) {
    let _ = ready.send(Err(error));
    finish_thread(done);
}

fn finish_thread(done: &mpsc::SyncSender<()>) {
    let sink = EVENT_SINK.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !sink.is_null() {
        // SAFETY: the pointer came from Box::into_raw on this hook thread and
        // the atomic swap gives this cleanup path sole ownership to free it.
        drop(unsafe { Box::from_raw(sink) });
    }
    let _ = done.send(());
}

fn post_quit_thread_id(thread_id: u32) -> io::Result<()> {
    if thread_id == 0 {
        return Ok(());
    }
    // SAFETY: the identifier names the dedicated worker whose queue was created
    // before install returned. WM_QUIT carries no pointers or shared ownership.
    unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.map_err(super::io_error)
}
