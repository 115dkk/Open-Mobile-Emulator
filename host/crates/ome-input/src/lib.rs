// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Validated input profiles and pure translation to absolute touch events.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current input-profile JSON schema version.
pub const INPUT_PROFILE_VERSION: u32 = 2;
/// Absolute pointer maximum used by guest input events.
pub const ABSOLUTE_AXIS_MAX: u16 = 32_767;

/// A width and height pair used for a profile's reference aspect ratio.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Size {
    /// Reference width.
    pub width: u32,
    /// Reference height.
    pub height: u32,
}

/// Policy used when the current display aspect differs from the profile reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    /// Keep the shorter axis and center the longer axis.
    Center,
    /// Preserve each point's distance from its nearest edges.
    Edges,
}

/// One normalized point in guest display coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalPoint {
    /// Horizontal coordinate from 0.0 through 1.0.
    pub x: f64,
    /// Vertical coordinate from 0.0 through 1.0.
    pub y: f64,
}

/// Mouse buttons accepted by an input trigger.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
    /// Primary mouse button.
    Left,
    /// Secondary mouse button.
    Right,
    /// Middle mouse button.
    Middle,
}

/// Mouse-wheel directions accepted by an input trigger.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WheelDirection {
    /// Wheel movement away from the user.
    Up,
    /// Wheel movement toward the user.
    Down,
}

/// Host input that activates a binding.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Trigger {
    /// One W3C `KeyboardEvent.code` value.
    Key {
        /// Keyboard code.
        code: String,
    },
    /// One mouse button.
    MouseButton {
        /// Mouse button.
        button: MouseButton,
    },
    /// One mouse-wheel direction.
    Wheel {
        /// Wheel direction.
        direction: WheelDirection,
    },
    /// Four keyboard codes interpreted as a virtual joystick.
    KeySet {
        /// Up direction code.
        up: String,
        /// Down direction code.
        down: String,
        /// Left direction code.
        left: String,
        /// Right direction code.
        right: String,
    },
}

/// Guest-side action produced by a binding.
///
/// This enum intentionally leaves the future sequence/macro action slot open. D9 excludes that
/// action from v1, so no sequence variant exists yet.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BindingAction {
    /// Touch one point, optionally holding contact until host-key release.
    Tap {
        /// Touch point.
        at: LogicalPoint,
        /// Whether contact remains down until release.
        hold: bool,
    },
    /// Move a touch contact linearly between two points.
    Swipe {
        /// Starting point.
        from: LogicalPoint,
        /// Ending point.
        to: LogicalPoint,
        /// Nominal duration in milliseconds.
        duration_ms: u32,
    },
    /// Move a held touch around a virtual joystick.
    Joystick {
        /// Joystick center.
        center: LogicalPoint,
        /// Radius relative to the display's shorter axis.
        radius: f64,
    },
    /// Touch either the current pointer position or one fixed point.
    MouseTap {
        /// Fixed point, or `None` for the current pointer position.
        at: Option<LogicalPoint>,
    },
    /// Turn wheel movement into a swipe around one point.
    WheelSwipe {
        /// Swipe center.
        at: LogicalPoint,
        /// Swipe distance in logical coordinates.
        distance: f64,
    },
    /// Forward the host input as a guest key without touch synthesis.
    PassThrough,
}

/// One stable input binding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    /// Stable identifier within its profile.
    pub id: String,
    /// Host input trigger.
    pub trigger: Trigger,
    /// Guest-side action.
    pub action: BindingAction,
}

/// One validated input profile exposed to the webview.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputProfile {
    /// Stable profile identifier.
    pub id: String,
    /// User-visible profile name.
    pub name: String,
    /// Whether the profile ships with the product and cannot be overwritten or deleted.
    #[serde(default)]
    pub bundled: bool,
    /// Optional package that selects this profile during automatic application.
    pub target_package: Option<String>,
    /// Aspect ratio used while authoring the logical points.
    pub reference_aspect: Size,
    /// Aspect-ratio adjustment policy.
    pub anchor: Anchor,
    /// Input bindings in display order.
    pub bindings: Vec<Binding>,
}

impl InputProfile {
    /// Parses and validates one versioned profile JSON document.
    pub fn parse(json: &str) -> Result<Self, ProfileError> {
        let document: serde_json::Value = serde_json::from_str(json).map_err(ProfileError::Json)?;
        let version = document
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(ProfileError::MissingVersion)?;
        if version != INPUT_PROFILE_VERSION {
            return Err(ProfileError::UnsupportedVersion(version));
        }
        let object = document.as_object().ok_or(ProfileError::MissingVersion)?;
        let mut profile_value = serde_json::Map::new();
        for (key, value) in object {
            if key != "version" {
                profile_value.insert(key.clone(), value.clone());
            }
        }
        let profile: Self = serde_json::from_value(serde_json::Value::Object(profile_value))
            .map_err(ProfileError::Json)?;
        profile.validate()?;
        Ok(profile)
    }

    /// Reads and validates one profile file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let json = fs::read_to_string(path).map_err(ProfileError::Io)?;
        Self::parse(&json)
    }

    /// Serializes this profile as a version-2 persistent document.
    pub fn to_document_json(&self) -> Result<String, ProfileError> {
        self.validate()?;
        let profile = serde_json::to_value(self).map_err(ProfileError::Json)?;
        let profile = profile
            .as_object()
            .expect("InputProfile always serializes as an object");
        let mut document = serde_json::Map::new();
        document.insert("version".to_owned(), INPUT_PROFILE_VERSION.into());
        for (key, value) in profile {
            if key != "bundled" {
                document.insert(key.clone(), value.clone());
            }
        }
        serde_json::to_string_pretty(&serde_json::Value::Object(document))
            .map_err(ProfileError::Json)
    }

    /// Checks profile identity, coordinates, triggers, and action-specific invariants.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err(ProfileError::EmptyIdentity);
        }
        if self.reference_aspect.width == 0 || self.reference_aspect.height == 0 {
            return Err(ProfileError::InvalidAspect);
        }
        if self
            .target_package
            .as_ref()
            .is_some_and(|package| package.trim().is_empty())
        {
            return Err(ProfileError::InvalidPackage);
        }
        let mut binding_ids = BTreeSet::new();
        let mut triggers = BTreeSet::new();
        let mut keyboard_codes = BTreeSet::new();
        for binding in &self.bindings {
            if binding.id.trim().is_empty() || !binding_ids.insert(binding.id.as_str()) {
                return Err(ProfileError::DuplicateOrEmptyBindingId(binding.id.clone()));
            }
            validate_trigger(&binding.trigger)?;
            if !triggers.insert(binding.trigger.clone()) {
                return Err(ProfileError::DuplicateTrigger);
            }
            for code in trigger_keyboard_codes(&binding.trigger) {
                if !keyboard_codes.insert(code) {
                    return Err(ProfileError::DuplicateTrigger);
                }
            }
            validate_action(&binding.action)?;
        }
        Ok(())
    }
}

fn validate_trigger(trigger: &Trigger) -> Result<(), ProfileError> {
    match trigger {
        Trigger::Key { code } => validate_code(code),
        Trigger::MouseButton { .. } | Trigger::Wheel { .. } => Ok(()),
        Trigger::KeySet {
            up,
            down,
            left,
            right,
        } => {
            for code in [up, down, left, right] {
                validate_code(code)?;
            }
            let codes = BTreeSet::from([up.as_str(), down.as_str(), left.as_str(), right.as_str()]);
            if codes.len() != 4 {
                return Err(ProfileError::DuplicateJoystickKey);
            }
            Ok(())
        }
    }
}

fn trigger_keyboard_codes(trigger: &Trigger) -> Vec<&str> {
    match trigger {
        Trigger::Key { code } => vec![code.as_str()],
        Trigger::KeySet {
            up,
            down,
            left,
            right,
        } => vec![up.as_str(), down.as_str(), left.as_str(), right.as_str()],
        Trigger::MouseButton { .. } | Trigger::Wheel { .. } => Vec::new(),
    }
}

fn validate_code(code: &str) -> Result<(), ProfileError> {
    if is_keyboard_code(code) {
        Ok(())
    } else {
        Err(ProfileError::InvalidKey(code.to_owned()))
    }
}

fn validate_action(action: &BindingAction) -> Result<(), ProfileError> {
    match action {
        BindingAction::Tap { at, .. } => validate_point(*at),
        BindingAction::Swipe {
            from,
            to,
            duration_ms,
        } => {
            validate_point(*from)?;
            validate_point(*to)?;
            if *duration_ms == 0 {
                return Err(ProfileError::InvalidDuration);
            }
            Ok(())
        }
        BindingAction::Joystick { center, radius } => {
            validate_point(*center)?;
            validate_positive(*radius, ProfileError::InvalidRadius)
        }
        BindingAction::MouseTap { at } => at.map_or(Ok(()), validate_point),
        BindingAction::WheelSwipe { at, distance } => {
            validate_point(*at)?;
            validate_positive(*distance, ProfileError::InvalidDistance)
        }
        BindingAction::PassThrough => Ok(()),
    }
}

fn validate_point(point: LogicalPoint) -> Result<(), ProfileError> {
    if point.x.is_finite()
        && point.y.is_finite()
        && (0.0..=1.0).contains(&point.x)
        && (0.0..=1.0).contains(&point.y)
    {
        Ok(())
    } else {
        Err(ProfileError::InvalidCoordinate)
    }
}

fn validate_positive(value: f64, error: ProfileError) -> Result<(), ProfileError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(error)
    }
}

/// Returns whether text is one supported W3C `KeyboardEvent.code` value.
pub fn is_keyboard_code(code: &str) -> bool {
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

/// One host key event supplied by the native keyboard adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    /// W3C `KeyboardEvent.code` value.
    pub code: String,
    /// True for key down, false for key up.
    pub pressed: bool,
}

/// One item in QEMU's QMP `input-send-event` list.
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
        /// Pointer button.
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

/// Stateless profile mapper for the currently wired touch actions.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mapper;

impl Mapper {
    /// Translates a host key press or release into QMP absolute touch items.
    pub fn translate(profile: &InputProfile, event: KeyEvent) -> Vec<InputEvent> {
        let Some(binding) = profile.bindings.iter().find(
            |binding| matches!(&binding.trigger, Trigger::Key { code } if code == &event.code),
        ) else {
            return Vec::new();
        };
        match &binding.action {
            BindingAction::Tap { at, hold } => {
                if !event.pressed {
                    return if *hold {
                        vec![InputEvent::Btn {
                            button: InputButton::Touch,
                            down: false,
                        }]
                    } else {
                        Vec::new()
                    };
                }
                let mut events = position_events(at.x, at.y).to_vec();
                events.push(InputEvent::Btn {
                    button: InputButton::Touch,
                    down: true,
                });
                if !hold {
                    events.push(InputEvent::Btn {
                        button: InputButton::Touch,
                        down: false,
                    });
                }
                events
            }
            BindingAction::Swipe {
                from,
                to,
                duration_ms,
            } if event.pressed => swipe_events(*from, *to, *duration_ms),
            BindingAction::Swipe { .. } => Vec::new(),
            // wiring: joystick, mouseTap, wheelSwipe, and passThrough need their capture/scheduler
            // adapters before they can emit QMP items.
            BindingAction::Joystick { .. }
            | BindingAction::MouseTap { .. }
            | BindingAction::WheelSwipe { .. }
            | BindingAction::PassThrough => Vec::new(),
        }
    }
}

fn swipe_events(from: LogicalPoint, to: LogicalPoint, duration_ms: u32) -> Vec<InputEvent> {
    let intervals = duration_ms.div_ceil(16).clamp(1, 120);
    let interval_count = usize::try_from(intervals).expect("interval bound fits usize");
    let mut events = Vec::with_capacity(interval_count.saturating_mul(2) + 4);
    events.extend(position_events(from.x, from.y));
    events.push(InputEvent::Btn {
        button: InputButton::Touch,
        down: true,
    });
    for index in 1..=intervals {
        let progress = f64::from(index) / f64::from(intervals);
        events.extend(position_events(
            from.x + (to.x - from.x) * progress,
            from.y + (to.y - from.y) * progress,
        ));
    }
    events.push(InputEvent::Btn {
        button: InputButton::Touch,
        down: false,
    });
    events
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

/// Loads every `.json` profile in a trusted directory, sorted by path.
pub fn load_profiles(directory: impl AsRef<Path>) -> Result<Vec<InputProfile>, ProfileError> {
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
    paths.into_iter().map(InputProfile::load).collect()
}

/// Profile parsing and validation errors.
#[derive(Debug, Error)]
pub enum ProfileError {
    /// JSON is malformed or contains unknown fields.
    #[error("input profile JSON is invalid")]
    Json(#[source] serde_json::Error),
    /// A profile file or directory could not be read.
    #[error("input profile file could not be read")]
    Io(#[source] std::io::Error),
    /// A version field was absent or was not an unsigned integer.
    #[error("input profile version is missing")]
    MissingVersion,
    /// Only [`INPUT_PROFILE_VERSION`] is supported.
    #[error("unsupported input profile version {0}")]
    UnsupportedVersion(u32),
    /// Profile ID and name must contain non-whitespace text.
    #[error("input profile identity is empty")]
    EmptyIdentity,
    /// Reference width and height must be positive.
    #[error("input profile reference aspect is invalid")]
    InvalidAspect,
    /// Target package must be absent or non-blank.
    #[error("input profile target package is invalid")]
    InvalidPackage,
    /// Binding IDs must be non-blank and unique.
    #[error("input binding ID is empty or duplicated: {0}")]
    DuplicateOrEmptyBindingId(String),
    /// One trigger appears more than once in a profile.
    #[error("input trigger is duplicated")]
    DuplicateTrigger,
    /// Binding key is not a supported W3C code name.
    #[error("invalid KeyboardEvent.code: {0}")]
    InvalidKey(String),
    /// A joystick's four direction keys must differ.
    #[error("joystick direction keys must be distinct")]
    DuplicateJoystickKey,
    /// A logical coordinate is non-finite or outside 0.0..=1.0.
    #[error("logical coordinate is invalid")]
    InvalidCoordinate,
    /// Swipe duration must be positive.
    #[error("swipe duration is invalid")]
    InvalidDuration,
    /// Joystick radius must be finite and positive.
    #[error("joystick radius is invalid")]
    InvalidRadius,
    /// Wheel-swipe distance must be finite and positive.
    #[error("wheel swipe distance is invalid")]
    InvalidDistance,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> InputProfile {
        InputProfile::parse(
            r#"{
                "version": 2,
                "id": "test",
                "name": "Test",
                "targetPackage": null,
                "referenceAspect": {"width": 16, "height": 9},
                "anchor": "center",
                "bindings": [
                    {"id":"tap","trigger":{"kind":"key","code":"Space"},"action":{"kind":"tap","at":{"x":0.5,"y":1.0},"hold":false}},
                    {"id":"hold","trigger":{"kind":"key","code":"KeyH"},"action":{"kind":"tap","at":{"x":0.25,"y":0.25},"hold":true}},
                    {"id":"swipe","trigger":{"kind":"key","code":"ArrowLeft"},"action":{"kind":"swipe","from":{"x":1.0,"y":0.5},"to":{"x":0.0,"y":0.5},"durationMs":32}}
                ]
            }"#,
        )
        .expect("profile")
    }

    #[test]
    fn tap_and_hold_translation_table() {
        let cases = [
            ("Space", true, 4, Some(false)),
            ("Space", false, 0, None),
            ("KeyH", true, 3, Some(true)),
            ("KeyH", false, 1, Some(false)),
            ("KeyA", true, 0, None),
        ];
        for (code, pressed, length, last_down) in cases {
            let events = Mapper::translate(
                &profile(),
                KeyEvent {
                    code: code.to_owned(),
                    pressed,
                },
            );
            assert_eq!(events.len(), length, "{code} {pressed}");
            if let Some(down) = last_down {
                assert_eq!(
                    events.last(),
                    Some(&InputEvent::Btn {
                        button: InputButton::Touch,
                        down,
                    })
                );
            }
        }
    }

    #[test]
    fn swipe_expands_to_start_intermediate_final_and_release() {
        let events = Mapper::translate(
            &profile(),
            KeyEvent {
                code: "ArrowLeft".to_owned(),
                pressed: true,
            },
        );
        assert_eq!(events.len(), 8);
        assert_eq!(
            events.first(),
            Some(&InputEvent::Abs {
                axis: InputAxis::X,
                value: 32_767,
            })
        );
        assert_eq!(
            events.last(),
            Some(&InputEvent::Btn {
                button: InputButton::Touch,
                down: false,
            })
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
    fn validation_rejects_each_contract_rule() {
        let invalid = [
            r#"{"version":2,"id":"x","name":"x","targetPackage":null,"referenceAspect":{"width":16,"height":9},"anchor":"center","bindings":[{"id":"a","trigger":{"kind":"key","code":"Space"},"action":{"kind":"tap","at":{"x":1.1,"y":0.0},"hold":false}}]}"#,
            r#"{"version":2,"id":"x","name":"x","targetPackage":null,"referenceAspect":{"width":16,"height":9},"anchor":"center","bindings":[{"id":"a","trigger":{"kind":"key","code":"Space"},"action":{"kind":"tap","at":{"x":0.0,"y":0.0},"hold":false}},{"id":"b","trigger":{"kind":"key","code":"Space"},"action":{"kind":"tap","at":{"x":0.0,"y":0.0},"hold":false}}]}"#,
            r#"{"version":2,"id":"x","name":"x","targetPackage":null,"referenceAspect":{"width":16,"height":9},"anchor":"center","bindings":[{"id":"a","trigger":{"kind":"keySet","up":"KeyW","down":"KeyW","left":"KeyA","right":"KeyD"},"action":{"kind":"joystick","center":{"x":0.5,"y":0.5},"radius":0.2}}]}"#,
            r#"{"version":2,"id":"x","name":"x","targetPackage":null,"referenceAspect":{"width":16,"height":9},"anchor":"center","bindings":[{"id":"a","trigger":{"kind":"key","code":"Space"},"action":{"kind":"swipe","from":{"x":0.0,"y":0.0},"to":{"x":1.0,"y":1.0},"durationMs":0}}]}"#,
            r#"{"version":2,"id":"x","name":"x","targetPackage":null,"referenceAspect":{"width":16,"height":9},"anchor":"center","bindings":[{"id":"a","trigger":{"kind":"keySet","up":"KeyW","down":"KeyS","left":"KeyA","right":"KeyD"},"action":{"kind":"joystick","center":{"x":0.5,"y":0.5},"radius":0.0}}]}"#,
            r#"{"version":2,"id":"x","name":"x","targetPackage":null,"referenceAspect":{"width":16,"height":9},"anchor":"center","bindings":[{"id":"a","trigger":{"kind":"wheel","direction":"up"},"action":{"kind":"wheelSwipe","at":{"x":0.5,"y":0.5},"distance":0.0}}]}"#,
        ];
        for json in invalid {
            assert!(InputProfile::parse(json).is_err(), "{json}");
        }
    }

    #[test]
    fn persistent_document_round_trips_without_bundled_flag() {
        let mut original = profile();
        original.bundled = true;
        let json = original.to_document_json().expect("document JSON");
        assert!(json.contains("\"version\": 2"));
        assert!(!json.contains("bundled"));
        let loaded = InputProfile::parse(&json).expect("reload");
        assert!(!loaded.bundled);
        assert_eq!(loaded.id, original.id);
    }
}
