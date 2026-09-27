// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Pure visibility and placement rules for the native overlay window.
#![forbid(unsafe_code)]

use ome_runtime::{AppSnapshot, GuestState};

/// How the overlay window should present the active input profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayMode {
    /// No overlay window is visible.
    Hidden,
    /// Binding markers are visible and mouse input passes through.
    Showing,
    /// The editor is visible and accepts mouse input.
    Editing,
}

/// A rectangle in physical screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    /// Horizontal screen coordinate.
    pub x: i32,
    /// Vertical screen coordinate.
    pub y: i32,
    /// Physical width.
    pub width: u32,
    /// Physical height.
    pub height: u32,
}

/// One complete native-window placement decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayPlacement {
    /// Visibility and interaction mode.
    pub mode: OverlayMode,
    /// Target screen rectangle, or none when the window must remain hidden.
    pub rect: Option<PhysicalRect>,
}

/// Applies the same hide and edit rules as the overlay webview.
pub fn overlay_mode(snapshot: &AppSnapshot) -> OverlayMode {
    let input = &snapshot.input;
    let active_profile_exists = input.active_id.as_ref().is_some_and(|active_id| {
        input
            .profiles
            .iter()
            .any(|profile| profile.id == *active_id)
    });
    if snapshot.guest.state != GuestState::Running
        || !active_profile_exists
        || (!input.overlay_visible && !input.editing)
    {
        OverlayMode::Hidden
    } else if input.editing {
        OverlayMode::Editing
    } else {
        OverlayMode::Showing
    }
}

/// Returns the part of the guest child covered by the overlay.
///
/// The child and guest image are the same size today. Keeping this seam lets letterboxing move here
/// if the hosted child later becomes larger than the guest image.
pub fn overlay_rect(guest_client_rect: PhysicalRect) -> PhysicalRect {
    guest_client_rect
}

/// Combines a mode with the attached guest child's known screen rectangle.
pub fn placement(mode: OverlayMode, guest_client_rect: Option<PhysicalRect>) -> OverlayPlacement {
    let rect = (mode != OverlayMode::Hidden)
        .then(|| guest_client_rect.map(overlay_rect))
        .flatten();
    OverlayPlacement { mode, rect }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> AppSnapshot {
        serde_json::from_str(include_str!(
            "../../../../tests/fixtures/contract/snapshot.sample.json"
        ))
        .expect("valid AppSnapshot fixture")
    }

    #[test]
    fn every_non_running_state_is_hidden() {
        for state in [
            GuestState::Stopped,
            GuestState::Starting,
            GuestState::Stopping,
            GuestState::Restarting,
            GuestState::Failed,
        ] {
            let mut value = snapshot();
            value.guest.state = state;
            assert_eq!(overlay_mode(&value), OverlayMode::Hidden, "{state:?}");
        }
    }

    #[test]
    fn running_requires_an_active_profile_that_exists() {
        let mut value = snapshot();
        value.input.active_id = None;
        assert_eq!(overlay_mode(&value), OverlayMode::Hidden);

        value.input.active_id = Some("missing-profile".to_owned());
        assert_eq!(overlay_mode(&value), OverlayMode::Hidden);

        value.input.active_id = Some(value.input.profiles[0].id.clone());
        assert_eq!(overlay_mode(&value), OverlayMode::Editing);
    }

    #[test]
    fn visibility_and_editing_table_matches_the_webview() {
        for (overlay_visible, editing, expected) in [
            (false, false, OverlayMode::Hidden),
            (true, false, OverlayMode::Showing),
            (false, true, OverlayMode::Editing),
            (true, true, OverlayMode::Editing),
        ] {
            let mut value = snapshot();
            value.input.overlay_visible = overlay_visible;
            value.input.editing = editing;
            assert_eq!(
                overlay_mode(&value),
                expected,
                "overlay_visible={overlay_visible}, editing={editing}"
            );
        }
    }

    #[test]
    fn overlay_rect_is_the_guest_client_rect() {
        let rect = PhysicalRect {
            x: -150,
            y: 240,
            width: 1920,
            height: 1080,
        };
        assert_eq!(overlay_rect(rect), rect);
    }

    #[test]
    fn placement_has_no_rect_when_hidden_or_unknown() {
        let rect = PhysicalRect {
            x: 10,
            y: 20,
            width: 1280,
            height: 720,
        };
        assert_eq!(
            placement(OverlayMode::Hidden, Some(rect)),
            OverlayPlacement {
                mode: OverlayMode::Hidden,
                rect: None,
            }
        );
        assert_eq!(
            placement(OverlayMode::Showing, None),
            OverlayPlacement {
                mode: OverlayMode::Showing,
                rect: None,
            }
        );
        assert_eq!(
            placement(OverlayMode::Editing, Some(rect)),
            OverlayPlacement {
                mode: OverlayMode::Editing,
                rect: Some(rect),
            }
        );
    }
}
