// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Snapshot projection, settings persistence and command dispatch. The Tauri shell calls this
//! crate and nothing else (docs/adr/0001-rust-core-thin-shell.md).
#![forbid(unsafe_code)]

pub mod contract;
pub mod home;
pub mod issues;
pub mod runtime;
pub mod settings;

pub use contract::*;
pub use home::{HomeError, OmeHome};
pub use ome_host_check::TableProbe;
#[cfg(windows)]
pub use ome_host_check::WindowsProbe;
pub use runtime::{AppRuntime, RuntimeDeps, display_presets, load_input_directory};
pub use settings::{
    MAX_SETTINGS_BYTES, SETTINGS_SCHEMA_VERSION, Settings, SettingsError, SettingsStore,
};
