// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Pure stage geometry and native guest-window hosting.
#![forbid(unsafe_code)]

use std::thread;
use std::time::{Duration, Instant};

use ome_platform_win::{PreviousStyle, WindowHandle, find_windows_of_process};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const DISCOVERY_INTERVAL: Duration = Duration::from_millis(100);
const SDL_WINDOW_CLASS: &str = "SDL_app";

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

#[derive(Debug)]
struct HostedWindow {
    parent: WindowHandle,
    previous: PreviousStyle,
}

#[derive(Clone, Copy, Debug)]
struct GuestWindow {
    handle: WindowHandle,
    process_id: u32,
}

impl GuestWindow {
    fn alive(self) -> bool {
        self.handle.belongs_to_process(self.process_id)
    }
}

/// Owns the reversible conversion of one QEMU SDL window into a hosted child.
#[derive(Debug, Default)]
pub struct GuestWindowHost {
    guest: Option<GuestWindow>,
    hosted: Option<HostedWindow>,
    last_rect: Option<Rect>,
    last_discovery_time: Option<Duration>,
    guest_dpi_before_attach: Option<u32>,
}

impl GuestWindowHost {
    /// Attaches the guest's preferred visible top-level window to the host parent.
    ///
    /// Discovery waits up to five seconds for a visible window, preferring
    /// `SDL_app` whenever it appears in an enumeration. Style, extended style,
    /// and parent changes are rolled back together
    /// when any native operation fails.
    pub fn attach(&mut self, target: HostingTarget) -> Result<(), HostingIssue> {
        self.detach()?;
        self.guest = None;
        self.hosted = None;
        self.last_discovery_time = None;
        self.guest_dpi_before_attach = None;
        if target.parent_window == 0 || target.guest_process_id == 0 {
            return Err(HostingIssue::Platform);
        }
        let parent =
            WindowHandle::from_u64(target.parent_window).map_err(|_| HostingIssue::Platform)?;
        let discovery_started = Instant::now();
        let guest = GuestWindow {
            handle: discover_guest_window(target.guest_process_id)?,
            process_id: target.guest_process_id,
        };
        self.last_discovery_time = Some(discovery_started.elapsed());
        self.guest_dpi_before_attach =
            Some(guest.handle.dpi().map_err(|_| HostingIssue::Platform)?);
        let rect = self.last_rect.unwrap_or(Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });
        let (width, height) = native_dimensions(rect)?;
        let previous = guest
            .handle
            .make_child_of_at(parent, rect.x, rect.y, width, height)
            .map_err(|_| HostingIssue::Platform)?;
        self.guest = Some(guest);
        self.hosted = Some(HostedWindow { parent, previous });
        Ok(())
    }

    /// Places an attached guest window in a physical parent-client rectangle.
    ///
    /// The latest rectangle is retained for the next attach. Calls made while no
    /// live guest is attached are accepted without a native operation.
    pub fn place(&mut self, rect: Rect) -> Result<(), HostingIssue> {
        self.last_rect = Some(rect);
        let (Some(_hosted), Some(guest)) = (self.hosted.as_ref(), self.guest) else {
            return Ok(());
        };
        if !guest.alive() {
            self.hosted = None;
            self.guest = None;
            return Ok(());
        }
        let (width, height) = native_dimensions(rect)?;
        guest
            .handle
            .set_bounds(rect.x, rect.y, width, height)
            .map_err(|_| HostingIssue::Platform)
    }

    /// Hides the attached guest child without detaching it.
    pub fn hide(&mut self) -> Result<(), HostingIssue> {
        let (Some(_hosted), Some(guest)) = (self.hosted.as_ref(), self.guest) else {
            return Ok(());
        };
        if !guest.alive() {
            self.hosted = None;
            self.guest = None;
            return Ok(());
        }
        guest.handle.hide().map_err(|_| HostingIssue::Platform)
    }

    /// Shows the attached guest child without taking keyboard focus.
    pub fn show(&mut self) -> Result<(), HostingIssue> {
        let (Some(_hosted), Some(guest)) = (self.hosted.as_ref(), self.guest) else {
            return Ok(());
        };
        if !guest.alive() {
            self.hosted = None;
            self.guest = None;
            return Ok(());
        }
        guest
            .handle
            .show_inactive()
            .map_err(|_| HostingIssue::Platform)
    }

    /// Restores the guest's saved style, extended style, and parent.
    ///
    /// A vanished or PID-reused HWND is treated as already detached.
    pub fn detach(&mut self) -> Result<(), HostingIssue> {
        let Some(hosted) = self.hosted.take() else {
            if self.guest.is_some_and(|guest| !guest.alive()) {
                self.guest = None;
            }
            return Ok(());
        };
        let Some(guest) = self.guest else {
            return Ok(());
        };
        if !guest.alive() {
            self.guest = None;
            return Ok(());
        }
        if let Err(_error) = guest.handle.restore_top_level(hosted.previous) {
            self.hosted = Some(hosted);
            return Err(HostingIssue::Platform);
        }
        Ok(())
    }

    /// Brings the guest forward while leaving keyboard focus with the host.
    ///
    /// Hosted children ask the parent window thread to raise the child and activate
    /// the parent. If the child held focus, the same callback returns it to the
    /// parent. A detached remembered window receives a best-effort foreground
    /// request. With no live window, the method is a no-op.
    pub fn to_front(&mut self) -> Result<(), HostingIssue> {
        let Some(guest) = self.guest else {
            return Ok(());
        };
        if !guest.alive() {
            self.hosted = None;
            self.guest = None;
            return Ok(());
        }
        if let Some(hosted) = self.hosted.as_ref() {
            return guest
                .handle
                .focus_as_child_of(hosted.parent)
                .map_err(|_| HostingIssue::Platform);
        }
        guest
            .handle
            .to_foreground()
            .map_err(|_| HostingIssue::Platform)
    }

    /// Reports whether a live guest window is currently embedded.
    pub fn is_attached(&self) -> bool {
        self.hosted.is_some() && self.guest_window_alive()
    }

    /// Reports whether the discovered HWND still exists and belongs to the PID.
    pub fn guest_window_alive(&self) -> bool {
        self.guest.is_some_and(GuestWindow::alive)
    }

    /// Reports whether the embedded guest is the focused window in the parent's GUI queue.
    pub fn guest_has_parent_thread_focus(&self) -> Result<bool, HostingIssue> {
        let Some(hosted) = self.hosted.as_ref() else {
            return Ok(false);
        };
        let Some(guest) = self.guest.filter(|guest| guest.alive()) else {
            return Ok(false);
        };
        hosted
            .parent
            .thread_focus()
            .map(|focused| focused == Some(guest.handle))
            .map_err(|_| HostingIssue::Platform)
    }

    /// Returns the discovered guest window token for diagnostics and native tests.
    pub fn guest_window(&self) -> Option<WindowHandle> {
        self.guest
            .filter(|guest| guest.alive())
            .map(|guest| guest.handle)
    }

    /// Returns how long the most recent successful window discovery took.
    pub fn last_discovery_time(&self) -> Option<Duration> {
        self.last_discovery_time
    }

    /// Returns the guest window DPI sampled immediately before the last attach.
    pub fn guest_dpi_before_attach(&self) -> Option<u32> {
        self.guest_dpi_before_attach
    }
}

impl Drop for GuestWindowHost {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}

fn discover_guest_window(pid: u32) -> Result<WindowHandle, HostingIssue> {
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;
    loop {
        let windows = find_windows_of_process(pid).map_err(|_| HostingIssue::Platform)?;
        if let Some(window) = preferred_visible_window(&windows) {
            return Ok(window);
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(HostingIssue::WindowNotFound);
        }
        thread::sleep(DISCOVERY_INTERVAL.min(deadline.saturating_duration_since(now)));
    }
}

fn preferred_visible_window(windows: &[WindowHandle]) -> Option<WindowHandle> {
    let mut first_visible = None;
    for window in windows.iter().copied() {
        if !window.is_visible().unwrap_or(false) {
            continue;
        }
        first_visible.get_or_insert(window);
        if window
            .class_name()
            .is_ok_and(|class_name| class_name == SDL_WINDOW_CLASS)
        {
            return Some(window);
        }
    }
    first_visible
}

fn native_dimensions(rect: Rect) -> Result<(i32, i32), HostingIssue> {
    let width = i32::try_from(rect.width).map_err(|_| HostingIssue::Platform)?;
    let height = i32::try_from(rect.height).map_err(|_| HostingIssue::Platform)?;
    Ok((width, height))
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
    /// The operating system rejected attach, placement, focus, or detach.
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
    fn unattached_place_and_detach_are_idempotent() {
        let mut host = GuestWindowHost::default();
        host.place(Rect {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: u32::MAX,
        })
        .expect("unattached place");
        host.hide().expect("unattached hide");
        host.show().expect("unattached show");
        host.detach().expect("unattached detach");
        host.to_front().expect("unattached focus");
        assert!(!host.is_attached());
        assert!(!host.guest_window_alive());
    }

    #[test]
    fn window_preference_uses_no_ineligible_windows() {
        assert_eq!(preferred_visible_window(&[]), None);
    }

    #[test]
    fn native_dimensions_reject_values_outside_win32_range() {
        assert_eq!(
            native_dimensions(Rect {
                x: 0,
                y: 0,
                width: i32::MAX as u32 + 1,
                height: 1,
            }),
            Err(HostingIssue::Platform)
        );
    }
}
