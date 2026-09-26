// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Pure stage geometry and a placeholder for native guest-window hosting.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A rectangle in physical pixels relative to the host client area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    /// Horizontal offset in physical pixels.
    pub x: i32,
    /// Vertical offset in physical pixels.
    pub y: i32,
    /// Physical width.
    pub width: u32,
    /// Physical height.
    pub height: u32,
}

/// A guest or viewport size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// A webview stage rectangle expressed in CSS pixels plus its device scale factor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageRect {
    /// CSS-pixel horizontal offset.
    pub x: f64,
    /// CSS-pixel vertical offset.
    pub y: f64,
    /// CSS-pixel width.
    pub width: f64,
    /// CSS-pixel height.
    pub height: f64,
    /// Physical pixels per CSS pixel.
    pub scale_factor: f64,
}

/// How the guest framebuffer occupies the stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageFit {
    /// Preserve aspect ratio and center a letterboxed rectangle.
    FitWindow,
    /// Keep one guest pixel per physical host pixel and center it.
    OneToOne,
}

/// Pure CSS-to-physical stage calculations.
#[derive(Clone, Copy, Debug, Default)]
pub struct StageGeometry;

impl StageGeometry {
    /// Converts every stage component to physical pixels by nearest-pixel rounding.
    ///
    /// Non-finite values, non-positive scale factors, negative dimensions, and values outside the
    /// destination integer range are clamped to a safe empty or bounded rectangle.
    pub fn physical(stage: &StageRect) -> Rect {
        if !stage.scale_factor.is_finite() || stage.scale_factor <= 0.0 {
            return Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            };
        }
        Rect {
            x: round_i32(stage.x * stage.scale_factor),
            y: round_i32(stage.y * stage.scale_factor),
            width: round_u32(stage.width * stage.scale_factor),
            height: round_u32(stage.height * stage.scale_factor),
        }
    }

    /// Fits a guest into a physical stage according to `fit`, centered on both axes.
    ///
    /// A zero guest or stage dimension returns an empty rectangle at the stage origin. Fit-window
    /// uses integer ratio comparison and never stretches one axis beyond the stage.
    pub fn fit(stage_px: Rect, guest: Size, fit: StageFit) -> Rect {
        if stage_px.width == 0 || stage_px.height == 0 || guest.width == 0 || guest.height == 0 {
            return Rect {
                x: stage_px.x,
                y: stage_px.y,
                width: 0,
                height: 0,
            };
        }
        let (width, height) = match fit {
            StageFit::OneToOne => (
                guest.width.min(stage_px.width),
                guest.height.min(stage_px.height),
            ),
            StageFit::FitWindow => {
                let stage_w = u64::from(stage_px.width);
                let stage_h = u64::from(stage_px.height);
                let guest_w = u64::from(guest.width);
                let guest_h = u64::from(guest.height);
                if stage_w * guest_h <= stage_h * guest_w {
                    let height = stage_w * guest_h / guest_w;
                    (
                        stage_px.width,
                        u32::try_from(height).expect("height is bounded by stage"),
                    )
                } else {
                    let width = stage_h * guest_w / guest_h;
                    (
                        u32::try_from(width).expect("width is bounded by stage"),
                        stage_px.height,
                    )
                }
            }
        };
        let offset_x = i64::from((stage_px.width - width) / 2);
        let offset_y = i64::from((stage_px.height - height) / 2);
        Rect {
            x: saturating_i32(i64::from(stage_px.x) + offset_x),
            y: saturating_i32(i64::from(stage_px.y) + offset_y),
            width,
            height,
        }
    }
}

fn round_i32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn round_u32(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    value.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

fn saturating_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    })
}

/// Opaque identifiers supplied by the native platform adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostingTarget {
    /// Host parent-window identifier, interpreted only by the native adapter.
    pub parent_window: u64,
    /// Guest process identifier.
    pub guest_process_id: u32,
}

/// Guest-window hosting facade reserved for the platform adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct GuestWindowHost;

impl GuestWindowHost {
    /// Attaches the guest's top-level window to the host parent.
    ///
    /// Returns [`HostingIssue::Unwired`] until the platform crate's window functions are connected.
    pub fn attach(&mut self, _target: HostingTarget) -> Result<(), HostingIssue> {
        // wiring: ome-platform-win supplies window discovery and safe attach here.
        Err(HostingIssue::Unwired)
    }

    /// Places an attached guest window in a physical client rectangle.
    ///
    /// Returns [`HostingIssue::Unwired`] until the platform window adapter is connected.
    pub fn place(&mut self, _rect: Rect) -> Result<(), HostingIssue> {
        // wiring: ome-platform-win applies the physical rectangle here.
        Err(HostingIssue::Unwired)
    }

    /// Restores the guest to a top-level window when possible.
    ///
    /// Returns [`HostingIssue::Unwired`] until the platform window adapter is connected.
    pub fn detach(&mut self) -> Result<(), HostingIssue> {
        // wiring: ome-platform-win restores style and parent here.
        Err(HostingIssue::Unwired)
    }
}

/// Guest-window hosting failures.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum HostingIssue {
    /// The platform window functions have not been connected yet.
    #[error("guest window hosting is not wired")]
    Unwired,
    /// The guest process has no eligible top-level window.
    #[error("guest window was not found")]
    WindowNotFound,
    /// The operating system rejected attach, placement, or detach.
    #[error("guest window hosting failed")]
    Platform,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_rounds_each_css_component() {
        assert_eq!(
            StageGeometry::physical(&StageRect {
                x: 10.25,
                y: 20.75,
                width: 100.2,
                height: 50.4,
                scale_factor: 1.5,
            }),
            Rect {
                x: 15,
                y: 31,
                width: 150,
                height: 76,
            }
        );
    }

    #[test]
    fn fit_window_centers_horizontal_letterbox() {
        assert_eq!(
            StageGeometry::fit(
                Rect {
                    x: 10,
                    y: 20,
                    width: 1000,
                    height: 1000,
                },
                Size {
                    width: 1920,
                    height: 1080,
                },
                StageFit::FitWindow,
            ),
            Rect {
                x: 10,
                y: 239,
                width: 1000,
                height: 562,
            }
        );
    }

    #[test]
    fn fit_window_centers_vertical_letterbox() {
        assert_eq!(
            StageGeometry::fit(
                Rect {
                    x: 5,
                    y: 7,
                    width: 1600,
                    height: 900,
                },
                Size {
                    width: 720,
                    height: 1280,
                },
                StageFit::FitWindow,
            ),
            Rect {
                x: 552,
                y: 7,
                width: 506,
                height: 900,
            }
        );
    }

    #[test]
    fn one_to_one_is_centered_and_clamped() {
        assert_eq!(
            StageGeometry::fit(
                Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                Size {
                    width: 1920,
                    height: 600,
                },
                StageFit::OneToOne,
            ),
            Rect {
                x: 0,
                y: 60,
                width: 1280,
                height: 600,
            }
        );
    }

    #[test]
    fn hosting_methods_fail_closed_until_platform_wiring_arrives() {
        let mut host = GuestWindowHost;
        assert_eq!(
            host.attach(HostingTarget {
                parent_window: 1,
                guest_process_id: 2,
            }),
            Err(HostingIssue::Unwired)
        );
        assert_eq!(
            host.place(Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }),
            Err(HostingIssue::Unwired)
        );
        assert_eq!(host.detach(), Err(HostingIssue::Unwired));
    }
}
