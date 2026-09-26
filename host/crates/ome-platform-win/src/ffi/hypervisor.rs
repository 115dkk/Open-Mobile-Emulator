// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Runtime WHPX capability probe that treats a missing DLL as unavailable.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::mem::{size_of, transmute};
use std::os::windows::ffi::OsStrExt;

use windows::Win32::Foundation::{FARPROC, FreeLibrary, HMODULE};
use windows::Win32::System::Hypervisor::{
    WHV_CAPABILITY, WHV_CAPABILITY_CODE, WHvCapabilityCodeHypervisorPresent,
};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::core::{BOOL, HRESULT, PCSTR, PCWSTR};

type WhvGetCapability =
    unsafe extern "system" fn(WHV_CAPABILITY_CODE, *mut c_void, u32, *mut u32) -> HRESULT;

pub(crate) fn whpx_available() -> io::Result<bool> {
    let library_name: Vec<u16> = std::ffi::OsStr::new("WinHvPlatform.dll")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: library_name is an aligned, NUL-terminated u16 buffer live for
    // the call. The System32-only search prevents application-directory DLL
    // substitution. One module reference transfers to us; no elevated privilege
    // or caller-owned buffer is involved. Any load failure means unavailable.
    let module = match unsafe {
        LoadLibraryExW(
            PCWSTR(library_name.as_ptr()),
            None,
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    } {
        Ok(module) => Module(module),
        Err(_) => return Ok(false),
    };
    // SAFETY: module is a live owned reference and the ASCII symbol includes a
    // NUL terminator. GetProcAddress retains no pointer and needs no privilege.
    let symbol: FARPROC =
        unsafe { GetProcAddress(module.0, PCSTR(c"WHvGetCapability".as_ptr().cast())) };
    let Some(symbol) = symbol else {
        return Ok(false);
    };
    // SAFETY: the symbol was resolved by its exact exported ABI name from
    // WinHvPlatform.dll. The target signature matches the published API; the
    // module stays loaded for the call, and no data pointer is transmuted.
    let get_capability: WhvGetCapability = unsafe { transmute(symbol) };
    let mut capability = WHV_CAPABILITY::default();
    let mut written = 0_u32;
    // SAFETY: capability is aligned and writable for exactly its structure
    // size; written is an aligned four-byte out value. The function pointer
    // has the verified system ABI, module lifetime covers the call, and this
    // read-only capability query needs no elevated privilege.
    let result = unsafe {
        get_capability(
            WHvCapabilityCodeHypervisorPresent,
            (&raw mut capability).cast(),
            size_of::<WHV_CAPABILITY>() as u32,
            &raw mut written,
        )
    };
    result.ok().map_err(super::io_error)?;
    if written < size_of::<BOOL>() as u32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "WHvGetCapability returned a truncated HypervisorPresent value",
        ));
    }
    // SAFETY: the requested capability code selects the HypervisorPresent
    // union member, and the API reported enough initialized bytes for BOOL.
    // Reading this Copy field stays within bounds and needs no privilege.
    Ok(unsafe { capability.HypervisorPresent }.as_bool())
}

struct Module(HMODULE);

impl Drop for Module {
    fn drop(&mut self) {
        // SAFETY: this module reference came from one successful LoadLibraryW
        // call and has not been freed; Drop releases it exactly once. No pointer,
        // bounds, alignment, or privilege assumption applies.
        let _ = unsafe { FreeLibrary(self.0) };
    }
}
