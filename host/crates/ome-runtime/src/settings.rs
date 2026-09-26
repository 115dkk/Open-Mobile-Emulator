// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use ome_guest_config::{GuestConfig, RawGuestConfig};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CloseAction, GpuMode, SettingsInput};

/// Maximum accepted settings document size.
pub const MAX_SETTINGS_BYTES: usize = 64 * 1024;
/// Current settings document schema version.
pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

/// Persisted product settings after validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Guest memory offered by the UI, constrained to 4096..=16384 MiB.
    pub memory_mib: u32,
    /// Guest virtual processors offered by the UI, constrained to 2..=8.
    pub vcpus: u32,
    /// Product GPU mode.
    pub gpu_mode: GpuMode,
    /// Window-close behavior.
    pub close_action: CloseAction,
    /// Whether the runtime should expose guest FPS when available.
    pub show_fps: bool,
    /// Whether startup may check for an update.
    pub auto_update_check: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            memory_mib: 8192,
            vcpus: 4,
            gpu_mode: GpuMode::Virgl,
            close_action: CloseAction::MinimizeToTray,
            show_fps: false,
            auto_update_check: true,
        }
    }
}

impl Settings {
    /// Validates frontend input using both the UI range and [`GuestConfig::validate`].
    ///
    /// No settings value exists until the guest configuration validator accepts the corresponding
    /// memory, processor, and GPU fields.
    pub fn validate(input: SettingsInput) -> Result<Self, SettingsError> {
        if !(4096..=16_384).contains(&input.memory_mib) || !(2..=8).contains(&input.vcpus) {
            return Err(SettingsError::InvalidInput);
        }
        GuestConfig::validate(RawGuestConfig {
            memory_mib: Some(i64::from(input.memory_mib)),
            vcpus: Some(i64::from(input.vcpus)),
            gpu: Some(
                match input.gpu_mode {
                    GpuMode::Virgl => "virgl",
                    GpuMode::Software => "std",
                }
                .to_owned(),
            ),
            ..RawGuestConfig::default()
        })
        .map_err(|_| SettingsError::InvalidInput)?;
        Ok(Self {
            memory_mib: input.memory_mib,
            vcpus: input.vcpus,
            gpu_mode: input.gpu_mode,
            close_action: input.close_action,
            show_fps: input.show_fps,
            auto_update_check: input.auto_update_check,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SettingsDocument {
    schema_version: u32,
    settings: Settings,
}

/// Bounded atomic settings store rooted at one fixed file.
#[derive(Clone, Debug)]
pub struct SettingsStore {
    path: PathBuf,
    staging_path: PathBuf,
}

impl SettingsStore {
    /// Creates a store descriptor without opening or creating files.
    pub fn new(home: &Path) -> Self {
        Self {
            path: home.join("settings.json"),
            staging_path: home.join("settings.json.staging"),
        }
    }

    /// Loads a bounded versioned document, or default settings when no file exists.
    ///
    /// An existing staging file is treated as an interrupted write and not promoted implicitly.
    pub fn load(&self) -> Result<Settings, SettingsError> {
        if self.staging_path.exists() {
            return Err(SettingsError::RecoveryRequired);
        }
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => return Err(SettingsError::InvalidDocument),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(error) => return Err(SettingsError::Io(error)),
        };
        if metadata.len() > MAX_SETTINGS_BYTES as u64 {
            return Err(SettingsError::TooLarge);
        }
        let file = File::open(&self.path).map_err(SettingsError::Io)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(MAX_SETTINGS_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(SettingsError::Io)?;
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::TooLarge);
        }
        let document: SettingsDocument =
            serde_json::from_slice(&bytes).map_err(SettingsError::Json)?;
        if document.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(SettingsError::UnsupportedVersion);
        }
        Settings::validate(SettingsInput {
            memory_mib: document.settings.memory_mib,
            vcpus: document.settings.vcpus,
            gpu_mode: document.settings.gpu_mode,
            close_action: document.settings.close_action,
            show_fps: document.settings.show_fps,
            auto_update_check: document.settings.auto_update_check,
        })
    }

    /// Serializes, syncs, and atomically renames a staging document over the current file.
    ///
    /// The previous document is untouched until the complete staging file has been synced. An
    /// existing staging file is retained for explicit recovery and causes an error. A staging file
    /// created by this call is removed when writing, syncing, or renaming fails.
    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let bytes = serde_json::to_vec_pretty(&SettingsDocument {
            schema_version: SETTINGS_SCHEMA_VERSION,
            settings: settings.clone(),
        })
        .map_err(SettingsError::Json)?;
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::TooLarge);
        }
        let mut staging = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.staging_path)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    SettingsError::RecoveryRequired
                } else {
                    SettingsError::Io(error)
                }
            })?;
        let result = (|| {
            staging.write_all(&bytes).map_err(SettingsError::Io)?;
            staging.sync_all().map_err(SettingsError::Io)?;
            drop(staging);
            fs::rename(&self.staging_path, &self.path).map_err(SettingsError::Io)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&self.staging_path);
        }
        result
    }
}

/// Settings validation and persistence errors.
#[derive(Debug, Error)]
pub enum SettingsError {
    /// UI or guest configuration validation rejected the input.
    #[error("settings input is invalid")]
    InvalidInput,
    /// JSON parsing or encoding failed.
    #[error("settings JSON is invalid")]
    Json(#[source] serde_json::Error),
    /// The saved document uses an unsupported schema version.
    #[error("settings version is unsupported")]
    UnsupportedVersion,
    /// The saved document exceeds 64 KiB.
    #[error("settings document is too large")]
    TooLarge,
    /// The saved path is not a regular file.
    #[error("settings document path is invalid")]
    InvalidDocument,
    /// A staging file from an interrupted write requires explicit recovery.
    #[error("settings recovery is required")]
    RecoveryRequired,
    /// File-system I/O failed.
    #[error("settings storage failed")]
    Io(#[source] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_enforces_ui_ranges_before_guest_ranges() {
        let valid = Settings::validate(SettingsInput {
            memory_mib: 16_384,
            vcpus: 8,
            gpu_mode: GpuMode::Software,
            close_action: CloseAction::StopGuest,
            show_fps: true,
            auto_update_check: false,
        });
        assert!(valid.is_ok());
        let invalid = Settings::validate(SettingsInput {
            memory_mib: 4095,
            vcpus: 1,
            gpu_mode: GpuMode::Virgl,
            close_action: CloseAction::MinimizeToTray,
            show_fps: false,
            auto_update_check: true,
        });
        assert!(matches!(invalid, Err(SettingsError::InvalidInput)));
    }

    #[test]
    fn store_round_trips_through_staging_rename() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = SettingsStore::new(directory.path());
        let settings = Settings {
            memory_mib: 12_288,
            vcpus: 6,
            ..Settings::default()
        };
        store.save(&settings).expect("save settings");
        assert_eq!(store.load().expect("load settings"), settings);
        assert!(!directory.path().join("settings.json.staging").exists());
    }

    #[test]
    fn load_rejects_oversized_documents() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join("settings.json"),
            vec![b'x'; MAX_SETTINGS_BYTES + 1],
        )
        .expect("write oversized document");
        let result = SettingsStore::new(directory.path()).load();
        assert!(matches!(result, Err(SettingsError::TooLarge)));
    }
}
