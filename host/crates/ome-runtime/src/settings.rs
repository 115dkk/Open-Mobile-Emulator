// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use ome_guest_config::{GuestConfig, RawGuestConfig};
use ome_host_check::HardwareLimits;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AdbAccess, CloseAction, GpuMode, SettingsInput};

/// Maximum accepted settings document size.
pub const MAX_SETTINGS_BYTES: usize = 64 * 1024;
/// Current settings document schema version.
pub const SETTINGS_SCHEMA_VERSION: u32 = 2;

/// Persisted product settings after validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Virtual-machine memory in MiB.
    pub memory_mib: u32,
    /// Virtual processor count.
    pub vcpus: u32,
    /// Product GPU mode.
    pub gpu_mode: GpuMode,
    /// Window-close behavior.
    pub close_action: CloseAction,
    /// Whether runtime FPS is exposed when available.
    pub show_fps: bool,
    /// Whether startup may check for an update.
    pub auto_update_check: bool,
    /// adb host exposure policy.
    #[serde(default = "default_adb_access")]
    pub adb_access: AdbAccess,
    /// Whether binding markers are shown by default.
    #[serde(default = "default_binding_overlay")]
    pub binding_overlay_default: bool,
}

const fn default_adb_access() -> AdbAccess {
    AdbAccess::Localhost
}

const fn default_binding_overlay() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            memory_mib: 8192,
            vcpus: 4,
            gpu_mode: GpuMode::Virgl,
            close_action: CloseAction::StopGuest,
            show_fps: false,
            auto_update_check: true,
            adb_access: AdbAccess::Localhost,
            binding_overlay_default: true,
        }
    }
}

impl Settings {
    /// Validates frontend input against host-derived product limits and broad QEMU limits.
    pub fn validate(input: SettingsInput, limits: &HardwareLimits) -> Result<Self, SettingsError> {
        if !(limits.memory_mib_min..=limits.memory_mib_max).contains(&input.memory_mib)
            || !(2..=limits.vcpus_max).contains(&input.vcpus)
        {
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
            adb_bind: Some(
                match input.adb_access {
                    AdbAccess::Localhost => "localhost",
                    AdbAccess::Network => "network",
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
            adb_access: input.adb_access,
            binding_overlay_default: input.binding_overlay_default,
        })
    }

    fn as_input(&self) -> SettingsInput {
        SettingsInput {
            memory_mib: self.memory_mib,
            vcpus: self.vcpus,
            gpu_mode: self.gpu_mode,
            close_action: self.close_action,
            show_fps: self.show_fps,
            auto_update_check: self.auto_update_check,
            adb_access: self.adb_access,
            binding_overlay_default: self.binding_overlay_default,
        }
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

    /// Loads a bounded versioned document using current host limits.
    ///
    /// Schema version 1 is accepted and migrated in memory. Its saved close action is retained;
    /// newly added fields use version-2 defaults. An interrupted staging write fails closed.
    pub fn load(&self, limits: &HardwareLimits) -> Result<Settings, SettingsError> {
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
        if !matches!(document.schema_version, 1 | SETTINGS_SCHEMA_VERSION) {
            return Err(SettingsError::UnsupportedVersion);
        }
        Self::validate_loaded(document.settings, limits)
    }

    fn validate_loaded(
        settings: Settings,
        limits: &HardwareLimits,
    ) -> Result<Settings, SettingsError> {
        Settings::validate(settings.as_input(), limits)
    }

    /// Serializes, syncs, and atomically renames a staging document over the current file.
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
    /// Input is outside host-derived product limits.
    #[error("settings input is invalid")]
    InvalidInput,
    /// JSON parsing or encoding failed.
    #[error("settings JSON is invalid")]
    Json(#[source] serde_json::Error),
    /// Saved schema version is unsupported.
    #[error("settings version is unsupported")]
    UnsupportedVersion,
    /// Saved document exceeds 64 KiB.
    #[error("settings document is too large")]
    TooLarge,
    /// Saved path is not a regular file.
    #[error("settings document path is invalid")]
    InvalidDocument,
    /// A staging file from an interrupted write requires recovery.
    #[error("settings recovery is required")]
    RecoveryRequired,
    /// File-system I/O failed.
    #[error("settings storage failed")]
    Io(#[source] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> HardwareLimits {
        HardwareLimits {
            memory_mib_min: 4096,
            memory_mib_max: 32_768,
            vcpus_max: 16,
        }
    }

    fn input(memory_mib: u32, vcpus: u32) -> SettingsInput {
        SettingsInput {
            memory_mib,
            vcpus,
            gpu_mode: GpuMode::Software,
            close_action: CloseAction::StopGuest,
            show_fps: true,
            auto_update_check: false,
            adb_access: AdbAccess::Localhost,
            binding_overlay_default: true,
        }
    }

    #[test]
    fn validation_uses_host_limits() {
        assert!(Settings::validate(input(32_768, 16), &limits()).is_ok());
        for invalid in [
            input(4095, 4),
            input(33_792, 4),
            input(8192, 1),
            input(8192, 17),
        ] {
            assert!(matches!(
                Settings::validate(invalid, &limits()),
                Err(SettingsError::InvalidInput)
            ));
        }
    }

    #[test]
    fn defaults_use_stop_localhost_and_visible_bindings() {
        let settings = Settings::default();
        assert_eq!(settings.close_action, CloseAction::StopGuest);
        assert_eq!(settings.adb_access, AdbAccess::Localhost);
        assert!(settings.binding_overlay_default);
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
        assert_eq!(store.load(&limits()).expect("load settings"), settings);
        assert!(!directory.path().join("settings.json.staging").exists());
    }

    #[test]
    fn version_one_migrates_new_fields_and_keeps_close_action() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join("settings.json"),
            r#"{
                "schemaVersion": 1,
                "settings": {
                    "memoryMib": 8192,
                    "vcpus": 4,
                    "gpuMode": "virgl",
                    "closeAction": "minimizeToTray",
                    "showFps": false,
                    "autoUpdateCheck": true
                }
            }"#,
        )
        .expect("write version one");
        let settings = SettingsStore::new(directory.path())
            .load(&limits())
            .expect("migrate version one");
        assert_eq!(settings.close_action, CloseAction::MinimizeToTray);
        assert_eq!(settings.adb_access, AdbAccess::Localhost);
        assert!(settings.binding_overlay_default);
    }

    #[test]
    fn load_rejects_oversized_documents() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join("settings.json"),
            vec![b'x'; MAX_SETTINGS_BYTES + 1],
        )
        .expect("write oversized document");
        let result = SettingsStore::new(directory.path()).load(&limits());
        assert!(matches!(result, Err(SettingsError::TooLarge)));
    }
}
