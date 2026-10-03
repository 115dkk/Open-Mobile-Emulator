// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
use std::sync::atomic::{AtomicBool, Ordering};

use ome_runtime::{CloseAction, GuestState};
use tauri::Manager;

use crate::commands::ShellState;

#[cfg(windows)]
thread_local! {
    static DPI_HOSTING_GUARD: std::cell::RefCell<Option<ome_platform_win::DpiHostingGuard>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(windows)]
pub(crate) fn keep_dpi_guard(guard: ome_platform_win::DpiHostingGuard) {
    DPI_HOSTING_GUARD.with(|slot| slot.replace(Some(guard)));
}

pub(crate) fn local_navigation(url: &tauri::Url) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    match (url.scheme(), url.host_str()) {
        ("tauri", Some("localhost")) | ("http" | "https", Some("tauri.localhost")) => {
            url.port().is_none()
        }
        ("http", Some("127.0.0.1")) if cfg!(debug_assertions) => url.port() == Some(1420),
        _ => false,
    }
}

pub(crate) fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window
            .show()
            .and_then(|()| window.unminimize())
            .and_then(|()| window.set_focus())
        {
            eprintln!("main window could not be shown: {error}");
        }
        refresh_overlay(app);
    }
}

fn refresh_overlay(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || crate::overlay::refresh(&app));
}

/// Set while a follow task is queued; window events arriving before it runs share that task.
static MOVE_PENDING: AtomicBool = AtomicBool::new(false);

/// Lets the guest window follow the host window without blocking the window thread.
///
/// The window thread must never wait for the runtime lock. A worker holding that lock calls
/// `SetWindowPos` on the guest popup or the overlay, both owned by this thread, and Windows makes
/// that call wait until this thread handles the resulting messages; waiting here deadlocked the
/// installed product on 2026-10-03 (the window stopped responding with the guest still running).
fn host_window_moved(app: &tauri::AppHandle) {
    if MOVE_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Cleared before the move is applied, so an event arriving during it queues another pass.
        MOVE_PENDING.store(false, Ordering::Release);
        let state = app.state::<ShellState>();
        if let Ok(mut guard) = state.runtime.lock()
            && let Ok(runtime) = &mut *guard
        {
            runtime.host_window_moved();
        }
    });
}

/// Decides between hiding to the tray and exiting off the window thread (see `host_window_moved`).
fn close_requested(window: &tauri::WebviewWindow) {
    let window = window.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = window.state::<ShellState>();
        let hide = match state.runtime.lock() {
            Ok(guard) => match &*guard {
                Ok(runtime) => {
                    let snapshot = runtime.snapshot();
                    snapshot.settings.close_action == CloseAction::MinimizeToTray
                        && snapshot.guest.state != GuestState::Stopped
                }
                Err(_) => false,
            },
            Err(_) => false,
        };
        if hide {
            if let Err(error) = window.hide() {
                eprintln!("main window could not be hidden: {error}");
            }
            refresh_overlay(window.app_handle());
        } else {
            crate::commands::request_app_exit(window.app_handle(), &state);
        }
    });
}

pub(crate) fn install(app: &tauri::App) -> tauri::Result<()> {
    let config = app.config().app.windows.first().ok_or_else(|| {
        tauri::Error::Io(std::io::Error::other(
            "main window configuration is missing",
        ))
    })?;
    let window = tauri::WebviewWindowBuilder::from_config(app, config)?
        .on_navigation(local_navigation)
        .devtools(cfg!(debug_assertions) && std::env::var_os("OME_DEVTOOLS").is_some())
        .build()?;
    let handle = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // Both outcomes keep the window: it is hidden, or the app exits after the guest stops.
            api.prevent_close();
            close_requested(&handle);
        }
        tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
            let paths = paths.clone();
            let app = handle.app_handle().clone();
            let shell = app.state::<ShellState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(issue) = crate::commands::install_native_paths(app, &shell, paths).await
                {
                    eprintln!("native file drop installation failed: {}", issue.code);
                }
            });
        }
        tauri::WindowEvent::Moved(_)
        | tauri::WindowEvent::Resized(_)
        | tauri::WindowEvent::ScaleFactorChanged { .. }
        | tauri::WindowEvent::Focused(_) => {
            host_window_moved(handle.app_handle());
            refresh_overlay(handle.app_handle());
        }
        _ => {}
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::local_navigation;

    #[test]
    fn accepts_only_the_packaged_origins_and_debug_server() {
        for url in [
            "tauri://localhost/",
            "http://tauri.localhost/",
            "https://tauri.localhost/index.html",
        ] {
            assert!(local_navigation(&tauri::Url::parse(url).expect("test URL")));
        }
        assert_eq!(
            local_navigation(&tauri::Url::parse("http://127.0.0.1:1420/").expect("test URL")),
            cfg!(debug_assertions)
        );
        for url in [
            "https://example.com/",
            "http://localhost:1420/",
            "http://127.0.0.1:1421/",
            "https://127.0.0.1:1420/",
            "tauri://other/",
            "http://tauri.localhost:1420/",
            "http://user@127.0.0.1:1420/",
            "https://user:password@tauri.localhost/",
            "file:///C:/page.html",
        ] {
            assert!(
                !local_navigation(&tauri::Url::parse(url).expect("test URL")),
                "{url}"
            );
        }
    }
}
