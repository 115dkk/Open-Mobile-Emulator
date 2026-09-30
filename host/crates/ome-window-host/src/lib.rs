// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Pure stage geometry and native guest-window hosting.
#![forbid(unsafe_code)]

use std::thread;
use std::time::{Duration, Instant};

use ome_platform_win::{WindowHandle, find_windows_of_process};
pub use ome_qmp::DisplayWindowGeometry;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const DISCOVERY_INTERVAL: Duration = Duration::from_millis(100);
const SDL_WINDOW_CLASS: &str = "SDL_app";

/// A rectangle in physical pixels; the frame (host client area or screen) is stated per use.
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
    /// Host owner-window identifier, interpreted only by the native adapter.
    pub parent_window: u64,
    /// Guest process identifier.
    pub guest_process_id: u32,
}

#[derive(Clone, Copy, Debug)]
struct GuestWindow {
    handle: WindowHandle,
    process_id: u32,
    owner: WindowHandle,
}

impl GuestWindow {
    fn alive(self) -> bool {
        self.handle.belongs_to_process(self.process_id)
    }
}

/// Tracks the QEMU SDL window that QEMU created as a popup owned by the host window.
///
/// QEMU patch 0005 (`-display sdl,owner-window=<HWND>`, ADR-0009 7번) creates the window
/// borderless, hidden, as a tool window with `WS_EX_NOACTIVATE`, already owned by the host, and
/// never resizes it to the guest resolution. Any host-side change to the style, owner or size of
/// that GL window freezes its presentation until the guest's next scanout
/// (`docs/evidence/M2/embedded-display-freeze.md`), so this type never restyles, re-owns, resizes,
/// shows or hides it. It changes only the Z-order and returns position, size and visibility as a
/// [`DisplayWindowGeometry`] that the caller sends over QMP `x-ome-display-window`; QEMU applies
/// it through SDL. QEMU owns the window's lifetime.
///
/// The geometry is in physical pixels, and QEMU runs per-monitor DPI aware. The calling process
/// must be per-monitor DPI aware too (the Tauri shell is, through `tao`), or a scaled monitor
/// hands it virtualized owner coordinates that no longer match QEMU's.
#[derive(Debug, Default)]
pub struct GuestWindowHost {
    guest: Option<GuestWindow>,
    overlay: Option<WindowHandle>,
    last_rect: Option<Rect>,
    visible: bool,
    last_discovery_time: Option<Duration>,
    guest_dpi_before_attach: Option<u32>,
}

impl GuestWindowHost {
    /// Finds the guest's `SDL_app` window, hidden or visible, and checks that it is top-level and
    /// owned by the host window. Nothing about the window is changed.
    ///
    /// Discovery waits up to five seconds for the owned `SDL_app` window of console 0; QEMU
    /// creates it hidden. QEMU makes one SDL window per console (the text consoles for the monitor,
    /// serial and parallel ports included) and patch 0005 gives every one of them the owner, while
    /// QMP `x-ome-display-window` drives console 0 only; its window is the one titled for console
    /// 0 (see [`is_first_console_title`]), or the only owned window. When the process has `SDL_app`
    /// windows but none of them qualifies, QEMU lacks patch 0005 or the configuration lacks
    /// `owner_window`, and the guest is not hosted ([`HostingIssue::Platform`]).
    pub fn attach(&mut self, target: HostingTarget) -> Result<(), HostingIssue> {
        self.detach()?;
        self.visible = false;
        self.last_discovery_time = None;
        self.guest_dpi_before_attach = None;
        if target.parent_window == 0 || target.guest_process_id == 0 {
            return Err(HostingIssue::Platform);
        }
        let owner =
            WindowHandle::from_u64(target.parent_window).map_err(|_| HostingIssue::Platform)?;
        let discovery_started = Instant::now();
        let handle = discover_guest_window(target.guest_process_id, owner)?;
        self.last_discovery_time = Some(discovery_started.elapsed());
        #[cfg(debug_assertions)]
        eprintln!(
            "[stage] guest popup discovered handle={:#x} after={}ms visible={:?} at={}",
            handle.as_u64(),
            discovery_started.elapsed().as_millis(),
            handle.is_visible().ok(),
            epoch_millis()
        );
        if !handle.is_top_level() || handle.owner() != Some(owner) {
            return Err(HostingIssue::Platform);
        }
        self.guest_dpi_before_attach = Some(handle.dpi().map_err(|_| HostingIssue::Platform)?);
        self.guest = Some(GuestWindow {
            handle,
            process_id: target.guest_process_id,
            owner,
        });
        Ok(())
    }

    /// Remembers a physical rectangle of the owner's client area, puts the popup in its Z-order
    /// place, and returns the visible geometry the caller must send over QMP.
    ///
    /// The rectangle is converted to screen pixels with the owner's current client origin, so
    /// calling again after the owner moves yields the corrected geometry. A visible registered
    /// overlay is the Z-order predecessor, preserving overlay above guest above owner; otherwise
    /// the popup goes to the top of the non-topmost band. An empty rectangle yields a hidden
    /// one-pixel geometry, since QEMU rejects a zero size. Returns `Ok(None)` without a native
    /// operation when no live guest is attached; the rectangle is still kept for the next attach.
    pub fn place(&mut self, rect: Rect) -> Result<Option<DisplayWindowGeometry>, HostingIssue> {
        self.last_rect = Some(rect);
        self.visible = true;
        let Some(guest) = self.live_guest() else {
            return Ok(None);
        };
        let screen = screen_rect(guest.owner, rect)?;
        self.apply_z_order(guest)?;
        display_geometry(screen, true).map(Some)
    }

    /// Registers the native overlay window used as the popup's Z-order predecessor.
    pub fn set_overlay_window(&mut self, overlay: Option<WindowHandle>) {
        self.overlay = overlay;
    }

    /// Checks the live popup against the expected screen rectangle.
    ///
    /// The expected rectangle is recomputed from the last rectangle and the owner's current client
    /// origin, so a moved owner is caught here as well. Returns `Some(expected)` for the caller to
    /// resend when the popup's window rectangle differs, and `None` when it matches, when the host
    /// keeps the popup hidden, or when nothing is attached. Re-applies the Z-order below a visible
    /// overlay.
    pub fn resync(&mut self) -> Result<Option<DisplayWindowGeometry>, HostingIssue> {
        let Some(rect) = self.last_rect else {
            return Ok(None);
        };
        let Some(guest) = self.live_guest() else {
            return Ok(None);
        };
        if let Some(overlay) = self.visible_overlay() {
            guest
                .handle
                .place_z_order(Some(overlay))
                .map_err(|_| HostingIssue::Platform)?;
        }
        if !self.visible {
            return Ok(None);
        }
        let expected = display_geometry(screen_rect(guest.owner, rect)?, true)?;
        if !expected.visible {
            return Ok(None);
        }
        let actual = guest
            .handle
            .window_rect()
            .map_err(|_| HostingIssue::Platform)?;
        if actual.x == expected.x
            && actual.y == expected.y
            && actual.width == expected.width
            && actual.height == expected.height
        {
            return Ok(None);
        }
        Ok(Some(expected))
    }

    /// Returns the last geometry with `visible: false` for the caller to send.
    ///
    /// The host itself never hides the window. Returns `Ok(None)` when no live guest is attached
    /// or no rectangle is known yet.
    pub fn hide(&mut self) -> Result<Option<DisplayWindowGeometry>, HostingIssue> {
        self.visible = false;
        self.current_geometry()
    }

    /// Returns the last geometry with `visible: true` for the caller to send.
    ///
    /// The host itself never shows the window; QEMU shows it without activation. Returns
    /// `Ok(None)` when no live guest is attached or no rectangle is known yet.
    pub fn show(&mut self) -> Result<Option<DisplayWindowGeometry>, HostingIssue> {
        self.visible = true;
        self.current_geometry()
    }

    /// Forgets the guest window. QEMU owns the window's lifetime, so nothing is restored.
    pub fn detach(&mut self) -> Result<(), HostingIssue> {
        self.guest = None;
        Ok(())
    }

    /// Changes only the popup's Z-order, without focus or activation.
    ///
    /// A visible registered overlay remains immediately above the guest. With no live guest or no
    /// remembered rectangle, the method is a no-op.
    pub fn to_front(&mut self) -> Result<(), HostingIssue> {
        if self.last_rect.is_none() {
            return Ok(());
        }
        let Some(guest) = self.live_guest() else {
            return Ok(());
        };
        self.apply_z_order(guest)
    }

    /// Reports whether a live guest popup is top-level and owned by the registered host.
    pub fn is_attached(&self) -> bool {
        let Some(guest) = self.guest else {
            return false;
        };
        guest.alive() && guest.handle.is_top_level() && guest.handle.owner() == Some(guest.owner)
    }

    /// Reports whether the discovered HWND still exists and belongs to the PID.
    pub fn guest_window_alive(&self) -> bool {
        self.guest.is_some_and(GuestWindow::alive)
    }

    /// Reports whether the guest owns focus in the host GUI thread queue.
    ///
    /// Owned popups never receive focus through product hosting; this diagnostic remains for the
    /// former child-mode spike and verifies that the host queue did not retain the guest HWND.
    pub fn guest_has_parent_thread_focus(&self) -> Result<bool, HostingIssue> {
        let Some(guest) = self.guest.filter(|guest| guest.alive()) else {
            return Ok(false);
        };
        guest
            .owner
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

    /// Returns the guest window DPI sampled when the last attach found the window.
    pub fn guest_dpi_before_attach(&self) -> Option<u32> {
        self.guest_dpi_before_attach
    }

    fn live_guest(&mut self) -> Option<GuestWindow> {
        let guest = self.guest?;
        if guest.alive() {
            Some(guest)
        } else {
            self.guest = None;
            None
        }
    }

    fn current_geometry(&mut self) -> Result<Option<DisplayWindowGeometry>, HostingIssue> {
        let Some(rect) = self.last_rect else {
            return Ok(None);
        };
        let Some(guest) = self.live_guest() else {
            return Ok(None);
        };
        let visible = self.visible;
        display_geometry(screen_rect(guest.owner, rect)?, visible).map(Some)
    }

    fn visible_overlay(&self) -> Option<WindowHandle> {
        self.overlay
            .filter(|overlay| overlay.is_visible().unwrap_or(false))
    }

    fn apply_z_order(&self, guest: GuestWindow) -> Result<(), HostingIssue> {
        #[cfg(debug_assertions)]
        eprintln!(
            "[stage] popup z-order insert_after={:?} at={}",
            self.visible_overlay().map(WindowHandle::as_u64),
            epoch_millis()
        );
        guest
            .handle
            .place_z_order(self.visible_overlay())
            .map_err(|_| HostingIssue::Platform)
    }
}

/// Wall-clock milliseconds for the debug traces, matching the runtime's `at=` stamps.
#[cfg(debug_assertions)]
fn epoch_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}

/// Converts a rectangle relative to the owner's client area into physical screen pixels.
fn screen_rect(owner: WindowHandle, rect: Rect) -> Result<Rect, HostingIssue> {
    let origin = owner
        .client_screen_rect()
        .map_err(|_| HostingIssue::Platform)?;
    Ok(Rect {
        x: rect.x.saturating_add(origin.x),
        y: rect.y.saturating_add(origin.y),
        width: rect.width,
        height: rect.height,
    })
}

/// Builds the QMP geometry for a screen rectangle.
///
/// QEMU accepts sizes from one pixel to `i32::MAX`. A larger size is a platform failure; an empty
/// rectangle becomes a hidden one-pixel window at the same position.
fn display_geometry(screen: Rect, visible: bool) -> Result<DisplayWindowGeometry, HostingIssue> {
    native_dimensions(screen)?;
    let empty = screen.width == 0 || screen.height == 0;
    Ok(DisplayWindowGeometry {
        x: screen.x,
        y: screen.y,
        width: screen.width.max(1),
        height: screen.height.max(1),
        visible: visible && !empty,
    })
}

fn discover_guest_window(pid: u32, owner: WindowHandle) -> Result<WindowHandle, HostingIssue> {
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;
    loop {
        let windows = find_windows_of_process(pid).map_err(|_| HostingIssue::Platform)?;
        let sdl = sdl_windows(&windows);
        let owned = sdl
            .iter()
            .copied()
            .filter(|window| window.owner() == Some(owner))
            .collect::<Vec<_>>();
        if let Some(window) = first_console_window(&owned) {
            return Ok(window);
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(if sdl.is_empty() {
                HostingIssue::WindowNotFound
            } else {
                HostingIssue::Platform
            });
        }
        thread::sleep(DISCOVERY_INTERVAL.min(deadline.saturating_duration_since(now)));
    }
}

/// Picks console 0's window among the owned `SDL_app` windows.
///
/// The window titled for console 0 wins; a single owned window (a QEMU without `-name` or with
/// `-nodefaults`) is taken as it is.
fn first_console_window(owned: &[WindowHandle]) -> Option<WindowHandle> {
    let single = match owned {
        [only] => Some(*only),
        _ => None,
    };
    owned
        .iter()
        .copied()
        .find(|window| {
            window
                .title()
                .is_ok_and(|title| is_first_console_title(&title))
        })
        .or(single)
}

/// Reports whether an SDL window title names console 0.
///
/// QEMU titles each console's window `QEMU (<name>-<index>)` followed by an optional status such
/// as ` [Stopped]` or ` - Press Ctrl-Alt-G to exit grab` (`ui/sdl2.c`, `sdl_update_caption`).
/// Without `-name` every window is titled `QEMU`, which names no console.
fn is_first_console_title(title: &str) -> bool {
    let Some(rest) = title.strip_prefix("QEMU (") else {
        return false;
    };
    rest.match_indices("-0)").any(|(index, marker)| {
        let status = &rest[index + marker.len()..];
        status.is_empty() || status.starts_with(" [") || status.starts_with(" - ")
    })
}

/// Keeps the `SDL_app` windows, hidden or visible, in enumeration order.
fn sdl_windows(windows: &[WindowHandle]) -> Vec<WindowHandle> {
    windows
        .iter()
        .copied()
        .filter(|window| {
            window
                .class_name()
                .is_ok_and(|class_name| class_name == SDL_WINDOW_CLASS)
        })
        .collect()
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
    fn unattached_calls_send_nothing_and_are_idempotent() {
        let mut host = GuestWindowHost::default();
        assert_eq!(
            host.place(Rect {
                x: 0,
                y: 0,
                width: u32::MAX,
                height: u32::MAX,
            })
            .expect("unattached place"),
            None
        );
        assert_eq!(host.hide().expect("unattached hide"), None);
        assert_eq!(host.show().expect("unattached show"), None);
        host.detach().expect("unattached detach");
        host.to_front().expect("unattached raise");
        assert_eq!(host.resync().expect("unattached resync"), None);
        host.set_overlay_window(None);
        assert!(!host.is_attached());
        assert!(!host.guest_window_alive());
        assert_eq!(host.guest_window(), None);
    }

    #[test]
    fn sdl_window_filter_keeps_nothing_from_nothing() {
        assert!(sdl_windows(&[]).is_empty());
        assert_eq!(first_console_window(&[]), None);
    }

    #[test]
    fn only_console_zero_titles_are_first_console_titles() {
        for title in [
            "QEMU (OME default-0)",
            "QEMU (OME default-0) [Stopped]",
            "QEMU (OME default-0) - Press Ctrl-Alt-G to exit grab",
            "QEMU (OME a-0)b-0)",
        ] {
            assert!(is_first_console_title(title), "{title}");
        }
        for title in [
            "QEMU (OME default-1)",
            "QEMU (OME default-10)",
            "QEMU (OME default-1) [Stopped]",
            "QEMU (OME a-0)b-1)",
            "QEMU",
            "",
            "OME default-0)",
        ] {
            assert!(!is_first_console_title(title), "{title}");
        }
    }

    #[test]
    fn display_geometry_carries_the_screen_rect_and_visibility() {
        let screen = Rect {
            x: -8,
            y: 40,
            width: 1280,
            height: 720,
        };
        assert_eq!(
            display_geometry(screen, true),
            Ok(DisplayWindowGeometry {
                x: -8,
                y: 40,
                width: 1280,
                height: 720,
                visible: true,
            })
        );
        assert_eq!(
            display_geometry(screen, false).map(|geometry| geometry.visible),
            Ok(false)
        );
    }

    #[test]
    fn display_geometry_of_an_empty_rect_is_hidden_at_one_pixel() {
        assert_eq!(
            display_geometry(
                Rect {
                    x: 5,
                    y: 6,
                    width: 0,
                    height: 300,
                },
                true,
            ),
            Ok(DisplayWindowGeometry {
                x: 5,
                y: 6,
                width: 1,
                height: 300,
                visible: false,
            })
        );
    }

    #[test]
    fn display_geometry_rejects_sizes_qemu_cannot_take() {
        assert_eq!(
            display_geometry(
                Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: i32::MAX as u32 + 1,
                },
                true,
            ),
            Err(HostingIssue::Platform)
        );
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
