// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Loading, validation, ordering, and recommendation of replaceable guest image profiles.
#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod family;
pub use family::*;

/// Distribution family used to build an image.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Distribution {
    /// Bliss OS image.
    Bliss,
    /// Android-x86 image.
    AndroidX86,
    /// Image built by this project's builder.
    SelfBuilt,
}

/// Native bridge included by an image.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Translator {
    /// Intel Houdini bridge.
    Houdini,
    /// Google libndk_translation bridge.
    NdkTranslation,
    /// Digitalis bridge.
    Digitalis,
    /// No native bridge.
    None,
}

/// Product support state for an image profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageStatus {
    /// The product has completion evidence for this image.
    Verified,
    /// The image has not completed product verification.
    Candidate,
    /// The image remains visible but is no longer supported.
    Deprecated,
}

/// Allow-listed QEMU behavior that an image profile may request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QemuOverride {
    /// Use the virgl-capable virtio VGA device.
    VirtioVgaGl,
    /// Enable OpenGL on the SDL display backend.
    SdlGl,
    /// Disable generated EDID data.
    EdidOff,
}

/// One dated verification record and its repository evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRecord {
    /// Verification date in `YYYY-MM-DD` form.
    pub verified_at: String,
    /// Repository-relative evidence document.
    pub evidence: String,
}

/// One installable guest image declaration from `manifests/images`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestImageProfile {
    /// Stable lowercase profile ID.
    pub id: String,
    /// User-visible image name.
    pub display_name: String,
    /// User-visible Android version.
    pub android_version: String,
    /// Android API level.
    pub api_level: u32,
    /// Distribution family.
    pub distribution: Distribution,
    /// Name of the matching entry in `manifests/artifacts.json`.
    pub artifact: String,
    /// Native bridge included by the image.
    pub translator: Translator,
    /// Additional kernel arguments selected at boot.
    pub boot_args: Vec<String>,
    /// Text used to identify the intended GRUB entry.
    pub grub_entry_hint: String,
    /// Ordered installer guidance shown by the wizard.
    pub install_guide: Vec<String>,
    /// Restricted QEMU behavior requested by this profile.
    pub qemu_overrides: Vec<QemuOverride>,
    /// Product support state.
    pub status: ImageStatus,
    /// Publisher release date in `YYYY-MM-DD` form when known.
    pub released_at: Option<String>,
    /// Product verification records.
    #[serde(default)]
    pub verifications: Vec<VerificationRecord>,
}

impl GuestImageProfile {
    /// Reads, validates, and returns every `.json` profile in a directory.
    ///
    /// Non-JSON entries are ignored. Any directory, file, JSON, or validation error aborts the
    /// operation so callers never receive a partly trusted profile list.
    pub fn load_all(dir: impl AsRef<Path>) -> Result<Vec<Self>, ProfileError> {
        let mut paths = fs::read_dir(dir)
            .map_err(ProfileError::Io)?
            .map(|entry| entry.map(|entry| entry.path()).map_err(ProfileError::Io))
            .collect::<Result<Vec<_>, _>>()?;
        paths.retain(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        });
        paths.sort();
        let mut profiles = Vec::with_capacity(paths.len());
        for path in paths {
            let json = fs::read_to_string(&path).map_err(ProfileError::Io)?;
            let profile: Self =
                serde_json::from_str(&json).map_err(|source| ProfileError::Json {
                    path: path.clone(),
                    source,
                })?;
            profile.validate()?;
            profiles.push(profile);
        }
        let mut ids = std::collections::BTreeSet::new();
        for profile in &profiles {
            if !ids.insert(profile.id.as_str()) {
                return Err(ProfileError::DuplicateId(profile.id.clone()));
            }
        }
        Ok(profiles)
    }

    /// Checks the field invariants that do not require the artifact manifest.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if !(21..=40).contains(&self.api_level) {
            return Err(ProfileError::InvalidApiLevel(self.api_level));
        }
        if self.id.is_empty()
            || !self.id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.'
            })
        {
            return Err(ProfileError::InvalidId(self.id.clone()));
        }
        for (field, value) in [
            ("display_name", self.display_name.as_str()),
            ("android_version", self.android_version.as_str()),
            ("artifact", self.artifact.as_str()),
            ("grub_entry_hint", self.grub_entry_hint.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ProfileError::Blank { field });
            }
        }
        if self.boot_args.is_empty()
            || self.boot_args.iter().any(|value| value.trim().is_empty())
            || self.install_guide.len() != 6
            || self
                .install_guide
                .iter()
                .any(|value| value.trim().is_empty())
        {
            return Err(ProfileError::InvalidInstructions);
        }
        if let Some(released_at) = self.released_at.as_deref() {
            parse_date(released_at).ok_or_else(|| ProfileError::InvalidDate(released_at.into()))?;
        }
        for verification in &self.verifications {
            if parse_date(&verification.verified_at).is_none()
                || verification.evidence.trim().is_empty()
                || Path::new(&verification.evidence).is_absolute()
                || verification.evidence.contains(char::from(92))
                || verification
                    .evidence
                    .split('/')
                    .any(|component| component == "..")
            {
                return Err(ProfileError::InvalidVerification);
            }
        }
        Ok(())
    }
}

/// Sorts profiles by Android version descending, then release date and ID descending.
pub fn sort_newest_first(profiles: &mut [GuestImageProfile]) {
    profiles.sort_by(|left, right| {
        version_key(&right.android_version)
            .cmp(&version_key(&left.android_version))
            .then_with(|| right.released_at.cmp(&left.released_at))
            .then_with(|| right.id.cmp(&left.id))
    });
}

/// Returns the index of the one profile Rust should recommend.
///
/// The newest verified profile more than 28 days old is preferred. If no verified profile has
/// crossed that threshold, the newest verified profile is returned. Invalid `today` input safely
/// disables only the age preference and still returns the newest verified profile.
pub fn recommended_index(profiles: &[GuestImageProfile], today: &str) -> Option<usize> {
    let newest = |indices: Vec<usize>| {
        indices.into_iter().max_by(|left, right| {
            profile_recency(&profiles[*left]).cmp(&profile_recency(&profiles[*right]))
        })
    };
    let mature = parse_date(today).map(|today| {
        profiles
            .iter()
            .enumerate()
            .filter(|(_, profile)| profile.status == ImageStatus::Verified)
            .filter_map(|(index, profile)| {
                let released = profile.released_at.as_deref().and_then(parse_date)?;
                (today.saturating_sub(released) > 28).then_some(index)
            })
            .collect::<Vec<_>>()
    });
    mature.and_then(newest).or_else(|| {
        newest(
            profiles
                .iter()
                .enumerate()
                .filter_map(|(index, profile)| {
                    (profile.status == ImageStatus::Verified).then_some(index)
                })
                .collect(),
        )
    })
}

fn profile_recency(profile: &GuestImageProfile) -> (Vec<u32>, Option<i64>, &str) {
    (
        version_key(&profile.android_version),
        profile.released_at.as_deref().and_then(parse_date),
        profile.id.as_str(),
    )
}

fn version_key(version: &str) -> Vec<u32> {
    version
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}

fn parse_date(value: &str) -> Option<i64> {
    let prefix = value.get(..10)?;
    let bytes = prefix.as_bytes();
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    let year = prefix.get(0..4)?.parse::<i32>().ok()?;
    let month = prefix.get(5..7)?.parse::<u32>().ok()?;
    let day = prefix.get(8..10)?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i32::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month =
        i32::try_from(month).expect("month fits i32") + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i32::try_from(day).expect("day fits i32") - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era)
}

/// Image-profile loading and validation failures.
#[derive(Debug, Error)]
pub enum ProfileError {
    /// A profile directory or file could not be read.
    #[error("guest image profile could not be read")]
    Io(#[source] std::io::Error),
    /// One profile is malformed or contains an unknown field.
    #[error("guest image profile JSON is invalid: {path}")]
    Json {
        /// Profile path.
        path: PathBuf,
        /// JSON parser failure.
        #[source]
        source: serde_json::Error,
    },
    /// API level is outside 21 through 40.
    #[error("guest image API level is invalid: {0}")]
    InvalidApiLevel(u32),
    /// Profile ID contains characters outside `[a-z0-9-]`.
    #[error("guest image profile ID is invalid: {0}")]
    InvalidId(String),
    /// A required version or identity field is blank.
    #[error("guest image field is blank: {field}")]
    Blank {
        /// Rejected field name.
        field: &'static str,
    },
    /// Boot or installation guidance is absent or malformed.
    #[error("guest image instructions are invalid")]
    InvalidInstructions,
    /// A date is not a valid `YYYY-MM-DD` prefix.
    #[error("guest image date is invalid: {0}")]
    InvalidDate(String),
    /// A verification record is malformed or points outside the repository.
    #[error("guest image verification record is invalid")]
    InvalidVerification,
    /// Two files declare the same profile ID.
    #[error("guest image profile ID is duplicated: {0}")]
    DuplicateId(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(
        id: &str,
        android: &str,
        status: ImageStatus,
        released_at: Option<&str>,
    ) -> GuestImageProfile {
        GuestImageProfile {
            id: id.to_owned(),
            display_name: format!("Android {android}"),
            android_version: android.to_owned(),
            api_level: 33,
            distribution: Distribution::Bliss,
            artifact: "artifact".to_owned(),
            translator: Translator::NdkTranslation,
            boot_args: vec!["quiet".to_owned()],
            grub_entry_hint: "Virgl".to_owned(),
            install_guide: (1..=6).map(|step| format!("Step {step}")).collect(),
            qemu_overrides: vec![QemuOverride::VirtioVgaGl],
            status,
            released_at: released_at.map(str::to_owned),
            verifications: Vec::new(),
        }
    }

    #[test]
    fn loads_json_files_and_rejects_unknown_fields() {
        let directory = tempfile::tempdir().expect("temp directory");
        let value = profile(
            "android-13",
            "13",
            ImageStatus::Verified,
            Some("2024-10-11"),
        );
        fs::write(
            directory.path().join("profile.json"),
            serde_json::to_vec(&value).expect("serialize"),
        )
        .expect("write profile");
        fs::write(directory.path().join("ignored.txt"), "not JSON").expect("write ignored");
        assert_eq!(
            GuestImageProfile::load_all(directory.path()).expect("load"),
            vec![value]
        );

        fs::write(
            directory.path().join("profile.json"),
            serde_json::to_string(&profile("android-13", "13", ImageStatus::Verified, None))
                .expect("serialize")
                .replacen('{', "{\"unknown\":true,", 1),
        )
        .expect("write invalid");
        assert!(matches!(
            GuestImageProfile::load_all(directory.path()),
            Err(ProfileError::Json { .. })
        ));
    }

    #[test]
    fn validation_rejects_api_id_blank_version_and_bad_date() {
        let mut value = profile(
            "android-13",
            "13",
            ImageStatus::Verified,
            Some("2024-10-11"),
        );
        value.api_level = 20;
        assert!(matches!(
            value.validate(),
            Err(ProfileError::InvalidApiLevel(20))
        ));
        value.api_level = 33;
        value.id = "Android_13".to_owned();
        assert!(matches!(value.validate(), Err(ProfileError::InvalidId(_))));
        value.id = "android-13".to_owned();
        value.android_version = "  ".to_owned();
        assert!(matches!(
            value.validate(),
            Err(ProfileError::Blank {
                field: "android_version"
            })
        ));
        value.android_version = "13".to_owned();
        value.released_at = Some("2024-02-30".to_owned());
        assert!(matches!(
            value.validate(),
            Err(ProfileError::InvalidDate(_))
        ));
    }

    #[test]
    fn repository_manifest_profiles_load_and_validate() {
        // Regression: the shipped profile ID `bliss-16.9.7-android-13` carries dots; rejecting it
        // made the product fail at start-up with `image_profiles_invalid`.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../manifests/images");
        let profiles = GuestImageProfile::load_all(&dir).expect("repository profiles load");
        assert!(
            profiles
                .iter()
                .any(|profile| profile.id == "bliss-16.9.7-android-13")
        );
    }

    #[test]
    fn newest_sort_uses_android_version_before_release_date() {
        let mut profiles = vec![
            profile("a13", "13", ImageStatus::Verified, Some("2025-01-01")),
            profile("a9", "9", ImageStatus::Verified, Some("2026-01-01")),
            profile("a15", "15", ImageStatus::Candidate, Some("2024-01-01")),
        ];
        sort_newest_first(&mut profiles);
        assert_eq!(
            profiles
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a15", "a13", "a9"]
        );
    }

    #[test]
    fn recommendation_rule_is_table_driven() {
        struct Case {
            name: &'static str,
            profiles: Vec<GuestImageProfile>,
            today: &'static str,
            expected: Option<&'static str>,
        }
        let cases = [
            Case {
                name: "newest mature verified",
                profiles: vec![
                    profile("13", "13", ImageStatus::Verified, Some("2024-10-11")),
                    profile("15", "15", ImageStatus::Verified, Some("2026-08-01")),
                    profile("16", "16", ImageStatus::Candidate, Some("2025-01-01")),
                ],
                today: "2026-09-27",
                expected: Some("15"),
            },
            Case {
                name: "exactly 28 days is not older",
                profiles: vec![
                    profile("13", "13", ImageStatus::Verified, Some("2024-10-11")),
                    profile("15", "15", ImageStatus::Verified, Some("2026-08-30")),
                ],
                today: "2026-09-27",
                expected: Some("13"),
            },
            Case {
                name: "newest verified fallback",
                profiles: vec![
                    profile("13", "13", ImageStatus::Verified, Some("2026-09-10")),
                    profile("15", "15", ImageStatus::Verified, Some("2026-09-20")),
                ],
                today: "2026-09-27",
                expected: Some("15"),
            },
            Case {
                name: "no verified profile",
                profiles: vec![profile(
                    "17",
                    "17",
                    ImageStatus::Candidate,
                    Some("2026-01-01"),
                )],
                today: "2026-09-27",
                expected: None,
            },
        ];
        for case in cases {
            let actual = recommended_index(&case.profiles, case.today)
                .map(|index| case.profiles[index].id.as_str());
            assert_eq!(actual, case.expected, "{}", case.name);
        }
    }
}
