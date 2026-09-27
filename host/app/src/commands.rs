// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
use std::sync::{Arc, Mutex};

use ome_runtime::{
    AppIssue, AppRuntime, AppSnapshot, Binding, Command, InputProfile, SettingsInput, Size,
    StageFit, StageRect, VsyncMode,
};
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

macro_rules! command_no_args {
    ($name:ident, $variant:expr) => {
        #[tauri::command]
        pub(crate) async fn $name(
            app: AppHandle,
            state: State<'_, ShellState>,
        ) -> Result<AppSnapshot, AppIssue> {
            apply(app, &state, $variant).await
        }
    };
}

command_no_args!(host_check_refresh, Command::HostCheckRefresh);
command_no_args!(wizard_continue, Command::WizardContinue);
command_no_args!(wizard_skip, Command::WizardSkip);
command_no_args!(whpx_enable, Command::WhpxEnable);
command_no_args!(artifact_download_start, Command::ArtifactDownloadStart);
command_no_args!(artifact_download_cancel, Command::ArtifactDownloadCancel);
command_no_args!(guest_start, Command::GuestStart);
command_no_args!(guest_stop, Command::GuestStop);
command_no_args!(guest_restart, Command::GuestRestart);
command_no_args!(screenshot_save, Command::ScreenshotSave);
command_no_args!(app_install_pick, Command::AppInstallPick);
command_no_args!(input_suspend_toggle, Command::InputSuspendToggle);
command_no_args!(input_editor_toggle, Command::InputEditorToggle);
command_no_args!(update_check, Command::UpdateCheck);
command_no_args!(update_install, Command::UpdateInstall);
command_no_args!(diagnostics_export, Command::DiagnosticsExport);
command_no_args!(open_logs_folder, Command::OpenLogsFolder);
command_no_args!(open_screenshots_folder, Command::OpenScreenshotsFolder);
command_no_args!(open_registration_page, Command::OpenRegistrationPage);
command_no_args!(guest_window_to_front, Command::GuestWindowToFront);

#[tauri::command]
pub(crate) async fn guest_image_select(
    app: AppHandle,
    state: State<'_, ShellState>,
    id: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestImageSelect { id }).await
}
#[tauri::command]
pub(crate) async fn guest_create(
    app: AppHandle,
    state: State<'_, ShellState>,
    image_id: String,
    size_gib: u32,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestCreate { image_id, size_gib }).await
}
#[tauri::command]
pub(crate) async fn guest_select(
    app: AppHandle,
    state: State<'_, ShellState>,
    name: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestSelect { name }).await
}
#[tauri::command]
pub(crate) async fn guest_delete(
    app: AppHandle,
    state: State<'_, ShellState>,
    name: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestDelete { name }).await
}
#[tauri::command]
pub(crate) async fn guest_reinstall(
    app: AppHandle,
    state: State<'_, ShellState>,
    name: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestReinstall { name }).await
}
#[tauri::command]
pub(crate) async fn guest_root_set(
    app: AppHandle,
    state: State<'_, ShellState>,
    enabled: bool,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::GuestRootSet { enabled }).await
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
pub(crate) async fn input_profile_select(
    app: AppHandle,
    state: State<'_, ShellState>,
    id: Option<String>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::InputProfileSelect { id }).await
}
#[tauri::command]
pub(crate) async fn input_profile_delete(
    app: AppHandle,
    state: State<'_, ShellState>,
    id: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::InputProfileDelete { id }).await
}
#[tauri::command]
pub(crate) async fn input_profile_save(
    app: AppHandle,
    state: State<'_, ShellState>,
    profile: InputProfile,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::InputProfileSave { profile }).await
}
#[tauri::command]
pub(crate) async fn input_binding_upsert(
    app: AppHandle,
    state: State<'_, ShellState>,
    profile_id: String,
    binding: Binding,
) -> Result<AppSnapshot, AppIssue> {
    apply(
        app,
        &state,
        Command::InputBindingUpsert {
            profile_id,
            binding,
        },
    )
    .await
}
#[tauri::command]
pub(crate) async fn input_binding_remove(
    app: AppHandle,
    state: State<'_, ShellState>,
    profile_id: String,
    id: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::InputBindingRemove { profile_id, id }).await
}
#[tauri::command]
pub(crate) async fn input_auto_apply_set(
    app: AppHandle,
    state: State<'_, ShellState>,
    enabled: bool,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::InputAutoApplySet { enabled }).await
}
#[tauri::command]
pub(crate) async fn input_suspend_hotkey_set(
    app: AppHandle,
    state: State<'_, ShellState>,
    code: String,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::InputSuspendHotkeySet { code }).await
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
pub(crate) async fn display_custom_apply(
    app: AppHandle,
    state: State<'_, ShellState>,
    size: Size,
    density_dpi: u32,
) -> Result<AppSnapshot, AppIssue> {
    apply(
        app,
        &state,
        Command::DisplayCustomApply { size, density_dpi },
    )
    .await
}
#[tauri::command]
pub(crate) async fn display_refresh_set(
    app: AppHandle,
    state: State<'_, ShellState>,
    hz: Option<u32>,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::DisplayRefreshSet { hz }).await
}
#[tauri::command]
pub(crate) async fn display_vsync_set(
    app: AppHandle,
    state: State<'_, ShellState>,
    mode: VsyncMode,
) -> Result<AppSnapshot, AppIssue> {
    apply(app, &state, Command::DisplayVsyncSet { mode }).await
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
