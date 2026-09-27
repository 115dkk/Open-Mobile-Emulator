// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
#![forbid(unsafe_code)]

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use ome_runtime::Desktop;

/// Windows desktop operations used only with runtime-selected paths and URLs.
#[derive(Default)]
pub(crate) struct WindowsDesktop {
    clipboard: Mutex<Option<arboard::Clipboard>>,
}

impl std::fmt::Debug for WindowsDesktop {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WindowsDesktop")
            .finish_non_exhaustive()
    }
}

impl WindowsDesktop {
    fn open(target: &std::ffi::OsStr) -> Result<(), String> {
        Command::new("explorer.exe")
            .arg(target)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

impl Desktop for WindowsDesktop {
    fn open_path(&self, path: &Path) -> Result<(), String> {
        Self::open(path.as_os_str())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        if !url.starts_with("https://") {
            return Err("only fixed HTTPS URLs may be opened".to_owned());
        }
        Self::open(url.as_ref())
    }

    fn copy_text(&self, text: &str) -> Result<(), String> {
        let mut guard = self
            .clipboard
            .lock()
            .map_err(|_| "clipboard lock failed".to_owned())?;
        if guard.is_none() {
            *guard = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        guard
            .as_mut()
            .expect("clipboard initialized above")
            .set_text(text)
            .map_err(|error| error.to_string())
    }

    fn launch_installer(&self, path: &Path) -> Result<(), String> {
        if !path.is_file() {
            return Err("verified installer is missing".to_owned());
        }
        Command::new(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}
