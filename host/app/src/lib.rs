// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
#![forbid(unsafe_code)]

mod admission;
mod commands;
mod desktop;
mod events;
mod overlay;
#[cfg(debug_assertions)]
mod overlay_check;
mod setup;
mod tray;
mod window;

use std::sync::Arc;
use std::time::Duration;

use ome_adb::{AdbSession, ProcessRunner};
use ome_artifacts::{ArtifactStore, Manifest, UreqFetch};
use ome_runtime::{AdbPowerOff, AppIssue, AppRuntime, Desktop, OmeHome, RuntimeDeps, WorkerDeps};
#[cfg(windows)]
use ome_supervisor::windows_adapter::WindowsProcessAdapter;
use ome_supervisor::{Supervisor, SupervisorPolicy, TcpQmpFactory};
use tauri::Manager;

use crate::desktop::WindowsDesktop;
use crate::setup::WindowsElevation;

fn initialize_runtime(manifest_root: &std::path::Path) -> Result<AppRuntime, AppIssue> {
    let home = OmeHome::from_path(OmeHome::resolve()).map_err(|error| {
        eprintln!("OME home resolution failed: {error}");
        commands::storage_issue()
    })?;
    #[cfg(windows)]
    let probe = ome_runtime::WindowsProbe;
    #[cfg(not(windows))]
    let probe = ome_runtime::TableProbe::new();
    let adb = ome_host_adb(&probe);
    let desktop: Box<dyn Desktop> = Box::new(WindowsDesktop::default());
    #[cfg(windows)]
    let supervisor_policy = adb
        .as_ref()
        .map_or_else(SupervisorPolicy::default, |session| SupervisorPolicy {
            power_off_hook: Arc::new(AdbPowerOff::new(session.clone())),
            ..SupervisorPolicy::default()
        });
    #[cfg(windows)]
    let supervisor = Some(Box::new(Supervisor::new(
        WindowsProcessAdapter,
        TcpQmpFactory,
        home.subdir("logs").map_err(|error| {
            eprintln!("OME log directory resolution failed: {error}");
            commands::storage_issue()
        })?,
        supervisor_policy,
    )) as Box<dyn ome_runtime::GuestProcess>);
    #[cfg(not(windows))]
    let supervisor = None;
    let manifest = Manifest::load(manifest_root.join("artifacts.json")).map_err(|error| {
        eprintln!("artifact manifest loading failed: {error}");
        commands::storage_issue()
    })?;
    let artifact_dir = home.subdir("artifacts").map_err(|error| {
        eprintln!("artifact directory resolution failed: {error}");
        commands::storage_issue()
    })?;
    let mut runtime = AppRuntime::open(
        home,
        RuntimeDeps {
            probe: Box::new(probe),
            artifacts: Some(ArtifactStore::new(
                manifest,
                artifact_dir,
                Box::new(UreqFetch::new()),
            )),
            adb,
            supervisor,
            desktop,
            window_host: Box::new(ome_window_host::GuestWindowHost::default()),
            family_adapter: None,
            images_dir: Some(manifest_root.join("images")),
            artifacts_manifest: Some(manifest_root.join("artifacts.json")),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )?;
    let helper = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|directory| directory.join("ome-setup.exe"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("ome-setup.exe"));
    runtime.set_worker_deps(WorkerDeps {
        elevation: std::sync::Arc::new(WindowsElevation::new(helper)),
        ..WorkerDeps::default()
    });
    Ok(runtime)
}

fn ome_host_adb(probe: &dyn ome_host_check::HostProbe) -> Option<AdbSession> {
    let found = probe.adb().ok().flatten()?;
    AdbSession::new(
        found.program,
        "127.0.0.1:5555".to_owned(),
        Box::new(ProcessRunner),
    )
    .ok()
}

fn start_event_pump(app: tauri::AppHandle, shell: commands::ShellState) {
    let receiver = shell.runtime.lock().ok().and_then(|guard| {
        guard
            .as_ref()
            .ok()
            .and_then(AppRuntime::subscribe_guest_events)
    });
    let Some(receiver) = receiver else { return };
    if let Err(error) = std::thread::Builder::new()
        .name("ome-runtime-events".to_owned())
        .spawn(move || {
            loop {
                let event = receiver.recv_timeout(Duration::from_secs(1));
                let Ok(mut guard) = shell.runtime.lock() else {
                    break;
                };
                let Ok(runtime) = &mut *guard else {
                    break;
                };
                let before = runtime.snapshot();
                match event {
                    Ok(event) => runtime.ingest_guest_event(event),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => runtime.tick(),
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
                let update_exit = runtime.take_update_exit_requested();
                let after = runtime.snapshot();
                let guest_rect = runtime.guest_client_screen_rect();
                let stage_visible = runtime.stage_visible();
                drop(guard);
                if update_exit {
                    commands::request_app_exit(&app, &shell);
                }
                if after != before {
                    events::snapshot(&app, &after, guest_rect, stage_visible);
                    tray::update_power_label(&app, after.guest.state);
                }
                let exit_requested = shell
                    .exit_after_stop
                    .lock()
                    .ok()
                    .and_then(|deadline| *deadline);
                if exit_requested.is_some_and(|requested| {
                    matches!(
                        after.guest.state,
                        ome_runtime::GuestState::Stopped | ome_runtime::GuestState::Failed
                    ) || requested.elapsed() >= Duration::from_secs(40)
                }) {
                    app.exit(0);
                    break;
                }
            }
        })
    {
        eprintln!("runtime event pump could not start: {error}");
    }
}

/// Starts the native Open Mobile Emulator shell.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            window::show_main(app);
        }))
        .setup(|app| {
            // Mixed hosting is thread-affine, so keep the guard on Tauri's window thread.
            #[cfg(windows)]
            let dpi_guard = ome_platform_win::set_thread_dpi_hosting_mixed()
                .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
            let manifest_root = if cfg!(debug_assertions) {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../manifests")
            } else {
                app.path().resource_dir()?.join("manifests")
            };
            let shell = commands::ShellState::new(initialize_runtime(&manifest_root));
            app.manage(shell.clone());
            window::install(app)?;
            overlay::install(app)?;
            #[cfg(windows)]
            if let Some(main) = app.get_webview_window("main") {
                let raw = main.hwnd()?.0 as usize as u64;
                if let Ok(mut guard) = shell.runtime.lock()
                    && let Ok(runtime) = &mut *guard
                {
                    runtime.set_host_window(raw);
                    if let Err(issue) = runtime.start_auto_update_check() {
                        eprintln!("automatic update check could not start: {}", issue.code);
                    }
                }
            }
            tray::install(app)?;
            #[cfg(windows)]
            window::keep_dpi_guard(dpi_guard);
            start_event_pump(app.handle().clone(), shell.clone());
            #[cfg(debug_assertions)]
            if overlay_check::requested() {
                overlay_check::start(app.handle().clone(), shell);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_snapshot,
            commands::host_check_refresh,
            commands::wizard_continue,
            commands::wizard_skip,
            commands::wizard_defer,
            commands::open_help,
            commands::app_quit,
            commands::whpx_enable,
            commands::artifact_download_start,
            commands::artifact_download_cancel,
            commands::guest_image_select,
            commands::guest_create,
            commands::guest_select,
            commands::guest_delete,
            commands::guest_reinstall,
            commands::guest_start,
            commands::guest_stop,
            commands::guest_restart,
            commands::guest_root_set,
            commands::guest_volume_set,
            commands::stage_rect_changed,
            commands::stage_hidden,
            commands::screenshot_save,
            commands::app_install_pick,
            commands::app_install_cancel,
            commands::app_uninstall,
            commands::app_launch,
            commands::input_profile_select,
            commands::input_suspend_toggle,
            commands::input_overlay_toggle,
            commands::input_profile_delete,
            commands::input_profile_save,
            commands::input_binding_upsert,
            commands::input_binding_remove,
            commands::input_editor_toggle,
            commands::input_auto_apply_set,
            commands::input_suspend_hotkey_set,
            commands::display_preset_apply,
            commands::display_custom_apply,
            commands::display_refresh_set,
            commands::display_vsync_set,
            commands::stage_fit_set,
            commands::settings_save,
            commands::update_check,
            commands::update_install,
            commands::diagnostics_export,
            commands::open_logs_folder,
            commands::open_screenshots_folder,
            commands::open_home_folder,
            commands::copy_to_clipboard,
            commands::open_registration_page,
            commands::google_account_add_open,
            commands::guest_window_to_front,
        ])
        .run(tauri::generate_context!())
        .expect("Open Mobile Emulator native shell failed");
}
