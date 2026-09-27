// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Read-only Windows host observations used by the first-run readiness probe.
#![allow(unsafe_code)]

use std::fs;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExW,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    ALL_PROCESSOR_GROUPS, GetActiveProcessorCount, IsProcessorFeaturePresent,
    PF_VIRT_FIRMWARE_ENABLED,
};
use windows::core::PCWSTR;

const REBOOT_PENDING_KEYS: [&str; 2] = [
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending",
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired",
];

/// Reports whether Windows says virtualization is enabled in firmware.
pub(crate) fn virtualization_firmware_enabled() -> bool {
    // SAFETY: PF_VIRT_FIRMWARE_ENABLED is a documented feature identifier. The call
    // takes no pointers, retains no state, and only reads an operating-system fact.
    unsafe { IsProcessorFeaturePresent(PF_VIRT_FIRMWARE_ENABLED).as_bool() }
}

/// Reports whether CPUID identifies a hypervisor above this x86_64 process.
#[cfg(target_arch = "x86_64")]
pub(crate) fn hypervisor_present() -> bool {
    let leaf = core::arch::x86_64::__cpuid(1);
    leaf.ecx & (1 << 31) != 0
}

/// Returns false on architectures where the x86_64 CPUID contract does not apply.
#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn hypervisor_present() -> bool {
    false
}

/// Reports whether either Windows servicing stack records a pending reboot.
pub(crate) fn reboot_pending() -> io::Result<bool> {
    for path in REBOOT_PENDING_KEYS {
        if registry_key_exists(path)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn registry_key_exists(path: &str) -> io::Result<bool> {
    let path = nul_terminated(path.as_ref())?;
    let mut key = HKEY(std::ptr::null_mut());
    // SAFETY: path is an aligned, NUL-terminated UTF-16 buffer that remains live
    // for the call. key is a valid writable out parameter. HKEY_LOCAL_MACHINE is
    // predefined, KEY_READ asks only for read access, and no key is created.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(path.as_ptr()),
            None,
            KEY_READ,
            &raw mut key,
        )
    };
    if status == ERROR_SUCCESS {
        let key = RegistryKey(key);
        drop(key);
        Ok(true)
    } else {
        classify_registry_open(status)
    }
}

fn classify_registry_open(status: WIN32_ERROR) -> io::Result<bool> {
    if status == ERROR_FILE_NOT_FOUND {
        Ok(false)
    } else {
        Err(io::Error::from_raw_os_error(status.0 as i32))
    }
}

/// Returns free bytes available to the caller on the path's volume.
pub(crate) fn free_disk_bytes(path: &Path) -> io::Result<u64> {
    let directory = nearest_existing_directory(path)?;
    let directory = nul_terminated(directory.as_os_str())?;
    let mut available = 0_u64;
    // SAFETY: directory is an aligned, NUL-terminated UTF-16 buffer live for the
    // call and names an existing directory. available is an aligned writable u64;
    // the two unused output pointers are null through None.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(directory.as_ptr()),
            Some(&raw mut available),
            None,
            None,
        )
    }
    .map_err(super::io_error)?;
    Ok(available)
}

fn nearest_existing_directory(path: &Path) -> io::Result<PathBuf> {
    let mut candidate = if path.as_os_str().is_empty() {
        std::env::current_dir()?
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    loop {
        match fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_dir() => return Ok(candidate),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let Some(parent) = candidate.parent() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "disk probe path has no existing directory ancestor",
            ));
        };
        candidate = parent.to_path_buf();
    }
}

/// Returns total physical memory reported by Windows.
pub(crate) fn total_memory_bytes() -> io::Result<u64> {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: status is an initialized, aligned writable MEMORYSTATUSEX whose
    // dwLength names its full size. The call writes only within that structure.
    unsafe { GlobalMemoryStatusEx(&raw mut status) }.map_err(super::io_error)?;
    Ok(status.ullTotalPhys)
}

/// Returns active processors across every Windows processor group.
pub(crate) fn logical_processors() -> u32 {
    // SAFETY: ALL_PROCESSOR_GROUPS is the documented sentinel. The call takes no
    // pointers and reads processor topology without changing host state.
    let count = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
    if count != 0 {
        return count;
    }
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .ok()
        .and_then(|count| u32::try_from(count).ok())
        .unwrap_or(1)
}

fn nul_terminated(value: &std::ffi::OsStr) -> io::Result<Vec<u16>> {
    let mut units: Vec<u16> = value.encode_wide().collect();
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows string contains an interior NUL",
        ));
    }
    units.push(0);
    Ok(units)
}

struct RegistryKey(HKEY);

impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: this handle came from one successful RegOpenKeyExW call and
        // remains owned by this value. Drop closes it exactly once.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::ERROR_ACCESS_DENIED;

    #[test]
    fn missing_registry_key_is_absence_but_other_errors_are_reported() {
        assert!(!classify_registry_open(ERROR_FILE_NOT_FOUND).expect("missing key"));
        let error = classify_registry_open(ERROR_ACCESS_DENIED).expect_err("access denied");
        assert_eq!(error.raw_os_error(), Some(ERROR_ACCESS_DENIED.0 as i32));
    }

    #[test]
    fn nearest_existing_directory_walks_up_from_a_missing_child() {
        let root = std::env::current_dir().expect("current directory");
        let missing = root.join("ome-host-probe-missing/child");
        assert_eq!(
            nearest_existing_directory(&missing).expect("existing ancestor"),
            root
        );
    }
}
