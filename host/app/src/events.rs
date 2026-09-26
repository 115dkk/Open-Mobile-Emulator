// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
use ome_runtime::{AppSnapshot, EVENT_SNAPSHOT};
use tauri::Emitter;

pub(crate) fn snapshot(app: &tauri::AppHandle, snapshot: &AppSnapshot) {
    // A delivery error does not undo a completed native operation. Its caller
    // also receives the snapshot as the command result.
    if let Err(error) = app.emit(EVENT_SNAPSHOT, snapshot) {
        eprintln!("snapshot event delivery failed: {error}");
    }
}
