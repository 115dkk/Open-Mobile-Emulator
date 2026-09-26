// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
use std::sync::{Arc, Mutex};

use ome_runtime::{AppIssue, AppRuntime, AppSnapshot, Command, SettingsInput, StageFit, StageRect};
use tauri::{AppHandle, State};

use crate::{admission::CommandAdmission, events};

#[derive(Clone)]
pub(crate) struct ShellState {
    pub(crate) runtime: Arc<Mutex<Result<AppRuntime, AppIssue>>>,
    admission: CommandAdmission,
}

impl ShellState {
    pub(crate) fn new(runtime: Result<AppRuntime, AppIssue>) -> Self {
        Self {
            runtime: Arc::new(Mutex::new(runtime)),
            admission: CommandAdmission::default(),
        }
    }
}

pub(crate) fn storage_issue() -> AppIssue {
    AppIssue {
        code: "app_storage_unavailable".into(),
        message: "앱 설정을 열지 못했습니다.".into(),
        next_action: Some("앱을 종료한 뒤 다시 실행하십시오.".into()),
    }
}

fn worker_issue() -> AppIssue {
    AppIssue {
        code: "app_worker_unavailable".into(),
        message: "요청한 작업을 마치지 못했습니다.".into(),
        next_action: Some("앱을 종료한 뒤 다시 실행하십시오.".into()),
    }
}

fn busy_issue() -> AppIssue {
    AppIssue {
        code: "app_busy".into(),
        message: "앱이 다른 작업을 처리하는 중입니다.".into(),
        next_action: Some("잠시 후 다시 시도하십시오.".into()),
    }
}

pub(crate) async fn apply(
    app: AppHandle,
    state: &ShellState,
    command: Command,
) -> Result<AppSnapshot, AppIssue> {
    let lease = state.admission.try_enter().ok_or_else(busy_issue)?;
    let runtime = Arc::clone(&state.runtime);
    tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        let mut guard = runtime.lock().map_err(|_| worker_issue())?;
        let snapshot = match &mut *guard {
            Ok(runtime) => runtime.apply(command)?,
            Err(issue) => return Err(issue.clone()),
        };
        // Emit before dropping the lease so a following command cannot publish
        // its snapshot ahead of this one.
        events::snapshot(&app, &snapshot);
        Ok(snapshot)
    })
    .await
    .map_err(|error| {
        eprintln!("native command task failed: {error}");
        worker_issue()
    })?
}

#[tauri::command]
pub(crate) async fn app_snapshot(state: State<'_, ShellState>) -> Result<AppSnapshot, AppIssue> {
    let runtime = Arc::clone(&state.runtime);
    tauri::async_runtime::spawn_blocking(move || {
        let guard = runtime.lock().map_err(|_| worker_issue())?;
        match &*guard {
            Ok(runtime) => Ok(runtime.snapshot()),
            Err(issue) => Err(issue.clone()),
        }
    })
    .await
    .map_err(|error| {
        eprintln!("snapshot task failed: {error}");
        worker_issue()
    })?
}

#[tauri::command]
pub(crate) async fn host_check_refresh(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::HostCheckRefresh).await
}

#[tauri::command]
pub(crate) async fn wizard_continue(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::WizardContinue).await
}

#[tauri::command]
pub(crate) async fn wizard_skip(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::WizardSkip).await
}

#[tauri::command]
pub(crate) async fn wizard_restart(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::WizardRestart).await
}

#[tauri::command]
pub(crate) async fn whpx_enable(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::WhpxEnable).await
}

#[tauri::command]
pub(crate) async fn artifact_download_start(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::ArtifactDownloadStart).await
}

#[tauri::command]
pub(crate) async fn artifact_download_cancel(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::ArtifactDownloadCancel).await
}

#[tauri::command]
pub(crate) async fn guest_disk_create(
    app: AppHandle,
    state: State<'_, ShellState>,
    size_gib: u32,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestDiskCreate { size_gib }).await
}

#[tauri::command]
pub(crate) async fn guest_start(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestStart).await
}

#[tauri::command]
pub(crate) async fn guest_stop(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestStop).await
}

#[tauri::command]
pub(crate) async fn guest_restart(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestRestart).await
}

#[tauri::command]
pub(crate) async fn stage_rect_changed(
    app: AppHandle,
    state: State<'_, ShellState>,
    rect: StageRect,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::StageRectChanged { rect }).await
}

#[tauri::command]
pub(crate) async fn screenshot_save(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::ScreenshotSave).await
}

#[tauri::command]
pub(crate) async fn app_install_pick(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::AppInstallPick).await
}

#[tauri::command]
pub(crate) async fn app_uninstall(
    app: AppHandle,
    state: State<'_, ShellState>,
    package: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::AppUninstall { package }).await
}

#[tauri::command]
pub(crate) async fn app_launch(
    app: AppHandle,
    state: State<'_, ShellState>,
    package: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::AppLaunch { package }).await
}

#[tauri::command]
pub(crate) async fn keymap_set_active(
    app: AppHandle,
    state: State<'_, ShellState>,
    id: Option<String>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::KeymapSetActive { id }).await
}

#[tauri::command]
pub(crate) async fn keymap_set_enabled(
    app: AppHandle,
    state: State<'_, ShellState>,
    enabled: bool,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::KeymapSetEnabled { enabled }).await
}

#[tauri::command]
pub(crate) async fn keymap_delete(
    app: AppHandle,
    state: State<'_, ShellState>,
    id: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::KeymapDelete { id }).await
}

#[tauri::command]
pub(crate) async fn display_preset_apply(
    app: AppHandle,
    state: State<'_, ShellState>,
    id: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::DisplayPresetApply { id }).await
}

#[tauri::command]
pub(crate) async fn stage_fit_set(
    app: AppHandle,
    state: State<'_, ShellState>,
    fit: StageFit,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::StageFitSet { fit }).await
}

#[tauri::command]
pub(crate) async fn settings_save(
    app: AppHandle,
    state: State<'_, ShellState>,
    settings: SettingsInput,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::SettingsSave { settings }).await
}

#[tauri::command]
pub(crate) async fn update_check(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::UpdateCheck).await
}

#[tauri::command]
pub(crate) async fn update_install(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::UpdateInstall).await
}

#[tauri::command]
pub(crate) async fn diagnostics_export(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::DiagnosticsExport).await
}

#[tauri::command]
pub(crate) async fn open_logs_folder(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::OpenLogsFolder).await
}

#[tauri::command]
pub(crate) async fn open_screenshots_folder(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::OpenScreenshotsFolder).await
}

#[tauri::command]
pub(crate) async fn open_registration_page(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::OpenRegistrationPage).await
}

#[tauri::command]
pub(crate) async fn guest_window_to_front(
    app: AppHandle,
    state: State<'_, ShellState>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestWindowToFront).await
}
