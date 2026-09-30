// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Read-only audio device observation used to choose the guest's audio backend.
#![allow(unsafe_code)]

use windows::Win32::Media::Audio::waveOutGetNumDevs;

/// Counts the waveform-audio output devices Windows currently exposes.
///
/// A host without any output device (a server, a stripped-down VM, a PC whose audio is disabled)
/// cannot open DirectSound, and QEMU treats a failed `-audiodev dsound` as fatal
/// (`docs/evidence/M2/dod-ci.md`, run 21).
pub(crate) fn output_devices() -> u32 {
    // SAFETY: the call takes no arguments, touches no caller memory and only reads a system count.
    unsafe { waveOutGetNumDevs() }
}
