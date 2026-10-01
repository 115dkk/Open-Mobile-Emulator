// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Versioned per-guest metadata stored beside each persistent virtual disk.
#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use ome_guest_image::{DeviceId, ProbeItem, ProbeOutcome, ProbeState};
use ome_wizard::WizardState;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::home_state;

/// Current `guest.json` schema version.
pub const GUEST_SCHEMA_VERSION: u32 = 1;
/// Maximum accepted metadata size.
pub const MAX_GUEST_BYTES: usize = 256 * 1024;
const ADOPTED_IMAGE_ID: &str = "bliss-16.9.7-android-13";
const ADOPTED_ANDROID_VERSION: &str = "13";
const ADOPTED_API_LEVEL: u32 = 33;

/// Stable persisted state for one installed operating system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestRecord {
    /// Safe directory and runtime identifier.
    pub id: String,
    /// Source image profile identifier.
    pub image_id: String,
    /// User-facing Android version.
    pub android_version: String,
    /// Android API level used to select a generation adapter.
    pub api_level: u32,
    /// Current virtual-disk file size.
    pub disk_bytes: u64,
    /// Creation time in RFC 3339 when known.
    pub created_at: Option<String>,
    /// Most recent successful process start.
    pub last_started_at: Option<String>,
    /// Stored first-boot capability result and values.
    pub capabilities: StoredCapabilities,
    /// GSF Android ID in both accepted forms.
    pub device_id: Option<StoredDeviceId>,
    /// Last time the registration page was opened for this operating system.
    pub registration_opened_at: Option<String>,
    /// Whether applications may request root, or unknown before probing.
    pub root_enabled: Option<bool>,
    /// How the installed guest boots; absent for guests the interactive installer made.
    pub boot: Option<StoredBoot>,
    /// Whether the one-time migration from the interactive install layout was attempted.
    pub direct_boot_migration_attempted: bool,
    /// Outcome of the unattended install; absent for guests made before ADR-0010.
    pub install: Option<StoredInstall>,
}

impl GuestRecord {
    /// Applies one complete capability probe result.
    pub fn apply_probe(&mut self, outcome: &ProbeOutcome, probed_at: String) {
        self.capabilities = StoredCapabilities {
            probed_at: Some(probed_at),
            items: outcome
                .items
                .iter()
                .map(|(item, state)| StoredCapabilityItem {
                    id: StoredProbeItem::from(*item),
                    state: StoredProbeState::from(*state),
                })
                .collect(),
            native_bridge: outcome.native_bridge.clone(),
            media_volume: outcome.media_volume,
            google_accounts: outcome.google_accounts,
            foreground_package: outcome.foreground.clone(),
            display: outcome.display.map(|display| StoredDisplay {
                width: display.width,
                height: display.height,
                density_dpi: display.density_dpi,
            }),
            packages: outcome
                .packages
                .iter()
                .map(|package| StoredPackage {
                    package: package.package.clone(),
                    version_code: package.version_code,
                })
                .collect(),
        };
        self.device_id = outcome.device_id.as_ref().map(StoredDeviceId::from);
        self.root_enabled = outcome.root_enabled;
    }
}

/// Direct-boot metadata for an installed guest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredBoot {
    /// Direct kernel boot is the only stored method.
    pub method: StoredBootMethod,
    /// Directory name passed to the guest initrd as `SRC`.
    pub src: String,
    /// Whether the product migrated an interactive installer guest to direct boot.
    #[serde(default)]
    pub migrated_from_disk: bool,
    /// Whether the migrated guest has completed one direct boot.
    #[serde(default)]
    pub direct_boot_verified: bool,
}

/// Supported persisted boot method.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoredBootMethod {
    /// Start the extracted kernel and initrd without GRUB.
    Direct,
}

/// Persisted unattended-install outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredInstall {
    /// Latest install state.
    pub state: StoredInstallState,
    /// Completion time in RFC 3339, only for a successful install.
    pub finished_at: Option<String>,
    /// User-visible failure reason, only for a failed install.
    pub failure: Option<String>,
}

/// Persisted unattended-install state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoredInstallState {
    /// The hidden helper VM is running.
    Running,
    /// The helper completed and direct boot is ready.
    Installed,
    /// The helper failed, stopped early, or was interrupted.
    Failed,
}

/// Persisted capability probe report and parsed values.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredCapabilities {
    /// Probe time in RFC 3339.
    pub probed_at: Option<String>,
    /// Stable item list.
    pub items: Vec<StoredCapabilityItem>,
    /// Native bridge library when configured.
    pub native_bridge: Option<String>,
    /// Media stream volume index.
    pub media_volume: Option<u32>,
    /// Signed-in Google account count.
    pub google_accounts: Option<u32>,
    /// Foreground package at probe time.
    pub foreground_package: Option<String>,
    /// Display dimensions and density.
    pub display: Option<StoredDisplay>,
    /// Third-party packages observed by the probe.
    pub packages: Vec<StoredPackage>,
}

/// Persisted capability item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredCapabilityItem {
    /// Capability identifier.
    pub id: StoredProbeItem,
    /// Availability result.
    pub state: StoredProbeState,
}

/// Stable serialized probe identifiers, independent from debug names in the image crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoredProbeItem {
    BootMarker,
    AppList,
    DisplaySize,
    MediaVolume,
    DeviceId,
    Screenshot,
    ForegroundApp,
    Multitouch,
    NativeBridge,
    Root,
}

impl From<ProbeItem> for StoredProbeItem {
    fn from(value: ProbeItem) -> Self {
        match value {
            ProbeItem::BootMarker => Self::BootMarker,
            ProbeItem::AppList => Self::AppList,
            ProbeItem::DisplaySize => Self::DisplaySize,
            ProbeItem::MediaVolume => Self::MediaVolume,
            ProbeItem::DeviceId => Self::DeviceId,
            ProbeItem::Screenshot => Self::Screenshot,
            ProbeItem::ForegroundApp => Self::ForegroundApp,
            ProbeItem::Multitouch => Self::Multitouch,
            ProbeItem::NativeBridge => Self::NativeBridge,
            ProbeItem::Root => Self::Root,
        }
    }
}

/// Stable serialized probe states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StoredProbeState {
    Available,
    Unavailable,
    Unknown,
}

impl From<ProbeState> for StoredProbeState {
    fn from(value: ProbeState) -> Self {
        match value {
            ProbeState::Available => Self::Available,
            ProbeState::Unavailable => Self::Unavailable,
            ProbeState::Unknown => Self::Unknown,
        }
    }
}

/// Persisted GSF Android ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredDeviceId {
    pub decimal: String,
    pub hex: String,
}

impl From<&DeviceId> for StoredDeviceId {
    fn from(value: &DeviceId) -> Self {
        Self {
            decimal: value.decimal.clone(),
            hex: value.hex.clone(),
        }
    }
}

/// Persisted display value from a capability probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredDisplay {
    pub width: u32,
    pub height: u32,
    pub density_dpi: u32,
}

/// Persisted package value from a capability probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredPackage {
    pub package: String,
    pub version_code: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuestDocument {
    version: u32,
    id: String,
    image_id: String,
    android_version: String,
    api_level: u32,
    disk_bytes: u64,
    created_at: Option<String>,
    last_started_at: Option<String>,
    capabilities: StoredCapabilities,
    device_id: Option<StoredDeviceId>,
    registration_opened_at: Option<String>,
    root_enabled: Option<bool>,
    /// Absent in guest documents written before ADR-0010.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boot: Option<StoredBoot>,
    /// Absent in guest documents written before legacy guests could be migrated.
    #[serde(default, skip_serializing_if = "is_false")]
    direct_boot_migration_attempted: bool,
    /// Absent in guest documents written before ADR-0010.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    install: Option<StoredInstall>,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl From<&GuestRecord> for GuestDocument {
    fn from(record: &GuestRecord) -> Self {
        Self {
            version: GUEST_SCHEMA_VERSION,
            id: record.id.clone(),
            image_id: record.image_id.clone(),
            android_version: record.android_version.clone(),
            api_level: record.api_level,
            disk_bytes: record.disk_bytes,
            created_at: record.created_at.clone(),
            last_started_at: record.last_started_at.clone(),
            capabilities: record.capabilities.clone(),
            device_id: record.device_id.clone(),
            registration_opened_at: record.registration_opened_at.clone(),
            root_enabled: record.root_enabled,
            boot: record.boot.clone(),
            direct_boot_migration_attempted: record.direct_boot_migration_attempted,
            install: record.install.clone(),
        }
    }
}

impl From<GuestDocument> for GuestRecord {
    fn from(document: GuestDocument) -> Self {
        Self {
            id: document.id,
            image_id: document.image_id,
            android_version: document.android_version,
            api_level: document.api_level,
            disk_bytes: document.disk_bytes,
            created_at: document.created_at,
            last_started_at: document.last_started_at,
            capabilities: document.capabilities,
            device_id: document.device_id,
            registration_opened_at: document.registration_opened_at,
            root_enabled: document.root_enabled,
            boot: document.boot,
            direct_boot_migration_attempted: document.direct_boot_migration_attempted,
            install: document.install,
        }
    }
}

/// Atomic metadata store under `<home>/vm`.
#[derive(Clone, Debug)]
pub struct GuestStore {
    root: PathBuf,
}

impl GuestStore {
    /// Creates a store descriptor. The caller owns creation of the fixed `vm` directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns one validated guest directory.
    pub fn guest_dir(&self, id: &str) -> Result<PathBuf, GuestStoreError> {
        validate_id(id)?;
        Ok(self.root.join(id))
    }

    /// Lists metadata documents and adopts disk-only directories in memory.
    pub fn load_all(&self) -> Result<Vec<GuestRecord>, GuestStoreError> {
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(GuestStoreError::Io)? {
            let entry = entry.map_err(GuestStoreError::Io)?;
            if !entry.file_type().map_err(GuestStoreError::Io)?.is_dir() {
                continue;
            }
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                return Err(GuestStoreError::InvalidId);
            };
            validate_id(&id)?;
            let directory = entry.path();
            let disk = directory.join("disk.qcow2");
            let metadata = directory.join("guest.json");
            if metadata.is_file() {
                let mut record = load_document(&metadata)?;
                if record.id != id {
                    return Err(GuestStoreError::InvalidId);
                }
                if disk.is_file() && record.disk_bytes == 0 {
                    record.disk_bytes = disk.metadata().map_err(GuestStoreError::Io)?.len();
                }
                records.push(record);
            } else if disk.is_file() {
                records.push(GuestRecord {
                    id,
                    image_id: ADOPTED_IMAGE_ID.to_owned(),
                    android_version: ADOPTED_ANDROID_VERSION.to_owned(),
                    api_level: ADOPTED_API_LEVEL,
                    disk_bytes: disk.metadata().map_err(GuestStoreError::Io)?.len(),
                    created_at: None,
                    last_started_at: None,
                    capabilities: StoredCapabilities::default(),
                    device_id: None,
                    registration_opened_at: None,
                    root_enabled: None,
                    boot: None,
                    direct_boot_migration_attempted: false,
                    install: None,
                });
            }
        }
        records.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(records)
    }

    /// Writes one metadata document through a staging file and atomic rename.
    pub fn save(&self, record: &GuestRecord) -> Result<(), GuestStoreError> {
        validate_id(&record.id)?;
        let directory = self.guest_dir(&record.id)?;
        fs::create_dir_all(&directory).map_err(GuestStoreError::Io)?;
        let path = directory.join("guest.json");
        let staging = directory.join("guest.json.staging");
        let bytes = serde_json::to_vec_pretty(&GuestDocument::from(record))
            .map_err(GuestStoreError::Json)?;
        if bytes.len() > MAX_GUEST_BYTES {
            return Err(GuestStoreError::TooLarge);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(GuestStoreError::Io)?;
        let result = (|| {
            file.write_all(&bytes).map_err(GuestStoreError::Io)?;
            file.sync_all().map_err(GuestStoreError::Io)?;
            drop(file);
            fs::rename(&staging, &path).map_err(GuestStoreError::Io)
        })();
        if result.is_err() {
            let _ = fs::remove_file(staging);
        }
        result
    }

    /// Loads the selected virtual-machine identifier from `<home>/state.json`.
    pub fn load_active(&self) -> Result<Option<String>, GuestStoreError> {
        let home = self.root.parent().ok_or(GuestStoreError::InvalidId)?;
        Ok(home_state::load(home)?.active_guest)
    }

    /// Atomically persists the selected virtual-machine identifier.
    pub fn save_active(&self, active_guest: Option<&str>) -> Result<(), GuestStoreError> {
        if let Some(id) = active_guest {
            validate_id(id)?;
        }
        let home = self.root.parent().ok_or(GuestStoreError::InvalidId)?;
        let mut state = home_state::load(home)?;
        state.active_guest = active_guest.map(str::to_owned);
        home_state::save(home, &state)
    }

    /// Loads first-run wizard progress from `<home>/state.json`.
    pub fn load_wizard(&self) -> Result<Option<WizardState>, GuestStoreError> {
        let home = self.root.parent().ok_or(GuestStoreError::InvalidId)?;
        Ok(home_state::load(home)?.wizard)
    }

    /// Atomically persists first-run wizard progress while preserving guest selection.
    pub fn save_wizard(&self, wizard: &WizardState) -> Result<(), GuestStoreError> {
        let home = self.root.parent().ok_or(GuestStoreError::InvalidId)?;
        let mut state = home_state::load(home)?;
        state.wizard = Some(wizard.clone());
        home_state::save(home, &state)
    }

    /// Removes one complete virtual-machine directory after runtime state validation.
    pub fn delete(&self, id: &str) -> Result<(), GuestStoreError> {
        let directory = self.guest_dir(id)?;
        if directory.exists() {
            fs::remove_dir_all(directory).map_err(GuestStoreError::Io)?;
        }
        Ok(())
    }
}

fn load_document(path: &Path) -> Result<GuestRecord, GuestStoreError> {
    let metadata = path.metadata().map_err(GuestStoreError::Io)?;
    if !metadata.is_file() || metadata.len() > MAX_GUEST_BYTES as u64 {
        return Err(GuestStoreError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .map_err(GuestStoreError::Io)?
        .take(MAX_GUEST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(GuestStoreError::Io)?;
    if bytes.len() > MAX_GUEST_BYTES {
        return Err(GuestStoreError::TooLarge);
    }
    let document: GuestDocument = serde_json::from_slice(&bytes).map_err(GuestStoreError::Json)?;
    if document.version != GUEST_SCHEMA_VERSION {
        return Err(GuestStoreError::UnsupportedVersion);
    }
    validate_id(&document.id)?;
    let mut record: GuestRecord = document.into();
    if record
        .install
        .as_ref()
        .is_some_and(|install| install.state == StoredInstallState::Running)
    {
        record.install = Some(StoredInstall {
            state: StoredInstallState::Failed,
            finished_at: None,
            failure: Some("설치가 끝나기 전에 앱이 종료되었습니다.".to_owned()),
        });
    }
    Ok(record)
}

pub(crate) fn validate_id(id: &str) -> Result<(), GuestStoreError> {
    let path = Path::new(id);
    let one_normal_component = {
        let mut components = path.components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    };
    let valid_text = !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if one_normal_component && valid_text {
        Ok(())
    } else {
        Err(GuestStoreError::InvalidId)
    }
}

/// Guest metadata storage failure.
#[derive(Debug, Error)]
pub enum GuestStoreError {
    #[error("guest identifier is invalid")]
    InvalidId,
    #[error("guest metadata version is unsupported")]
    UnsupportedVersion,
    #[error("guest metadata is too large")]
    TooLarge,
    #[error("guest metadata JSON is invalid")]
    Json(#[source] serde_json::Error),
    #[error("guest metadata storage failed")]
    Io(#[source] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adopts_disk_only_directory_and_persists_flat_document_on_save() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("vm");
        let guest = root.join("default");
        fs::create_dir_all(&guest).expect("guest directory");
        fs::write(guest.join("disk.qcow2"), [0_u8; 11]).expect("disk");
        let store = GuestStore::new(&root);
        let record = store.load_all().expect("load").remove(0);
        assert_eq!(record.id, "default");
        assert_eq!(record.disk_bytes, 11);
        assert_eq!(record.api_level, 33);
        assert!(!guest.join("guest.json").exists());
        store.save(&record).expect("save");
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(guest.join("guest.json")).expect("metadata bytes"))
                .expect("metadata JSON");
        assert_eq!(document["version"], 1);
        assert_eq!(document["id"], "default");
        assert!(document.get("guest").is_none());
        assert_eq!(store.load_all().expect("reload"), [record]);
    }

    #[test]
    fn legacy_document_without_boot_or_install_keys_loads() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("vm");
        let guest = root.join("default");
        fs::create_dir_all(&guest).expect("guest directory");
        fs::write(guest.join("disk.qcow2"), [0_u8; 1]).expect("disk");
        fs::write(
            guest.join("guest.json"),
            r#"{
              "version":1,"id":"default","imageId":"bliss-16.9.7-android-13",
              "androidVersion":"13","apiLevel":33,"diskBytes":1,"createdAt":null,
              "lastStartedAt":null,"capabilities":{"probedAt":null,"items":[],
              "nativeBridge":null,"mediaVolume":null,"googleAccounts":null,
              "foregroundPackage":null,"display":null,"packages":[]},"deviceId":null,
              "registrationOpenedAt":null,"rootEnabled":null
            }"#,
        )
        .expect("legacy metadata");
        let record = GuestStore::new(root).load_all().expect("load").remove(0);
        assert_eq!(record.boot, None);
        assert_eq!(record.install, None);
    }

    #[test]
    fn direct_boot_document_without_migration_flags_loads_with_false_defaults() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("vm");
        let guest = root.join("default");
        fs::create_dir_all(&guest).expect("guest directory");
        fs::write(guest.join("disk.qcow2"), [0_u8; 1]).expect("disk");
        fs::write(
            guest.join("guest.json"),
            r#"{
              "version":1,"id":"default","imageId":"bliss-16.9.7-android-13",
              "androidVersion":"13","apiLevel":33,"diskBytes":1,"createdAt":null,
              "lastStartedAt":null,"capabilities":{"probedAt":null,"items":[],
              "nativeBridge":null,"mediaVolume":null,"googleAccounts":null,
              "foregroundPackage":null,"display":null,"packages":[]},"deviceId":null,
              "registrationOpenedAt":null,"rootEnabled":null,
              "boot":{"method":"direct","src":"ome"}
            }"#,
        )
        .expect("direct boot metadata");

        let record = GuestStore::new(root).load_all().expect("load").remove(0);

        let boot = record.boot.expect("direct boot");
        assert!(!boot.migrated_from_disk);
        assert!(!boot.direct_boot_verified);
        assert!(!record.direct_boot_migration_attempted);
    }

    #[test]
    fn running_install_loads_as_failed() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("vm");
        let guest = root.join("default");
        fs::create_dir_all(&guest).expect("guest directory");
        fs::write(guest.join("disk.qcow2"), [0_u8; 1]).expect("disk");
        let store = GuestStore::new(&root);
        let mut record = store.load_all().expect("adopt").remove(0);
        record.install = Some(StoredInstall {
            state: StoredInstallState::Running,
            finished_at: None,
            failure: None,
        });
        store.save(&record).expect("save running install");
        let loaded = store.load_all().expect("reload").remove(0);
        assert_eq!(
            loaded.install,
            Some(StoredInstall {
                state: StoredInstallState::Failed,
                finished_at: None,
                failure: Some("설치가 끝나기 전에 앱이 종료되었습니다.".to_owned()),
            })
        );
    }

    #[test]
    fn replaces_an_existing_document_without_leaving_staging_data() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("vm");
        let guest = root.join("default");
        fs::create_dir_all(&guest).expect("guest directory");
        fs::write(guest.join("disk.qcow2"), [0_u8; 11]).expect("disk");
        let store = GuestStore::new(&root);
        let mut record = store.load_all().expect("load").remove(0);
        store.save(&record).expect("first save");
        record.last_started_at = Some("2026-09-27T12:00:00+09:00".to_owned());
        store.save(&record).expect("replacement save");
        assert_eq!(store.load_all().expect("reload"), [record]);
        assert!(!guest.join("guest.json.staging").exists());
    }

    #[test]
    fn rejects_directory_escape() {
        let directory = tempfile::tempdir().expect("temp directory");
        let store = GuestStore::new(directory.path());
        assert!(matches!(
            store.guest_dir("../other"),
            Err(GuestStoreError::InvalidId)
        ));
    }

    #[test]
    fn save_active_preserves_saved_wizard() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("vm");
        fs::create_dir(&root).expect("guest root");
        let store = GuestStore::new(root);
        let wizard = WizardState {
            step: ome_wizard::Step::FirstBoot,
            completed: std::collections::BTreeSet::from([ome_wizard::Step::HostCheck]),
        };

        store.save_wizard(&wizard).expect("save wizard");
        store.save_active(Some("default")).expect("save active");

        assert_eq!(store.load_wizard().expect("load wizard"), Some(wizard));
    }

    #[test]
    fn save_wizard_preserves_saved_active_guest() {
        let directory = tempfile::tempdir().expect("temp directory");
        let root = directory.path().join("vm");
        fs::create_dir(&root).expect("guest root");
        let store = GuestStore::new(root);
        let wizard = WizardState {
            step: ome_wizard::Step::FirstBoot,
            completed: std::collections::BTreeSet::from([ome_wizard::Step::HostCheck]),
        };

        store.save_active(Some("default")).expect("save active");
        store.save_wizard(&wizard).expect("save wizard");

        assert_eq!(
            store.load_active().expect("load active").as_deref(),
            Some("default")
        );
    }
}
