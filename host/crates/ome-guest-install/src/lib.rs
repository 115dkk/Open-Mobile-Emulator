// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Host-side parts of the unattended operating-system install (ADR-0010): reading the boot files out of the installer ISO, building the helper initrd, composing kernel command lines and parsing the helper's serial-log progress.
#![forbid(unsafe_code)]

pub mod cpio;
pub mod helper;
pub mod iso9660;
pub mod progress;
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support;
