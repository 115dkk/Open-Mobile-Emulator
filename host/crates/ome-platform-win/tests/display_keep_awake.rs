// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! The display keep-awake guard acquires and releases on its own thread.
//!
//! This lives in its own test binary: the crate's `owned_handle_closes_exactly_once` unit test
//! closes a stale handle value on purpose, and a thread created in the same process at that moment
//! can receive that value.
#![forbid(unsafe_code)]
#![cfg(windows)]

use ome_platform_win::DisplayKeepAwake;

#[test]
fn display_keep_awake_holds_and_releases_on_its_own_thread() {
    let first = DisplayKeepAwake::acquire().expect("acquire display requirement");
    let second = DisplayKeepAwake::acquire().expect("a second guard is independent");
    drop(first);
    drop(second);
}
