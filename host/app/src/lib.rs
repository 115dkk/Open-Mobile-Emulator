// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
#![forbid(unsafe_code)]

mod admission;
mod commands;
mod events;
mod tray;
mod window;

use ome_runtime::{AppIssue, AppRuntime, OmeHome, RuntimeDeps};
use tauri::Manager;

fn initialize_runtime() -> Result<AppRuntime, AppIssue> {
    let home = OmeHome::from_path(OmeHome::resolve()).map_err(|error| {
        eprintln!("OME home resolution failed: {error}");
        commands::storage_issue()
    })?;
    #[cfg(windows)]
    let probe = Box::new(ome_runtime::WindowsProbe);
    #[cfg(not(windows))]
    let probe = Box::new(ome_runtime::TableProbe::new());
    AppRuntime::open(
        home,
        RuntimeDeps {
            probe,
            artifacts: None,
            adb: None,
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            window::show_main(app);
        }))
        .setup(|app| {
            // Keep an open failure intact so app_snapshot reports it to the UI.
            app.manage(commands::ShellState::new(initialize_runtime()));
            window::install(app)?;
            tray::install(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_snapshot,
            commands::host_check_refresh,
            commands::wizard_continue,
            commands::wizard_skip,
            commands::wizard_restart,
            commands::whpx_enable,
            commands::artifact_download_start,
            commands::artifact_download_cancel,
            commands::guest_disk_create,
            commands::guest_start,
            commands::guest_stop,
            commands::guest_restart,
            commands::stage_rect_changed,
            commands::screenshot_save,
            commands::app_install_pick,
            commands::app_uninstall,
            commands::app_launch,
            commands::keymap_set_active,
            commands::keymap_set_enabled,
            commands::keymap_delete,
            commands::display_preset_apply,
            commands::stage_fit_set,
            commands::settings_save,
            commands::update_check,
            commands::update_install,
            commands::diagnostics_export,
            commands::open_logs_folder,
            commands::open_screenshots_folder,
            commands::open_registration_page,
            commands::guest_window_to_front,
        ])
        .run(tauri::generate_context!())
        .expect("Open Mobile Emulator native shell failed");
}
