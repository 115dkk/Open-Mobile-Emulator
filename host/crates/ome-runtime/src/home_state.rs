// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Versioned runtime state shared by guest selection and the first-run wizard.
#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use ome_wizard::WizardState;
use serde::{Deserialize, Serialize};

use crate::guest_store::{GuestStoreError, validate_id};

pub(crate) const HOME_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HomeState {
    pub version: u32,
    pub active_guest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wizard: Option<WizardState>,
}

impl Default for HomeState {
    fn default() -> Self {
        Self {
            version: HOME_STATE_SCHEMA_VERSION,
            active_guest: None,
            wizard: None,
        }
    }
}

pub(crate) fn load(home: &Path) -> Result<HomeState, GuestStoreError> {
    let path = home.join("state.json");
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HomeState::default());
        }
        Err(error) => return Err(GuestStoreError::Io(error)),
    };
    let state: HomeState = serde_json::from_slice(&bytes).map_err(GuestStoreError::Json)?;
    if state.version != HOME_STATE_SCHEMA_VERSION {
        return Err(GuestStoreError::UnsupportedVersion);
    }
    if let Some(id) = state.active_guest.as_deref() {
        validate_id(id)?;
    }
    Ok(state)
}

pub(crate) fn save(home: &Path, state: &HomeState) -> Result<(), GuestStoreError> {
    if let Some(id) = state.active_guest.as_deref() {
        validate_id(id)?;
    }
    let path = home.join("state.json");
    let staging = home.join("state.json.staging");
    let bytes = serde_json::to_vec_pretty(state).map_err(GuestStoreError::Json)?;
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ome_wizard::Step;

    use super::*;

    #[test]
    fn legacy_state_without_wizard_loads_with_none() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join("state.json"),
            r#"{"version":1,"activeGuest":"default"}"#,
        )
        .expect("legacy state");

        let state = load(directory.path()).expect("load legacy state");
        assert_eq!(state.version, HOME_STATE_SCHEMA_VERSION);
        assert_eq!(state.active_guest.as_deref(), Some("default"));
        assert_eq!(state.wizard, None);
    }

    #[test]
    fn wizard_state_round_trips() {
        let directory = tempfile::tempdir().expect("temp directory");
        let state = HomeState {
            version: HOME_STATE_SCHEMA_VERSION,
            active_guest: Some("default".to_owned()),
            wizard: Some(WizardState {
                step: Step::FirstBoot,
                completed: BTreeSet::from([Step::HostCheck, Step::GuestInstall]),
            }),
        };

        save(directory.path(), &state).expect("save state");
        assert_eq!(load(directory.path()).expect("load state"), state);
    }

    #[test]
    fn unsupported_version_fails() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join("state.json"),
            r#"{"version":2,"activeGuest":null}"#,
        )
        .expect("unsupported state");

        assert!(matches!(
            load(directory.path()),
            Err(GuestStoreError::UnsupportedVersion)
        ));
    }
}
