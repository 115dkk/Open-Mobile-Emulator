// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Read-only OpenGL observation: which implementation a plain WGL context lands on.
//!
//! virglrenderer needs OpenGL 2.0 with shaders. A host without a display driver that provides
//! OpenGL (a server, a VM with a basic display adapter, a remote session) hands WGL Microsoft's
//! "GDI Generic" OpenGL 1.1, and QEMU then dies with "No provider of glCreateShader found"
//! (`docs/evidence/M2/dod-ci.md`, run 22). The product asks once, on a hidden window, before it
//! chooses the guest's graphics device.
#![allow(unsafe_code)]

use std::ffi::CStr;
use std::io;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetDC, HDC, ReleaseDC};
use windows::Win32::Graphics::OpenGL::{
    ChoosePixelFormat, GL_RENDERER, GL_VERSION, HGLRC, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE,
    PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR, SetPixelFormat, glGetString,
    wglCreateContext, wglDeleteContext, wglMakeCurrent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPED,
};
use windows::core::w;

use super::io_error;

/// Version and renderer strings of the default WGL context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Capability {
    pub(crate) version: String,
    pub(crate) renderer: String,
}

struct ProbeWindow(HWND);

impl Drop for ProbeWindow {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateWindowExW on this thread and is destroyed once.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

struct WindowDc(HWND, HDC);

impl Drop for WindowDc {
    fn drop(&mut self) {
        // SAFETY: the DC came from GetDC on this window and is released exactly once here.
        unsafe {
            ReleaseDC(Some(self.0), self.1);
        }
    }
}

struct Context(HGLRC);

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: the context was created by wglCreateContext on this thread; making no context
        // current before deleting it is the documented order.
        unsafe {
            let _ = wglMakeCurrent(HDC::default(), HGLRC::default());
            let _ = wglDeleteContext(self.0);
        }
    }
}

/// Creates a hidden window, a default pixel format and a WGL context, and reads the strings.
pub(crate) fn capability() -> io::Result<Capability> {
    // SAFETY: "STATIC" is a system window class that needs no registration; the window is hidden,
    // never shown and destroyed by the guard. Every handle below is owned by a guard that releases
    // it in reverse order, and the pixel-format descriptor outlives the calls that read it.
    unsafe {
        let window = ProbeWindow(
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("Open Mobile Emulator OpenGL probe"),
                WS_OVERLAPPED,
                0,
                0,
                16,
                16,
                None,
                None,
                None,
                None,
            )
            .map_err(io_error)?,
        );
        let dc = GetDC(Some(window.0));
        if dc.is_invalid() {
            return Err(io::Error::other("GetDC failed for the OpenGL probe window"));
        }
        let dc = WindowDc(window.0, dc);
        let descriptor = PIXELFORMATDESCRIPTOR {
            nSize: size_of::<PIXELFORMATDESCRIPTOR>() as u16,
            nVersion: 1,
            dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL,
            iPixelType: PFD_TYPE_RGBA,
            cColorBits: 24,
            cDepthBits: 16,
            iLayerType: PFD_MAIN_PLANE.0 as u8,
            ..Default::default()
        };
        let format = ChoosePixelFormat(dc.1, &descriptor);
        if format == 0 {
            return Err(io::Error::other("no OpenGL pixel format is available"));
        }
        SetPixelFormat(dc.1, format, &descriptor).map_err(io_error)?;
        let context = Context(wglCreateContext(dc.1).map_err(io_error)?);
        wglMakeCurrent(dc.1, context.0).map_err(io_error)?;
        let read = |name: u32| -> io::Result<String> {
            let pointer = glGetString(name);
            if pointer.is_null() {
                return Err(io::Error::other("glGetString returned no string"));
            }
            Ok(CStr::from_ptr(pointer.cast())
                .to_string_lossy()
                .into_owned())
        };
        let version = read(GL_VERSION)?;
        let renderer = read(GL_RENDERER)?;
        Ok(Capability { version, renderer })
    }
}
