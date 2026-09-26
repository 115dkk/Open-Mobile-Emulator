// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! The contract between the Rust core and the webview: the snapshot the webview draws, the
//! commands it may send, and the issue it receives when a command fails.
//!
//! `host/ui/src/contracts.ts` mirrors every type here field for field. The two are kept equal
//! by the JSON fixtures under `tests/fixtures/contract/`, which the Rust tests write and the
//! TypeScript tests parse. Change both sides in one commit. Field names are camelCase on the
//! wire. Nothing in this file decides anything: it is data. Authority stays with the modules
//! that produce the snapshot (`docs/ARCHITECTURE.md` section 3).

use serde::{Deserialize, Serialize};

/// Bumped when a field changes meaning or a variant is removed. Adding an optional field or a
/// new variant does not bump it.
pub const CONTRACT_VERSION: u32 = 1;

/// The one read-only projection the webview renders. Rust builds it; the webview never derives
/// permission or judgement from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub contract_version: u32,
    pub product_version: String,
    pub phase: AppPhase,
    pub host: HostReport,
    pub wizard: WizardView,
    pub guest: GuestView,
    pub apps: AppsView,
    pub keymap: KeymapView,
    pub display: DisplayView,
    pub settings: SettingsView,
    pub update: UpdateView,
    /// Recent events for the diagnostics section, newest first, at most five.
    pub notices: Vec<Notice>,
    /// The last failed command's issue, until the next successful command clears it.
    pub issue: Option<AppIssue>,
}

/// Which body the window shows. The rail is hidden while the wizard runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppPhase {
    Wizard,
    Main,
}

// ---- host readiness --------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostReport {
    pub rows: Vec<HostRow>,
    /// True when no row is `Blocked`. `Attention` rows do not block.
    pub ready: bool,
    /// When the host was last inspected, RFC 3339. None before the first inspection.
    pub inspected_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRow {
    pub id: HostCheckId,
    pub status: HostStatus,
    /// One user-facing sentence (Korean, 합니다체), never an internal name.
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostCheckId {
    CpuVirtualization,
    HypervisorPlatform,
    RebootPending,
    WhpxAvailable,
    QemuPresent,
    FirmwarePresent,
    AdbPresent,
    DiskSpace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostStatus {
    Ready,
    Attention,
    Blocked,
}

// ---- first-run wizard -------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardView {
    pub step: WizardStep,
    /// The primary button is enabled.
    pub can_continue: bool,
    /// The step may be skipped (R8 registration yes, R9 consent no).
    pub can_skip: bool,
    pub download: Option<TransferProgress>,
    /// GSF Android ID for the registration step, when read from the guest.
    pub gsf_id: Option<String>,
    pub disk_size_gib: u32,
    pub disk_free_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WizardStep {
    HostCheck,
    WhpxConsent,
    RebootPending,
    ArtifactDownload,
    GuestInstall,
    FirstBoot,
    GoogleRegistration,
    AppInstall,
    Done,
}

/// Progress of a long transfer: the ISO download or an app install.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub stage: TransferStage,
    pub done_bytes: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: Option<u64>,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferStage {
    Waiting,
    Transferring,
    Verifying,
    Verified,
    Failed,
    Cancelled,
}

// ---- guest -------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestView {
    pub state: GuestState,
    /// `sys.boot_completed` observed through adb. Distinct from `Running`.
    pub boot_completed: bool,
    pub adb_connected: bool,
    pub hosting: HostingMode,
    /// Guest resolution as reported by the guest, when known.
    pub resolution: Option<Size>,
    pub last_exit: Option<LastExit>,
    /// Frames per second, only when `settings.show_fps` is on and the guest reports it.
    pub fps: Option<u32>,
    pub started_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuestState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Restarting,
    Failed,
}

/// How the guest window is shown. `Embedded` is plan 1 (reparented into the stage),
/// `SeparateWindow` is plan 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostingMode {
    None,
    Embedded,
    SeparateWindow,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastExit {
    pub kind: ExitKind,
    pub at: String,
    /// Path of the stderr log kept for this exit, shown in mono.
    pub log_path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExitKind {
    UserStop,
    GuestReset,
    Unexpected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// A rectangle in physical pixels relative to the window's client area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The stage rectangle as the webview lays it out: CSS pixels plus the device scale factor.
/// Rust converts it to physical pixels; the webview never does.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale_factor: f64,
}

// ---- apps --------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsView {
    /// False while the guest is not booted; the list then holds the last known items.
    pub available: bool,
    pub items: Vec<AppItem>,
    pub install: Option<TransferProgress>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppItem {
    pub package: String,
    /// Label as the guest reports it. User data, shown as is.
    pub label: String,
    pub version_name: Option<String>,
    pub installed_at: Option<String>,
}

// ---- key mapping and display ----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeymapView {
    pub profiles: Vec<KeymapProfileSummary>,
    pub active_id: Option<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeymapProfileSummary {
    pub id: String,
    pub name: String,
    pub bundled: bool,
    pub binding_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayView {
    pub presets: Vec<DisplayPreset>,
    pub active_id: Option<String>,
    pub fit: StageFit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayPreset {
    pub id: String,
    pub size: Size,
    pub density_dpi: u32,
    pub orientation: Orientation,
    /// Applying this preset needs a guest reboot (the `video=` boot argument changes).
    pub needs_reboot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Landscape,
    Portrait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageFit {
    FitWindow,
    OneToOne,
}

// ---- settings and update --------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub memory_mib: u32,
    pub vcpus: u32,
    pub gpu_mode: GpuMode,
    pub close_action: CloseAction,
    pub show_fps: bool,
    pub auto_update_check: bool,
    pub home_dir: String,
    pub disk_usage_bytes: Option<u64>,
}

/// The subset of settings the webview may change. Validated by `ome-guest-config` on receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub memory_mib: u32,
    pub vcpus: u32,
    pub gpu_mode: GpuMode,
    pub close_action: CloseAction,
    pub show_fps: bool,
    pub auto_update_check: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GpuMode {
    Virgl,
    Software,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseAction {
    MinimizeToTray,
    StopGuest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub current_version: String,
    pub state: UpdateState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate {
        checked_at: String,
    },
    Available {
        version: String,
        notes_url: Option<String>,
    },
    Downloading {
        progress: TransferProgress,
    },
    ReadyToInstall {
        version: String,
    },
    Failed {
        issue: AppIssue,
    },
}

// ---- notices and issues ----------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub at: String,
    pub level: NoticeLevel,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
}

/// What the webview receives when a command fails. `message` and `next_action` are the words
/// the screen shows; the raw error text goes to the log, never here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppIssue {
    /// Stable machine code, snake_case, for tests and logs.
    pub code: String,
    pub message: String,
    pub next_action: Option<String>,
}

// ---- commands ----------------------------------------------------------------------------------

/// Every command the webview may send. Each variant is one Tauri command in `host/app`
/// with the same name in snake_case; the shell does nothing but forward it to
/// `AppRuntime::apply`. No variant carries a path, URL or command line from the webview.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// Re-run the host inspection.
    HostCheckRefresh,
    /// Advance the wizard from its current step after the step's work succeeded.
    WizardContinue,
    /// Skip the current step when `WizardView::can_skip` is true.
    WizardSkip,
    /// Reset the wizard to its first step (settings: 처음부터 다시 설정).
    WizardRestart,
    /// The user pressed 켜기 on the consent screen. Launches the fixed-verb setup helper (R9).
    WhpxEnable,
    ArtifactDownloadStart,
    ArtifactDownloadCancel,
    GuestDiskCreate {
        size_gib: u32,
    },
    GuestStart,
    GuestStop,
    GuestRestart,
    /// The stage rectangle changed in the webview layout.
    StageRectChanged {
        rect: StageRect,
    },
    /// Save a guest screenshot into the screenshots folder.
    ScreenshotSave,
    /// Open a file dialog on the Rust side and install the chosen package(s).
    AppInstallPick,
    AppUninstall {
        package: String,
    },
    AppLaunch {
        package: String,
    },
    KeymapSetActive {
        id: Option<String>,
    },
    KeymapSetEnabled {
        enabled: bool,
    },
    KeymapDelete {
        id: String,
    },
    DisplayPresetApply {
        id: String,
    },
    StageFitSet {
        fit: StageFit,
    },
    SettingsSave {
        settings: SettingsInput,
    },
    UpdateCheck,
    UpdateInstall,
    DiagnosticsExport,
    OpenLogsFolder,
    OpenScreenshotsFolder,
    /// Open the Google uncertified-device registration page in the default browser (R8).
    OpenRegistrationPage,
    /// Bring the separate guest window to the front (hosting plan 2).
    GuestWindowToFront,
}

impl Command {
    /// The Tauri command name for this variant, used by the capability test and the shell.
    pub const fn tauri_name(&self) -> &'static str {
        match self {
            Command::HostCheckRefresh => "host_check_refresh",
            Command::WizardContinue => "wizard_continue",
            Command::WizardSkip => "wizard_skip",
            Command::WizardRestart => "wizard_restart",
            Command::WhpxEnable => "whpx_enable",
            Command::ArtifactDownloadStart => "artifact_download_start",
            Command::ArtifactDownloadCancel => "artifact_download_cancel",
            Command::GuestDiskCreate { .. } => "guest_disk_create",
            Command::GuestStart => "guest_start",
            Command::GuestStop => "guest_stop",
            Command::GuestRestart => "guest_restart",
            Command::StageRectChanged { .. } => "stage_rect_changed",
            Command::ScreenshotSave => "screenshot_save",
            Command::AppInstallPick => "app_install_pick",
            Command::AppUninstall { .. } => "app_uninstall",
            Command::AppLaunch { .. } => "app_launch",
            Command::KeymapSetActive { .. } => "keymap_set_active",
            Command::KeymapSetEnabled { .. } => "keymap_set_enabled",
            Command::KeymapDelete { .. } => "keymap_delete",
            Command::DisplayPresetApply { .. } => "display_preset_apply",
            Command::StageFitSet { .. } => "stage_fit_set",
            Command::SettingsSave { .. } => "settings_save",
            Command::UpdateCheck => "update_check",
            Command::UpdateInstall => "update_install",
            Command::DiagnosticsExport => "diagnostics_export",
            Command::OpenLogsFolder => "open_logs_folder",
            Command::OpenScreenshotsFolder => "open_screenshots_folder",
            Command::OpenRegistrationPage => "open_registration_page",
            Command::GuestWindowToFront => "guest_window_to_front",
        }
    }
}

/// Every Tauri command name, in one place, so the shell's `generate_handler!` list and the
/// capability file can be checked against it. `app_snapshot` is the read command and is not a
/// `Command` variant.
pub const TAURI_COMMANDS: &[&str] = &[
    "app_snapshot",
    "host_check_refresh",
    "wizard_continue",
    "wizard_skip",
    "wizard_restart",
    "whpx_enable",
    "artifact_download_start",
    "artifact_download_cancel",
    "guest_disk_create",
    "guest_start",
    "guest_stop",
    "guest_restart",
    "stage_rect_changed",
    "screenshot_save",
    "app_install_pick",
    "app_uninstall",
    "app_launch",
    "keymap_set_active",
    "keymap_set_enabled",
    "keymap_delete",
    "display_preset_apply",
    "stage_fit_set",
    "settings_save",
    "update_check",
    "update_install",
    "diagnostics_export",
    "open_logs_folder",
    "open_screenshots_folder",
    "open_registration_page",
    "guest_window_to_front",
];

/// Event names the shell emits to the webview. `snapshot` carries a full `AppSnapshot`;
/// `progress` carries a `TransferProgress` for frequent download updates.
pub const EVENT_SNAPSHOT: &str = "snapshot";
pub const EVENT_PROGRESS: &str = "progress";
