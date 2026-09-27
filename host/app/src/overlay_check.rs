// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Deferred interactive measurement driver for the ignored real overlay check.

use std::time::{Duration, Instant};

use ome_overlay::{PhysicalRect, overlay_mode, placement};
use ome_runtime::{Command, GuestState, StageRect};
use tauri::Manager;

use crate::commands::ShellState;

const DEADLINE: Duration = Duration::from_secs(180);

pub(crate) fn requested() -> bool {
    std::env::args_os().any(|argument| argument == "--real-overlay-window-check")
}

pub(crate) fn start(app: tauri::AppHandle, shell: ShellState) {
    let worker_app = app.clone();
    let result = std::thread::Builder::new()
        .name("ome-real-overlay-check".to_owned())
        .spawn(move || {
            let outcome = run(&worker_app, &shell);
            let stopped = stop_guest(&shell);
            let code = match (outcome, stopped) {
                (Ok(()), Ok(())) => 0,
                (Err(error), Ok(())) => {
                    eprintln!("overlay_check.error={error}");
                    1
                }
                (Ok(()), Err(error)) => {
                    eprintln!("overlay_check.error=cleanup failed: {error}");
                    1
                }
                (Err(error), Err(cleanup)) => {
                    eprintln!("overlay_check.error={error}; cleanup failed: {cleanup}");
                    1
                }
            };
            worker_app.exit(code);
        });
    if let Err(error) = result {
        eprintln!("overlay_check.error=worker start failed: {error}");
        app.exit(1);
    }
}

fn run(app: &tauri::AppHandle, shell: &ShellState) -> Result<(), String> {
    println!("overlay_check.started_unix_ms={}", unix_millis());
    println!("overlay_check.preflight_qemu_process_count=0 checked by ignored test");
    let profile_id = {
        let guard = shell.runtime.lock().map_err(|_| "runtime lock poisoned")?;
        let runtime = guard.as_ref().map_err(|issue| issue.code.clone())?;
        runtime
            .snapshot()
            .input
            .profiles
            .first()
            .map(|profile| profile.id.clone())
            .ok_or("no input profile")?
    };
    apply(
        shell,
        Command::GuestSelect {
            id: "default".to_owned(),
        },
    )?;
    apply(
        shell,
        Command::InputProfileSelect {
            id: Some(profile_id),
        },
    )?;
    apply(shell, Command::GuestStart)?;
    wait_for(shell, |state| state == GuestState::Running, "running")?;

    let main = app
        .get_webview_window("main")
        .ok_or("main window is missing")?;
    let size = main.inner_size().map_err(|error| error.to_string())?;
    let scale = main.scale_factor().map_err(|error| error.to_string())?;
    let snapshot = apply(
        shell,
        Command::StageRectChanged {
            rect: StageRect {
                x: 0.0,
                y: 0.0,
                width: f64::from(size.width) / scale,
                height: f64::from(size.height) / scale,
                scale_factor: scale,
            },
        },
    )?;
    let guest = wait_for_guest_rect(shell)?;
    let rect = PhysicalRect {
        x: guest.x,
        y: guest.y,
        width: guest.width,
        height: guest.height,
    };
    println!(
        "guest.client_rect={},{},{}x{}",
        rect.x, rect.y, rect.width, rect.height
    );
    if !snapshot.input.overlay_visible {
        apply(shell, Command::InputOverlayToggle)?;
    }

    let editing_snapshot = apply(shell, Command::InputEditorToggle)?;
    let editing = placement(overlay_mode(&editing_snapshot), Some(rect));
    if editing.mode != ome_overlay::OverlayMode::Editing || editing.rect != Some(rect) {
        return Err(format!("unexpected editing placement: {editing:?}"));
    }
    let editing_started = Instant::now();
    crate::overlay::snapshot(app, &editing_snapshot, Some(guest));
    let window = wait_for_overlay(app)?;
    let editing_rect = record_window("editing", &window, editing_started)?;
    if editing_rect != rect {
        return Err(format!(
            "overlay and guest client rectangles differ: overlay={editing_rect:?}, guest={rect:?}"
        ));
    }
    println!("editing.ignore_cursor_events=false requested; raw style was not queried");

    let showing_snapshot = apply(shell, Command::InputEditorToggle)?;
    let showing = placement(overlay_mode(&showing_snapshot), Some(rect));
    if showing.mode != ome_overlay::OverlayMode::Showing || showing.rect != Some(rect) {
        return Err(format!("unexpected showing placement: {showing:?}"));
    }
    let showing_started = Instant::now();
    crate::overlay::snapshot(app, &showing_snapshot, Some(guest));
    record_window("showing", &window, showing_started)?;
    println!("showing.ignore_cursor_events=true requested; raw style was not queried");

    let hidden_snapshot = apply(shell, Command::InputOverlayToggle)?;
    let hidden = placement(overlay_mode(&hidden_snapshot), Some(rect));
    if hidden.mode != ome_overlay::OverlayMode::Hidden || hidden.rect.is_some() {
        return Err(format!("unexpected hidden placement: {hidden:?}"));
    }
    let hidden_started = Instant::now();
    crate::overlay::snapshot(app, &hidden_snapshot, Some(guest));
    println!(
        "overlay_toggle.apply_to_hidden_ms={}",
        hidden_started.elapsed().as_millis()
    );
    let hidden_visible = window.is_visible().map_err(|error| error.to_string())?;
    println!("overlay_toggle.visible={hidden_visible}");
    if hidden_visible {
        return Err("overlay remained visible after InputOverlayToggle".to_owned());
    }

    let before_stop = apply(shell, Command::InputOverlayToggle)?;
    let before_stop_placement = placement(overlay_mode(&before_stop), Some(rect));
    if before_stop_placement.mode != ome_overlay::OverlayMode::Showing {
        return Err(format!(
            "overlay did not return to showing before stop: {before_stop_placement:?}"
        ));
    }
    crate::overlay::snapshot(app, &before_stop, Some(guest));
    if !window.is_visible().map_err(|error| error.to_string())? {
        return Err("overlay was hidden before GuestStop".to_owned());
    }

    let stop_started = Instant::now();
    stop_guest(shell)?;
    let stopped = {
        let guard = shell.runtime.lock().map_err(|_| "runtime lock poisoned")?;
        let runtime = guard.as_ref().map_err(|issue| issue.code.clone())?;
        runtime.snapshot()
    };
    crate::overlay::snapshot(app, &stopped, None);
    println!(
        "guest_stop.to_stopped_ms={}",
        stop_started.elapsed().as_millis()
    );
    let stopped_visible = window.is_visible().map_err(|error| error.to_string())?;
    println!("guest_stop.overlay_visible={stopped_visible}");
    if stopped_visible {
        return Err("overlay remained visible after GuestStop".to_owned());
    }
    println!("overlay_check.finished_unix_ms={}", unix_millis());
    Ok(())
}

fn stop_guest(shell: &ShellState) -> Result<(), String> {
    let state = {
        let guard = shell.runtime.lock().map_err(|_| "runtime lock poisoned")?;
        guard
            .as_ref()
            .map_err(|issue| issue.code.clone())?
            .snapshot()
            .guest
            .state
    };
    if matches!(state, GuestState::Stopped | GuestState::Failed) {
        return Ok(());
    }
    apply(shell, Command::GuestStop)?;
    wait_for(
        shell,
        |state| matches!(state, GuestState::Stopped | GuestState::Failed),
        "stopped",
    )
}

fn apply(shell: &ShellState, command: Command) -> Result<ome_runtime::AppSnapshot, String> {
    let mut guard = shell.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let runtime = guard.as_mut().map_err(|issue| issue.code.clone())?;
    runtime.apply(command).map_err(|issue| issue.code)
}

fn wait_for(
    shell: &ShellState,
    expected: impl Fn(GuestState) -> bool,
    name: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let state = {
            let guard = shell.runtime.lock().map_err(|_| "runtime lock poisoned")?;
            guard
                .as_ref()
                .map_err(|issue| issue.code.clone())?
                .snapshot()
                .guest
                .state
        };
        if expected(state) {
            return Ok(());
        }
        if state == GuestState::Failed || Instant::now() >= deadline {
            return Err(format!("guest did not reach {name}: {state:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_guest_rect(shell: &ShellState) -> Result<ome_runtime::Rect, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let rect = {
            let guard = shell.runtime.lock().map_err(|_| "runtime lock poisoned")?;
            guard
                .as_ref()
                .map_err(|issue| issue.code.clone())?
                .guest_client_screen_rect()
        };
        if let Some(rect) = rect {
            return Ok(rect);
        }
        if Instant::now() >= deadline {
            return Err("guest client rectangle is unavailable".to_owned());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_overlay(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(window) = crate::overlay::window(app) {
            return Ok(window);
        }
        if Instant::now() >= deadline {
            return Err("overlay window was not created".to_owned());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn record_window(
    prefix: &str,
    window: &tauri::WebviewWindow,
    started: Instant,
) -> Result<PhysicalRect, String> {
    let position = window.inner_position().map_err(|error| error.to_string())?;
    let size = window.inner_size().map_err(|error| error.to_string())?;
    println!(
        "{prefix}.apply_to_visible_ms={}",
        started.elapsed().as_millis()
    );
    println!(
        "{prefix}.client_rect={},{},{}x{}",
        position.x, position.y, size.width, size.height
    );
    let visible = window.is_visible().map_err(|error| error.to_string())?;
    println!("{prefix}.visible={visible}");
    if !visible {
        return Err(format!("overlay is hidden in {prefix} mode"));
    }
    Ok(PhysicalRect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
