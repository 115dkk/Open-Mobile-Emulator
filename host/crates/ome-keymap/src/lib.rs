// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Validated keymap profiles and pure translation to absolute touch events.
#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current keymap JSON schema version.
pub const KEYMAP_VERSION: u32 = 1;
/// Absolute pointer maximum used by guest input events.
pub const ABSOLUTE_AXIS_MAX: u16 = 32_767;

/// One versioned keymap profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeymapProfile {
    /// Must equal [`KEYMAP_VERSION`].
    pub version: u32,
    /// Stable profile identifier.
    pub id: String,
    /// User-visible profile name.
    pub name: String,
    /// Optional authoring note ignored by mapping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Key-to-touch bindings.
    pub bindings: Vec<Binding>,
}

impl KeymapProfile {
    /// Parses and validates a profile JSON document.
    ///
    /// Unknown fields, unsupported versions, duplicate keys, invalid W3C code names, out-of-range
    /// coordinates, and zero-duration swipes return [`ProfileError`].
    pub fn parse(json: &str) -> Result<Self, ProfileError> {
        let profile: Self = serde_json::from_str(json).map_err(ProfileError::Json)?;
        profile.validate()?;
        Ok(profile)
    }

    /// Reads and validates a profile file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let json = fs::read_to_string(path).map_err(ProfileError::Io)?;
        Self::parse(&json)
    }

    /// Checks all profile invariants without performing I/O.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.version != KEYMAP_VERSION {
            return Err(ProfileError::UnsupportedVersion(self.version));
        }
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err(ProfileError::EmptyIdentity);
        }
        let mut keys = std::collections::BTreeSet::new();
        for binding in &self.bindings {
            if !is_keyboard_code(&binding.key) {
                return Err(ProfileError::InvalidKey(binding.key.clone()));
            }
            if !keys.insert(binding.key.as_str()) {
                return Err(ProfileError::DuplicateKey(binding.key.clone()));
            }
            match &binding.action {
                Action::Tap { x, y } => validate_point(*x, *y)?,
                Action::Swipe {
                    from,
                    to,
                    duration_ms,
                } => {
                    validate_point(from[0], from[1])?;
                    validate_point(to[0], to[1])?;
                    if *duration_ms == 0 {
                        return Err(ProfileError::InvalidDuration);
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_point(x: f64, y: f64) -> Result<(), ProfileError> {
    if x.is_finite() && y.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
        Ok(())
    } else {
        Err(ProfileError::InvalidCoordinate)
    }
}

fn is_keyboard_code(code: &str) -> bool {
    matches!(
        code,
        "Backquote"
            | "Backslash"
            | "Backspace"
            | "BracketLeft"
            | "BracketRight"
            | "Comma"
            | "ContextMenu"
            | "Delete"
            | "End"
            | "Enter"
            | "Equal"
            | "Escape"
            | "Home"
            | "Insert"
            | "Minus"
            | "PageDown"
            | "PageUp"
            | "Period"
            | "Quote"
            | "Semicolon"
            | "Slash"
            | "Space"
            | "Tab"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "ArrowUp"
            | "ShiftLeft"
            | "ShiftRight"
            | "ControlLeft"
            | "ControlRight"
            | "AltLeft"
            | "AltRight"
            | "MetaLeft"
            | "MetaRight"
            | "CapsLock"
            | "NumLock"
            | "ScrollLock"
    ) || code.strip_prefix("Key").is_some_and(|suffix| {
        suffix.len() == 1 && suffix.bytes().all(|byte| byte.is_ascii_uppercase())
    }) || code
        .strip_prefix("Digit")
        .is_some_and(|suffix| suffix.len() == 1 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
        || code
            .strip_prefix('F')
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|number| (1..=24).contains(&number))
        || code.strip_prefix("Numpad").is_some_and(|suffix| {
            matches!(
                suffix,
                "0" | "1"
                    | "2"
                    | "3"
                    | "4"
                    | "5"
                    | "6"
                    | "7"
                    | "8"
                    | "9"
                    | "Add"
                    | "Comma"
                    | "Decimal"
                    | "Divide"
                    | "Enter"
                    | "Equal"
                    | "Multiply"
                    | "Subtract"
            )
        })
}

/// One key binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// W3C `KeyboardEvent.code` value.
    pub key: String,
    /// Touch action produced by a press event.
    pub action: Action,
}

/// Normalized touch action in profile space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Action {
    /// Instant touch at normalized coordinates.
    Tap {
        /// Horizontal coordinate from 0.0 to 1.0.
        x: f64,
        /// Vertical coordinate from 0.0 to 1.0.
        y: f64,
    },
    /// Linear touch movement in normalized coordinates.
    Swipe {
        /// Start coordinate `[x, y]`.
        from: [f64; 2],
        /// End coordinate `[x, y]`.
        to: [f64; 2],
        /// Nominal duration used to determine interpolation count.
        #[serde(rename = "durationMs")]
        duration_ms: u32,
    },
}

/// One host key event supplied by the native keyboard adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// W3C `KeyboardEvent.code` value.
    pub code: String,
    /// True for key down, false for key up.
    pub pressed: bool,
}

/// One item in QEMU's QMP `input-send-event` list.
///
/// Coordinates are emitted as separate absolute X and Y items, and contact uses the QMP `touch`
/// button. Serializing this value produces the QMP `type` and nested `data` object unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
pub enum InputEvent {
    /// Absolute pointer movement on one 0..=32767 axis.
    Abs {
        /// Axis changed by this item.
        axis: InputAxis,
        /// Absolute axis value.
        value: u16,
    },
    /// Touch-contact button state.
    Btn {
        /// Always [`InputButton::Touch`] for mapper output.
        button: InputButton,
        /// True for press and false for release.
        down: bool,
    },
}

/// Axis names accepted by QMP absolute pointer movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputAxis {
    /// Horizontal axis.
    X,
    /// Vertical axis.
    Y,
}

/// Pointer button names used by mapper output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputButton {
    /// Screen contact on the guest touch device.
    Touch,
}

/// Stateless profile mapper.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mapper;

impl Mapper {
    /// Translates a pressed key into absolute touch items.
    ///
    /// Releases and keys without a binding return an empty vector. Each pointer position emits an
    /// absolute X item followed by an absolute Y item. A tap then emits touch press and release. A
    /// swipe emits start position, press, interpolated positions, and release. Swipe duration is
    /// sampled every 16 ms and capped at 120 movement intervals.
    pub fn translate(profile: &KeymapProfile, event: KeyEvent) -> Vec<InputEvent> {
        if !event.pressed {
            return Vec::new();
        }
        let Some(binding) = profile
            .bindings
            .iter()
            .find(|binding| binding.key == event.code)
        else {
            return Vec::new();
        };
        match binding.action {
            Action::Tap { x, y } => {
                let mut events = position_events(x, y).to_vec();
                events.extend([
                    InputEvent::Btn {
                        button: InputButton::Touch,
                        down: true,
                    },
                    InputEvent::Btn {
                        button: InputButton::Touch,
                        down: false,
                    },
                ]);
                events
            }
            Action::Swipe {
                from,
                to,
                duration_ms,
            } => {
                let intervals = duration_ms.div_ceil(16).clamp(1, 120);
                let interval_count = usize::try_from(intervals).expect("interval bound fits usize");
                let mut events = Vec::with_capacity(interval_count.saturating_mul(2) + 4);
                events.extend(position_events(from[0], from[1]));
                events.push(InputEvent::Btn {
                    button: InputButton::Touch,
                    down: true,
                });
                for index in 1..=intervals {
                    let progress = f64::from(index) / f64::from(intervals);
                    events.extend(position_events(
                        from[0] + (to[0] - from[0]) * progress,
                        from[1] + (to[1] - from[1]) * progress,
                    ));
                }
                events.push(InputEvent::Btn {
                    button: InputButton::Touch,
                    down: false,
                });
                events
            }
        }
    }
}

fn position_events(x: f64, y: f64) -> [InputEvent; 2] {
    [
        InputEvent::Abs {
            axis: InputAxis::X,
            value: normalized_axis(x),
        },
        InputEvent::Abs {
            axis: InputAxis::Y,
            value: normalized_axis(y),
        },
    ]
}

fn normalized_axis(value: f64) -> u16 {
    (value.clamp(0.0, 1.0) * f64::from(ABSOLUTE_AXIS_MAX)).floor() as u16
}

/// Loads every `.json` profile in a preset directory, sorted by path.
///
/// Non-JSON files are ignored. Any directory read, file read, or validation error aborts the load;
/// callers never receive a partially validated set.
pub fn load_presets(directory: impl AsRef<Path>) -> Result<Vec<KeymapProfile>, ProfileError> {
    let mut paths = fs::read_dir(directory)
        .map_err(ProfileError::Io)?
        .map(|entry| entry.map(|entry| entry.path()).map_err(ProfileError::Io))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| {
        path.extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    });
    paths.sort();
    paths.into_iter().map(KeymapProfile::load).collect()
}

/// Profile parsing and validation errors.
#[derive(Debug, Error)]
pub enum ProfileError {
    /// JSON is malformed or contains unknown fields.
    #[error("keymap JSON is invalid")]
    Json(#[source] serde_json::Error),
    /// A profile file or directory could not be read.
    #[error("keymap file could not be read")]
    Io(#[source] std::io::Error),
    /// Only [`KEYMAP_VERSION`] is supported.
    #[error("unsupported keymap version {0}")]
    UnsupportedVersion(u32),
    /// Profile ID and name must both contain non-whitespace text.
    #[error("keymap identity is empty")]
    EmptyIdentity,
    /// Binding key is not a supported W3C code name.
    #[error("invalid KeyboardEvent.code: {0}")]
    InvalidKey(String),
    /// One code appears more than once.
    #[error("duplicate key binding: {0}")]
    DuplicateKey(String),
    /// A normalized coordinate is non-finite or outside 0.0..=1.0.
    #[error("touch coordinate is invalid")]
    InvalidCoordinate,
    /// Swipe duration must be positive.
    #[error("swipe duration is invalid")]
    InvalidDuration,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> KeymapProfile {
        KeymapProfile::parse(
            r#"{
                "version": 1,
                "id": "test",
                "name": "Test",
                "bindings": [
                    {"key":"Space","action":{"kind":"tap","x":0.5,"y":1.0}},
                    {"key":"ArrowLeft","action":{"kind":"swipe","from":[1.0,0.5],"to":[0.0,0.5],"durationMs":32}}
                ]
            }"#,
        )
        .expect("profile")
    }

    #[test]
    fn tap_and_release_translation_table() {
        let cases = [
            (
                KeyEvent {
                    code: "Space".to_owned(),
                    pressed: true,
                },
                vec![
                    InputEvent::Abs {
                        axis: InputAxis::X,
                        value: 16_383,
                    },
                    InputEvent::Abs {
                        axis: InputAxis::Y,
                        value: 32_767,
                    },
                    InputEvent::Btn {
                        button: InputButton::Touch,
                        down: true,
                    },
                    InputEvent::Btn {
                        button: InputButton::Touch,
                        down: false,
                    },
                ],
            ),
            (
                KeyEvent {
                    code: "Space".to_owned(),
                    pressed: false,
                },
                Vec::new(),
            ),
            (
                KeyEvent {
                    code: "KeyA".to_owned(),
                    pressed: true,
                },
                Vec::new(),
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(Mapper::translate(&profile(), event), expected);
        }
    }

    #[test]
    fn swipe_expands_to_start_intermediate_final_and_release() {
        assert_eq!(
            Mapper::translate(
                &profile(),
                KeyEvent {
                    code: "ArrowLeft".to_owned(),
                    pressed: true,
                }
            ),
            vec![
                InputEvent::Abs {
                    axis: InputAxis::X,
                    value: 32_767,
                },
                InputEvent::Abs {
                    axis: InputAxis::Y,
                    value: 16_383,
                },
                InputEvent::Btn {
                    button: InputButton::Touch,
                    down: true,
                },
                InputEvent::Abs {
                    axis: InputAxis::X,
                    value: 16_383,
                },
                InputEvent::Abs {
                    axis: InputAxis::Y,
                    value: 16_383,
                },
                InputEvent::Abs {
                    axis: InputAxis::X,
                    value: 0,
                },
                InputEvent::Abs {
                    axis: InputAxis::Y,
                    value: 16_383,
                },
                InputEvent::Btn {
                    button: InputButton::Touch,
                    down: false,
                },
            ]
        );
    }

    #[test]
    fn serializes_qmp_input_event_shape() {
        let events = Mapper::translate(
            &profile(),
            KeyEvent {
                code: "Space".to_owned(),
                pressed: true,
            },
        );
        assert_eq!(
            serde_json::to_value(&events).expect("events serialize"),
            serde_json::json!([
                {"type":"abs","data":{"axis":"x","value":16383}},
                {"type":"abs","data":{"axis":"y","value":32767}},
                {"type":"btn","data":{"button":"touch","down":true}},
                {"type":"btn","data":{"button":"touch","down":false}}
            ])
        );
    }

    #[test]
    fn rejects_bad_coordinates_duplicate_keys_and_unknown_codes() {
        for json in [
            r#"{"version":1,"id":"x","name":"x","bindings":[{"key":"Space","action":{"kind":"tap","x":1.1,"y":0.0}}]}"#,
            r#"{"version":1,"id":"x","name":"x","bindings":[{"key":"Space","action":{"kind":"tap","x":0.0,"y":0.0}},{"key":"Space","action":{"kind":"tap","x":0.0,"y":0.0}}]}"#,
            r#"{"version":1,"id":"x","name":"x","bindings":[{"key":"A","action":{"kind":"tap","x":0.0,"y":0.0}}]}"#,
        ] {
            assert!(KeymapProfile::parse(json).is_err());
        }
    }
}
