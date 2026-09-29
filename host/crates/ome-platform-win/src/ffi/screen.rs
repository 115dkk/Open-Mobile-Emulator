// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Screen sampling for native integration checks: the share of lit pixels in a screen region.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HBITMAP, HDC, HGDIOBJ, ReleaseDC,
    SRCCOPY, SelectObject,
};

/// Distance between sampled pixels on each axis.
const SAMPLE_STEP: usize = 12;
/// A sampled pixel counts as lit when its red, green and blue sum exceeds this value.
const LIT_THRESHOLD: u32 = 60;

struct ScreenDc(HDC);

impl Drop for ScreenDc {
    fn drop(&mut self) {
        // SAFETY: the DC came from GetDC(NULL) on this thread and is released exactly once here
        // with the same null window it was obtained for.
        unsafe {
            ReleaseDC(None, self.0);
        }
    }
}

struct MemoryDc(HDC);

impl Drop for MemoryDc {
    fn drop(&mut self) {
        // SAFETY: the DC came from CreateCompatibleDC, is owned solely by this guard, and every
        // object selected into it has been deselected by the Selection guard dropped earlier.
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: the bitmap came from CreateCompatibleBitmap, is owned solely by this guard, and
        // is no longer selected into any DC when the guard drops.
        unsafe {
            let _ = DeleteObject(self.0.into());
        }
    }
}

struct Selection {
    dc: HDC,
    previous: HGDIOBJ,
}

impl Drop for Selection {
    fn drop(&mut self) {
        // SAFETY: dc is the live memory DC and previous is the object SelectObject returned for
        // it, so restoring it deselects the bitmap without leaking either object.
        unsafe {
            SelectObject(self.dc, self.previous);
        }
    }
}

pub(crate) fn lit_share(x: i32, y: i32, width: u32, height: u32) -> io::Result<f64> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "region size is out of range");
    let width_i32 = i32::try_from(width).map_err(|_| invalid())?;
    let height_i32 = i32::try_from(height).map_err(|_| invalid())?;
    if width_i32 <= 0 || height_i32 <= 0 {
        return Err(invalid());
    }
    let row = usize::try_from(width).map_err(|_| invalid())?;
    let rows = usize::try_from(height).map_err(|_| invalid())?;
    let bytes = row
        .checked_mul(rows)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(invalid)?;

    // SAFETY: GetDC(NULL) borrows the screen DC for this thread; the guard releases it once.
    let screen = ScreenDc(unsafe { GetDC(None) });
    if screen.0.is_invalid() {
        return Err(io::Error::other("GetDC(NULL) failed"));
    }
    // SAFETY: screen.0 is a live DC; the new memory DC is owned by the guard below.
    let memory = MemoryDc(unsafe { CreateCompatibleDC(Some(screen.0)) });
    if memory.0.is_invalid() {
        return Err(io::Error::other("CreateCompatibleDC failed"));
    }
    // SAFETY: screen.0 is a live DC and both dimensions were checked positive and in range.
    let bitmap = Bitmap(unsafe { CreateCompatibleBitmap(screen.0, width_i32, height_i32) });
    if bitmap.0.is_invalid() {
        return Err(io::Error::other("CreateCompatibleBitmap failed"));
    }
    {
        // SAFETY: memory.0 and bitmap.0 are live objects owned by their guards; the Selection
        // guard restores the previous object before either guard drops.
        let previous = unsafe { SelectObject(memory.0, bitmap.0.into()) };
        if previous.is_invalid() {
            return Err(io::Error::other("SelectObject failed"));
        }
        let _selection = Selection {
            dc: memory.0,
            previous,
        };
        // SAFETY: both DCs are live, the destination bitmap is exactly width by height, and the
        // source rectangle is plain screen coordinates; no pointer is retained.
        unsafe {
            BitBlt(
                memory.0,
                0,
                0,
                width_i32,
                height_i32,
                Some(screen.0),
                x,
                y,
                SRCCOPY,
            )
        }
        .map_err(super::io_error)?;
    }

    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width_i32,
            // A negative height asks for top-down rows.
            biHeight: -height_i32,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0_u8; bytes];
    // SAFETY: the bitmap is no longer selected into a DC, pixels holds exactly width * height
    // 32-bit pixels (32-bit rows need no padding), and info describes that layout; the call
    // writes only inside both buffers and retains neither pointer.
    let copied = unsafe {
        GetDIBits(
            screen.0,
            bitmap.0,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &raw mut info,
            DIB_RGB_COLORS,
        )
    };
    if copied != height_i32 {
        return Err(io::Error::other(format!(
            "GetDIBits copied {copied} of {height} rows"
        )));
    }

    let mut total = 0_u64;
    let mut lit = 0_u64;
    for sample_y in (0..rows).step_by(SAMPLE_STEP) {
        for sample_x in (0..row).step_by(SAMPLE_STEP) {
            let offset = (sample_y * row + sample_x) * 4;
            let blue = u32::from(pixels[offset]);
            let green = u32::from(pixels[offset + 1]);
            let red = u32::from(pixels[offset + 2]);
            total += 1;
            if red + green + blue > LIT_THRESHOLD {
                lit += 1;
            }
        }
    }
    Ok(lit as f64 / total as f64)
}
