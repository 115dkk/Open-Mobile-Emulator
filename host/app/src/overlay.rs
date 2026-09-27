// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Native owner window for the input markers and editor.

use std::sync::Mutex;

use ome_overlay::{OverlayMode, OverlayPlacement, PhysicalRect, overlay_mode, placement};
use ome_runtime::{AppSnapshot, HostingMode, Rect, StageRect};
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow};

use crate::{commands::ShellState, window::local_navigation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppliedPlacement {
    placement: OverlayPlacement,
    owner_suppressed: bool,
}

#[derive(Debug, Default)]
struct OverlayState {
    window: Option<WebviewWindow>,
    last_applied: Option<AppliedPlacement>,
    stage_rect: Option<StageRect>,
}

/// Owns the lazy overlay window and its last native placement.
pub(crate) struct OverlayWindow {
    app: tauri::AppHandle,
    main: WebviewWindow,
    state: Mutex<OverlayState>,
}

impl std::fmt::Debug for OverlayWindow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OverlayWindow")
            .field("main", &self.main.label())
            .finish_non_exhaustive()
    }
}

impl OverlayWindow {
    fn new(app: tauri::AppHandle, main: WebviewWindow) -> Self {
        Self {
            app,
            main,
            state: Mutex::new(OverlayState::default()),
        }
    }

    fn create_window(&self) -> tauri::Result<WebviewWindow> {
        create_overlay_window(
            &self.app,
            &self.main,
            WebviewUrl::App("overlay.html".into()),
        )
    }

    /// Applies one complete placement, skipping an unchanged native state.
    pub(crate) fn apply(&self, placement: &OverlayPlacement) {
        if let Err(error) = self.try_apply(placement) {
            eprintln!("overlay window could not be updated: {error}");
        }
    }

    fn try_apply(&self, placement: &OverlayPlacement) -> tauri::Result<()> {
        let owner_suppressed = self.main.is_minimized()? || !self.main.is_visible()?;
        let next = AppliedPlacement {
            placement: *placement,
            owner_suppressed,
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| tauri::Error::Io(std::io::Error::other("overlay state is poisoned")))?;
        if state.last_applied == Some(next) {
            return Ok(());
        }
        let previous_mode = state
            .last_applied
            .filter(|last| !last.owner_suppressed && last.placement.rect.is_some())
            .map(|last| last.placement.mode);
        let should_show =
            !owner_suppressed && placement.mode != OverlayMode::Hidden && placement.rect.is_some();
        if !should_show {
            if let Some(window) = state.window.as_ref() {
                window.hide()?;
            }
            if previous_mode == Some(OverlayMode::Editing)
                && placement.mode != OverlayMode::Editing
                && !owner_suppressed
            {
                self.main.set_focus()?;
            }
            state.last_applied = Some(next);
            return Ok(());
        }

        let window = match state.window.clone() {
            Some(window) => window,
            None => {
                let window = self.create_window()?;
                state.window = Some(window.clone());
                window
            }
        };
        apply_native(&window, &self.main, previous_mode, placement)?;
        state.last_applied = Some(next);
        Ok(())
    }

    fn remember_stage_rect(&self, rect: StageRect) {
        if let Ok(mut state) = self.state.lock() {
            state.stage_rect = Some(rect);
        }
    }

    fn fallback_rect(&self) -> Option<PhysicalRect> {
        let stage = self.state.lock().ok()?.stage_rect?;
        let relative = ome_window_host::StageGeometry::physical(&ome_window_host::StageRect {
            x: stage.x,
            y: stage.y,
            width: stage.width,
            height: stage.height,
            scale_factor: stage.scale_factor,
        });
        let origin = self.main.inner_position().ok()?;
        Some(PhysicalRect {
            x: origin.x.saturating_add(relative.x),
            y: origin.y.saturating_add(relative.y),
            width: relative.width,
            height: relative.height,
        })
    }

    fn sync(&self, snapshot: &AppSnapshot, guest_rect: Option<Rect>) {
        let mode = overlay_mode(snapshot);
        let exact = guest_rect.map(to_physical_rect);
        let rect = if snapshot.guest.hosting == HostingMode::Embedded {
            exact.or_else(|| self.fallback_rect())
        } else {
            None
        };
        self.apply(&placement(mode, rect));
    }
}

fn create_overlay_window<R: tauri::Runtime, M: Manager<R>>(
    app: &M,
    main: &WebviewWindow<R>,
    url: WebviewUrl,
) -> tauri::Result<WebviewWindow<R>> {
    tauri::WebviewWindowBuilder::new(app, "overlay", url)
        .owner(main)?
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .always_on_top(false)
        .accept_first_mouse(true)
        .on_navigation(local_navigation)
        .build()
}

fn apply_native<R: tauri::Runtime>(
    window: &WebviewWindow<R>,
    main: &WebviewWindow<R>,
    previous_mode: Option<OverlayMode>,
    placement: &OverlayPlacement,
) -> tauri::Result<()> {
    let rect = placement
        .rect
        .expect("visible overlay placement always has a rectangle");
    window.set_position(PhysicalPosition::new(rect.x, rect.y))?;
    window.set_size(PhysicalSize::new(rect.width, rect.height))?;
    window.set_ignore_cursor_events(placement.mode != OverlayMode::Editing)?;
    window.show()?;
    if placement.mode == OverlayMode::Editing && previous_mode != Some(OverlayMode::Editing) {
        window.set_focus()?;
    } else if placement.mode != OverlayMode::Editing && previous_mode == Some(OverlayMode::Editing)
    {
        main.set_focus()?;
    }
    Ok(())
}

fn to_physical_rect(rect: Rect) -> PhysicalRect {
    PhysicalRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

pub(crate) fn install(app: &tauri::App) -> tauri::Result<()> {
    let main = app
        .get_webview_window("main")
        .ok_or_else(|| tauri::Error::Io(std::io::Error::other("main window is missing")))?;
    app.manage(OverlayWindow::new(app.handle().clone(), main));
    Ok(())
}

pub(crate) fn snapshot(app: &tauri::AppHandle, snapshot: &AppSnapshot, guest_rect: Option<Rect>) {
    app.state::<OverlayWindow>().sync(snapshot, guest_rect);
}

pub(crate) fn remember_stage_rect(app: &tauri::AppHandle, rect: StageRect) {
    app.state::<OverlayWindow>().remember_stage_rect(rect);
}

pub(crate) fn window(app: &tauri::AppHandle) -> Option<WebviewWindow> {
    app.state::<OverlayWindow>()
        .state
        .lock()
        .ok()?
        .window
        .clone()
}

pub(crate) fn refresh(app: &tauri::AppHandle) {
    let shell = app.state::<ShellState>();
    let values = shell.runtime.lock().ok().and_then(|guard| {
        let runtime = guard.as_ref().ok()?;
        Some((runtime.snapshot(), runtime.guest_client_screen_rect()))
    });
    if let Some((snapshot, guest_rect)) = values {
        app.state::<OverlayWindow>().sync(&snapshot, guest_rect);
    }
}
