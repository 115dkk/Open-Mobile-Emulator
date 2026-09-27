// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Trusted native desktop operations kept behind a test seam.
#![forbid(unsafe_code)]

use std::path::Path;

/// Opens fixed native destinations and writes trusted snapshot values to the clipboard.
pub trait Desktop: Send + Sync {
    /// Opens one native folder.
    fn open_path(&self, path: &Path) -> Result<(), String>;
    /// Opens one fixed HTTPS URL in the default browser.
    fn open_url(&self, url: &str) -> Result<(), String>;
    /// Replaces clipboard text.
    fn copy_text(&self, text: &str) -> Result<(), String>;
    /// Launches one verified local installer package.
    fn launch_installer(&self, _path: &Path) -> Result<(), String> {
        Err("installer launch is unavailable".to_owned())
    }
}

/// Fail-closed desktop used by tests or shells that do not provide native integration.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableDesktop;

impl Desktop for UnavailableDesktop {
    fn open_path(&self, _path: &Path) -> Result<(), String> {
        Err("desktop integration is unavailable".to_owned())
    }

    fn open_url(&self, _url: &str) -> Result<(), String> {
        Err("desktop integration is unavailable".to_owned())
    }

    fn copy_text(&self, _text: &str) -> Result<(), String> {
        Err("desktop integration is unavailable".to_owned())
    }

    fn launch_installer(&self, _path: &Path) -> Result<(), String> {
        Err("desktop integration is unavailable".to_owned())
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub(crate) struct RecordingDesktop {
    paths: std::sync::Arc<std::sync::Mutex<Vec<std::path::PathBuf>>>,
    urls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    texts: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    installers: std::sync::Arc<std::sync::Mutex<Vec<std::path::PathBuf>>>,
}

#[cfg(test)]
impl RecordingDesktop {
    pub(crate) fn paths(&self) -> Vec<std::path::PathBuf> {
        self.paths.lock().expect("recorded paths lock").clone()
    }

    pub(crate) fn urls(&self) -> Vec<String> {
        self.urls.lock().expect("recorded URLs lock").clone()
    }

    pub(crate) fn texts(&self) -> Vec<String> {
        self.texts.lock().expect("recorded clipboard lock").clone()
    }
}

#[cfg(test)]
impl Desktop for RecordingDesktop {
    fn open_path(&self, path: &Path) -> Result<(), String> {
        self.paths
            .lock()
            .map_err(|_| "recording lock failed".to_owned())?
            .push(path.to_path_buf());
        Ok(())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        self.urls
            .lock()
            .map_err(|_| "recording lock failed".to_owned())?
            .push(url.to_owned());
        Ok(())
    }

    fn copy_text(&self, text: &str) -> Result<(), String> {
        self.texts
            .lock()
            .map_err(|_| "recording lock failed".to_owned())?
            .push(text.to_owned());
        Ok(())
    }

    fn launch_installer(&self, path: &Path) -> Result<(), String> {
        self.installers
            .lock()
            .map_err(|_| "recording lock failed".to_owned())?
            .push(path.to_path_buf());
        Ok(())
    }
}
