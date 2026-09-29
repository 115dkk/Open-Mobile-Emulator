// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! The versioned contract between the Rust core and the webview.
//!
//! `host/ui/src/contracts.ts` mirrors every type here field for field. Field names are camelCase
//! on the wire. Rust computes recommendations, limits, capabilities, and blockers; the webview
//! only renders this projection and sends typed intent commands.

use serde::{Deserialize, Serialize};

pub use ome_input::{
    Anchor, Binding, BindingAction, InputProfile, LogicalPoint, MouseButton, Trigger,
    WheelDirection,
};

/// Current Rust-to-webview contract version.
pub const CONTRACT_VERSION: u32 = 7;

/// The one read-only projection the webview renders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    /// Contract schema version.
    pub contract_version: u32,
    /// Product version.
    pub product_version: String,
    /// Top-level application phase.
    pub phase: AppPhase,
    /// Blocking host condition, independent of wizard progress.
    pub blocker: Option<Blocker>,
    /// Latest host readiness report.
    pub host: HostReport,
    /// First-run wizard projection.
    pub wizard: WizardView,
    /// Available image profiles and installed virtual machines.
    pub images: ImagesView,
    /// Active virtual-machine projection.
    pub guest: GuestView,
    /// Installed application projection.
    pub apps: AppsView,
    /// Input profiles and input-pipeline state.
    pub input: InputView,
    /// Display settings and capabilities.
    pub display: DisplayView,
    /// Product settings and host-derived limits.
    pub settings: SettingsView,
    /// Product update state.
    pub update: UpdateView,
    /// Recent events, newest first, at most five.
    pub notices: Vec<Notice>,
    /// Last failed command issue until the next successful command.
    pub issue: Option<AppIssue>,
}

/// Which body the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppPhase {
    /// First-run wizard.
    Wizard,
    /// Main product shell.
    Main,
}

/// A host condition that replaces the normal body with a blocking screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blocker {
    /// Blocking condition category.
    pub kind: BlockerKind,
}

/// Stable categories for blocking host conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockerKind {
    /// Firmware or CPU virtualization is disabled.
    VirtualizationOff,
    /// The virtual-machine executable or firmware is missing.
    QemuMissing,
    /// Windows Hypervisor Platform is disabled.
    HypervisorPlatformOff,
}

/// Complete host readiness projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostReport {
    /// Readiness rows in stable order.
    pub rows: Vec<HostRow>,
    /// True when no row is blocked.
    pub ready: bool,
    /// Inspection time in RFC 3339, or `None` before inspection.
    pub inspected_at: Option<String>,
}

/// One host readiness row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRow {
    /// Stable check identifier.
    pub id: HostCheckId,
    /// Readiness state.
    pub status: HostStatus,
    /// User-facing Korean sentence.
    pub detail: String,
}

/// Stable host readiness checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostCheckId {
    /// CPU virtualization support.
    CpuVirtualization,
    /// Windows Hypervisor Platform feature.
    HypervisorPlatform,
    /// Pending restart state.
    RebootPending,
    /// WHPX execution interface.
    WhpxAvailable,
    /// Virtual-machine executable.
    QemuPresent,
    /// Virtual-machine firmware.
    FirmwarePresent,
    /// Android application management tool.
    AdbPresent,
    /// Free disk space.
    DiskSpace,
}

/// Outcome of one host readiness check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostStatus {
    /// Ready for use.
    Ready,
    /// Needs attention but does not block progress.
    Attention,
    /// Prevents setup or execution.
    Blocked,
}

/// First-run wizard projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardView {
    /// Current wizard step.
    pub step: WizardStep,
    /// Whether the primary action may run.
    pub can_continue: bool,
    /// Whether the current step may be skipped.
    pub can_skip: bool,
    /// Current image-transfer progress.
    pub download: Option<TransferProgress>,
    /// Image selected in the download step.
    pub image_id: Option<String>,
    /// Installer guidance from the selected profile.
    pub install_guide: Vec<String>,
    /// Selected virtual disk size.
    pub disk_size_gib: u32,
    /// Free bytes available to the installer.
    pub disk_free_bytes: Option<u64>,
}

/// Ordered first-run wizard steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WizardStep {
    /// Read-only host inspection.
    HostCheck,
    /// Explicit WHPX consent.
    WhpxConsent,
    /// Windows restart guidance.
    RebootPending,
    /// Verified image download.
    ArtifactDownload,
    /// Interactive operating-system installation.
    GuestInstall,
    /// First boot and capability probe.
    FirstBoot,
    /// Optional initial application installation.
    AppInstall,
    /// Wizard completion.
    Done,
}

/// Progress of an image download or application installation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    /// Transfer stage.
    pub stage: TransferStage,
    /// Completed byte count.
    pub done_bytes: u64,
    /// Total byte count when known.
    pub total_bytes: Option<u64>,
    /// Current byte rate when known.
    pub bytes_per_second: Option<u64>,
    /// User-visible item label.
    pub label: String,
}

/// Stable transfer stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferStage {
    /// Waiting to begin.
    Waiting,
    /// Bytes are moving.
    Transferring,
    /// Integrity is being checked.
    Verifying,
    /// Integrity check passed.
    Verified,
    /// Transfer failed.
    Failed,
    /// User cancelled the transfer.
    Cancelled,
}

/// Progress while one or more application packages are installed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    /// Current source filename or package identifier.
    pub label: String,
    /// Stable operation stage.
    pub stage: TransferStage,
    /// Completed package count.
    pub done_items: u64,
    /// Total package count.
    pub total_items: u64,
    /// Overall ratio from zero through one when known.
    pub ratio: Option<f64>,
    /// Compatibility byte-shaped count used by existing progress presentation.
    pub done_bytes: u64,
    /// Compatibility byte-shaped total used by existing progress presentation.
    pub total_bytes: Option<u64>,
}

/// Available operating-system profiles and installed virtual machines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagesView {
    /// Installable profiles, newest Android version first.
    pub profiles: Vec<GuestImageSummary>,
    /// Installed virtual machines.
    pub guests: Vec<GuestSummary>,
    /// Name of the selected virtual machine.
    pub active_guest: Option<String>,
}

/// Webview-facing summary of one image profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestImageSummary {
    /// Stable profile identifier.
    pub id: String,
    /// User-visible image name.
    pub display_name: String,
    /// Android version text.
    pub android_version: String,
    /// Android API level.
    pub api_level: u32,
    /// Distribution family.
    pub distribution: ImageDistribution,
    /// Native bridge.
    pub translator: ImageTranslator,
    /// Artifact byte size when the manifest provides it.
    pub size_bytes: Option<u64>,
    /// Product support state.
    pub status: ImageStatus,
    /// Publisher release date.
    pub released_at: Option<String>,
    /// Number of verified games.
    pub verified_games: u32,
    /// Rust-computed default-selection recommendation.
    pub recommended: bool,
}

/// Image distribution families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageDistribution {
    /// Bliss OS.
    Bliss,
    /// Android-x86.
    AndroidX86,
    /// Project-built image.
    SelfBuilt,
}

/// Native bridges carried by image profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageTranslator {
    /// Intel Houdini.
    Houdini,
    /// Google libndk_translation.
    NdkTranslation,
    /// Digitalis.
    Digitalis,
    /// No native bridge.
    None,
}

/// Image-profile support state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageStatus {
    /// Product verification completed.
    Verified,
    /// Verification has not completed.
    Candidate,
    /// Support has ended.
    Deprecated,
}

/// Summary of one installed virtual machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestSummary {
    /// Stable virtual-machine name.
    pub name: String,
    /// Source image profile ID.
    pub image_id: String,
    /// Android version text.
    pub android_version: String,
    /// Virtual disk size in GiB.
    pub disk_size_gib: u32,
    /// Last start time in RFC 3339 when known.
    pub last_started_at: Option<String>,
    /// Stored capability probe report.
    pub capabilities: CapabilityReport,
}

/// Result of the first-boot capability probe.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReport {
    /// Probe time in RFC 3339 when a probe has run.
    pub probed_at: Option<String>,
    /// Capability results in stable order.
    pub items: Vec<CapabilityItem>,
}

/// One capability result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityItem {
    /// Capability identifier.
    pub id: CapabilityId,
    /// Probe result.
    pub state: Capability,
}

/// Capabilities the product may probe inside the operating system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityId {
    /// Boot-completion marker.
    BootMarker,
    /// Installed application list.
    AppList,
    /// Display size and density control.
    DisplaySize,
    /// Media volume control.
    MediaVolume,
    /// Device identifier lookup.
    DeviceId,
    /// Screenshot capture.
    Screenshot,
    /// Foreground application lookup.
    ForegroundApp,
    /// Multitouch input device.
    Multitouch,
    /// Native bridge presence.
    NativeBridge,
    /// Root permission control.
    Root,
}

/// Availability of an optional operating-system capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    /// Capability probe succeeded.
    Available,
    /// Capability probe established absence.
    Unavailable,
    /// Capability has not been established.
    Unknown,
}

/// Active virtual-machine projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestView {
    /// Supervisor state.
    pub state: GuestState,
    /// Whether `sys.boot_completed` was observed.
    pub boot_completed: bool,
    /// Whether application management is connected.
    pub adb_connected: bool,
    /// How the operating-system window is shown.
    pub hosting: HostingMode,
    /// Operating-system resolution when known.
    pub resolution: Option<Size>,
    /// Last process exit.
    pub last_exit: Option<LastExit>,
    /// Frames per second when enabled and available.
    pub fps: Option<u32>,
    /// Current process start time.
    pub started_at: Option<String>,
    /// Active image profile ID.
    pub image_id: Option<String>,
    /// Active Android version.
    pub android_version: Option<String>,
    /// Active Android API level.
    pub api_level: Option<u32>,
    /// Active operating-system capability report.
    pub capabilities: CapabilityReport,
    /// GSF Android ID in lowercase hexadecimal when available.
    pub device_id: Option<String>,
    /// GSF Android ID in decimal when available.
    pub device_id_decimal: Option<String>,
    /// Number of signed-in Google accounts, or unknown before probing.
    pub google_accounts: Option<u32>,
    /// Local RFC 3339 time when registration was last opened for this operating system.
    pub registration_opened_at: Option<String>,
    /// Whether this operating-system generation can open Google account setup.
    pub add_account_supported: bool,
    /// Active virtual-machine process identifier.
    pub pid: Option<u32>,
    /// Address offered to adb clients.
    pub adb_address: Option<String>,
    /// Whether root requests are enabled, or unknown before probing.
    pub root_enabled: Option<bool>,
    /// Current media volume index from 0 through 15, or unknown before probing.
    pub media_volume: Option<u32>,
}

/// Virtual-machine supervisor states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuestState {
    /// Process is absent.
    Stopped,
    /// Process is starting.
    Starting,
    /// QMP is responding.
    Running,
    /// Stop was requested.
    Stopping,
    /// A guest reset is being converted to process restart.
    Restarting,
    /// Start or execution failed.
    Failed,
}

/// How the operating-system window is presented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostingMode {
    /// No window is active.
    None,
    /// Native window is shown as an owned popup over the stage.
    Embedded,
    /// Native window remains separate.
    SeparateWindow,
}

/// Last virtual-machine process exit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastExit {
    /// Exit category.
    pub kind: ExitKind,
    /// Exit time.
    pub at: String,
    /// Retained stderr log path.
    pub log_path: Option<String>,
}

/// Stable process-exit categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExitKind {
    /// User requested shutdown.
    UserStop,
    /// Operating system requested reset.
    GuestReset,
    /// Operating system did not complete boot in time.
    BootTimeout,
    /// Process crashed after starting.
    Crash,
    /// Process could not start.
    StartFailed,
}

/// Integer width and height pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// Rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    /// Left coordinate.
    pub x: i32,
    /// Top coordinate.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// Stage rectangle in CSS pixels plus device scale factor.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageRect {
    /// Left CSS coordinate.
    pub x: f64,
    /// Top CSS coordinate.
    pub y: f64,
    /// CSS width.
    pub width: f64,
    /// CSS height.
    pub height: f64,
    /// Device scale factor.
    pub scale_factor: f64,
}

/// Installed application projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsView {
    /// Whether application actions are currently available.
    pub available: bool,
    /// Last known installed applications.
    pub items: Vec<AppItem>,
    /// Current application installation progress.
    pub install: Option<InstallProgress>,
}

/// One installed application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppItem {
    /// Android package name.
    pub package: String,
    /// User-visible label.
    pub label: String,
    /// Version name when known.
    pub version_name: Option<String>,
    /// Package-manager version code when known.
    pub version_code: Option<u64>,
    /// Installation time when known.
    pub installed_at: Option<String>,
}

/// Input profile and pipeline projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputView {
    /// Bundled and user-created profiles.
    pub profiles: Vec<InputProfile>,
    /// Selected profile ID.
    pub active_id: Option<String>,
    /// Whether mapping is temporarily suspended.
    pub suspended: bool,
    /// Whether overlay editing is active.
    pub editing: bool,
    /// Whether foreground-application auto-apply is enabled.
    pub auto_apply: bool,
    /// Current foreground package when known.
    pub foreground_package: Option<String>,
    /// Multitouch capability.
    pub multitouch: Capability,
    /// W3C keyboard code used to suspend mapping.
    pub suspend_hotkey: String,
    /// Whether input bindings are visible over the operating-system window.
    pub overlay_visible: bool,
}

/// Display projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayView {
    /// Fixed display presets.
    pub presets: Vec<DisplayPreset>,
    /// Active fixed preset ID.
    pub active_id: Option<String>,
    /// Active custom display when one is selected.
    pub custom: Option<CustomDisplay>,
    /// Stage sizing policy.
    pub fit: StageFit,
    /// Requested refresh rate, or operating-system default.
    pub refresh_rate_hz: Option<u32>,
    /// Standard and current custom refresh choices.
    pub refresh_rates: Vec<u32>,
    /// Whether the QEMU build reports the patched refresh-rate option.
    pub refresh_supported: bool,
    /// Requested swap interval policy.
    pub vsync: VsyncMode,
    /// Whether the QEMU build reports the patched swap-interval option.
    pub vsync_supported: bool,
}

/// Fixed display preset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayPreset {
    /// Stable preset ID.
    pub id: String,
    /// Pixel size.
    pub size: Size,
    /// Android density.
    pub density_dpi: u32,
    /// Orientation.
    pub orientation: Orientation,
    /// Whether applying the preset requires a restart.
    pub needs_reboot: bool,
}

/// Custom display size and density.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomDisplay {
    /// Pixel size.
    pub size: Size,
    /// Android density.
    pub density_dpi: u32,
}

/// Display orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    /// Width is at least height.
    Landscape,
    /// Height exceeds width.
    Portrait,
}

/// Stage sizing policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageFit {
    /// Scale to the available stage.
    FitWindow,
    /// Show one guest pixel per host pixel.
    OneToOne,
}

/// Requested vertical synchronization mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VsyncMode {
    /// Disable synchronization.
    Off,
    /// Synchronize every frame.
    On,
    /// Use adaptive synchronization.
    Adaptive,
}

/// Persisted settings plus host-derived read-only limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Configured memory in MiB.
    pub memory_mib: u32,
    /// Host-derived minimum memory in MiB.
    pub memory_mib_min: u32,
    /// Host-derived maximum memory in MiB.
    pub memory_mib_max: u32,
    /// Configured virtual processor count.
    pub vcpus: u32,
    /// Host-derived maximum virtual processor count.
    pub vcpus_max: u32,
    /// GPU mode.
    pub gpu_mode: GpuMode,
    /// Window close behavior.
    pub close_action: CloseAction,
    /// Whether FPS is shown.
    pub show_fps: bool,
    /// Whether startup checks for updates.
    pub auto_update_check: bool,
    /// OME home directory.
    pub home_dir: String,
    /// Disk usage when known.
    pub disk_usage_bytes: Option<u64>,
    /// adb network exposure policy.
    pub adb_access: AdbAccess,
    /// Whether binding markers are shown by default.
    pub binding_overlay_default: bool,
}

/// Settings values the webview may change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    /// Requested memory in MiB.
    pub memory_mib: u32,
    /// Requested virtual processor count.
    pub vcpus: u32,
    /// Requested GPU mode.
    pub gpu_mode: GpuMode,
    /// Requested window close behavior.
    pub close_action: CloseAction,
    /// Requested FPS visibility.
    pub show_fps: bool,
    /// Requested automatic update check.
    pub auto_update_check: bool,
    /// Requested adb exposure policy.
    pub adb_access: AdbAccess,
    /// Requested default binding-marker visibility.
    pub binding_overlay_default: bool,
}

/// Product GPU mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GpuMode {
    /// Host OpenGL through virglrenderer.
    Virgl,
    /// Software rendering.
    Software,
}

/// Window-close behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseAction {
    /// Stop the virtual machine and close the window.
    StopGuest,
    /// Hide the window while the virtual machine runs.
    MinimizeToTray,
}

/// adb network exposure policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AdbAccess {
    /// Listen only on loopback.
    Localhost,
    /// Listen on all host network interfaces.
    Network,
}

/// Product update projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    /// Current product version.
    pub current_version: String,
    /// Update operation state.
    pub state: UpdateState,
}

/// Trusted installer asset selected from one GitHub release.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAsset {
    /// Publisher-provided leaf filename.
    pub name: String,
    /// Declared asset byte length.
    pub size_bytes: u64,
}

/// Product update operation state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateState {
    /// No update operation has run.
    Idle,
    /// Update metadata is being checked.
    Checking,
    /// Current version is newest.
    UpToDate {
        /// Check time.
        checked_at: String,
    },
    /// A newer version is available.
    Available {
        /// Available version.
        version: String,
        /// Release notes URL when supplied by trusted update metadata.
        notes_url: Option<String>,
        /// Installer asset chosen by native code.
        asset: UpdateAsset,
    },
    /// Update package is downloading.
    Downloading {
        /// Download progress.
        progress: TransferProgress,
    },
    /// Verified update is ready.
    ReadyToInstall {
        /// Ready version.
        version: String,
    },
    /// Update operation failed.
    Failed {
        /// User-facing failure.
        issue: AppIssue,
    },
}

/// One recent product event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    /// Event time.
    pub at: String,
    /// Severity.
    pub level: NoticeLevel,
    /// User-facing Korean sentence.
    pub message: String,
}

/// Notice severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoticeLevel {
    /// Informational event.
    Info,
    /// Event needing attention.
    Warning,
    /// Failed operation.
    Error,
}

/// Typed user-facing command failure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppIssue {
    /// Stable snake_case machine code.
    pub code: String,
    /// Korean 합니다체 outcome sentence.
    pub message: String,
    /// Korean 합니다체 next action when one exists.
    pub next_action: Option<String>,
}

/// Help destinations selected by trusted native code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HelpTopic {
    /// Firmware virtualization setup help.
    VirtualizationBios,
    /// Windows Hypervisor Platform setup help.
    HypervisorPlatform,
    /// Google account registration help.
    GoogleAccount,
    /// Adb access security help.
    AdbSecurity,
    /// Matching QEMU source-offer download.
    QemuSource,
    /// Third-party software notices.
    ThirdPartyNotices,
    /// Release notes for the available update.
    ReleaseNotes,
}

/// Snapshot-owned values that native code may copy to the clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardItem {
    /// GSF Android ID from the active operating system.
    DeviceId,
    /// Adb address from the active virtual machine.
    AdbAddress,
}

/// Every command the webview may send.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// Re-run host inspection.
    HostCheckRefresh,
    /// Advance the first-run wizard.
    WizardContinue,
    /// Skip an optional wizard step.
    WizardSkip,
    /// Close the wizard while retaining its current progress.
    WizardDefer,
    /// Open a trusted help destination.
    OpenHelp {
        /// Help destination chosen by the webview.
        topic: HelpTopic,
    },
    /// Close the window and end the application.
    AppQuit,
    /// Enable WHPX after explicit consent.
    WhpxEnable,
    /// Start the selected image download.
    ArtifactDownloadStart,
    /// Cancel the selected image download.
    ArtifactDownloadCancel,
    /// Select an installable image profile.
    GuestImageSelect {
        /// Profile ID.
        id: String,
    },
    /// Create a virtual machine from an image profile.
    GuestCreate {
        /// Profile ID.
        image_id: String,
        /// Virtual disk size in GiB.
        size_gib: u32,
    },
    /// Select an installed virtual machine.
    GuestSelect {
        /// Virtual-machine identifier.
        id: String,
    },
    /// Delete an installed virtual machine and disk.
    GuestDelete {
        /// Virtual-machine identifier.
        id: String,
    },
    /// Delete and reinstall one virtual machine from the same profile.
    GuestReinstall {
        /// Virtual-machine name.
        name: String,
    },
    /// Start the selected virtual machine.
    GuestStart,
    /// Stop the selected virtual machine.
    GuestStop,
    /// Restart the selected virtual machine.
    GuestRestart,
    /// Enable or disable operating-system root requests.
    GuestRootSet {
        /// Requested root state.
        enabled: bool,
    },
    /// Set the operating-system media volume.
    GuestVolumeSet {
        /// Media volume index from 0 through 15.
        index: u32,
    },
    /// Update the webview stage rectangle and make the hosted child visible.
    StageRectChanged {
        /// Current stage geometry.
        rect: StageRect,
    },
    /// Hide the hosted child because the stage left the webview.
    StageHidden,
    /// Save an operating-system screenshot.
    ScreenshotSave,
    /// Open a native package picker and install selected packages.
    AppInstallPick,
    /// Cancel the active package installation.
    AppInstallCancel,
    /// Uninstall one package.
    AppUninstall {
        /// Android package name.
        package: String,
    },
    /// Launch one package.
    AppLaunch {
        /// Android package name.
        package: String,
    },
    /// Select an input profile or no profile.
    InputProfileSelect {
        /// Profile ID.
        id: Option<String>,
    },
    /// Toggle mapping suspension.
    InputSuspendToggle,
    /// Toggle the input-binding overlay.
    InputOverlayToggle,
    /// Delete one user profile.
    InputProfileDelete {
        /// Profile ID.
        id: String,
    },
    /// Create or replace one user profile.
    InputProfileSave {
        /// Complete validated profile.
        profile: InputProfile,
    },
    /// Insert or replace one binding.
    InputBindingUpsert {
        /// Owning profile ID.
        profile_id: String,
        /// Complete binding.
        binding: Binding,
    },
    /// Remove one binding.
    InputBindingRemove {
        /// Owning profile ID.
        profile_id: String,
        /// Binding ID.
        id: String,
    },
    /// Toggle overlay editing.
    InputEditorToggle,
    /// Enable or disable foreground-application auto-apply.
    InputAutoApplySet {
        /// Requested auto-apply state.
        enabled: bool,
    },
    /// Change the mapping suspend hotkey.
    InputSuspendHotkeySet {
        /// W3C `KeyboardEvent.code` value.
        code: String,
    },
    /// Apply one fixed display preset.
    DisplayPresetApply {
        /// Preset ID.
        id: String,
    },
    /// Apply a custom size and density.
    DisplayCustomApply {
        /// Pixel size.
        size: Size,
        /// Android density.
        density_dpi: u32,
    },
    /// Change the requested refresh rate.
    DisplayRefreshSet {
        /// Hertz, or `None` for the operating-system default.
        hz: Option<u32>,
    },
    /// Change the requested vertical synchronization mode.
    DisplayVsyncSet {
        /// Requested synchronization mode.
        mode: VsyncMode,
    },
    /// Change stage sizing policy.
    StageFitSet {
        /// Requested stage fit.
        fit: StageFit,
    },
    /// Validate and persist product settings.
    SettingsSave {
        /// Complete editable settings.
        settings: SettingsInput,
    },
    /// Check for product updates.
    UpdateCheck,
    /// Install a verified update.
    UpdateInstall,
    /// Export a diagnostic bundle through a native picker.
    DiagnosticsExport,
    /// Open the fixed logs folder.
    OpenLogsFolder,
    /// Open the fixed screenshots folder.
    OpenScreenshotsFolder,
    /// Open the fixed product home folder.
    OpenHomeFolder,
    /// Copy a snapshot-owned value to the clipboard.
    CopyToClipboard {
        /// Value selected by the webview.
        item: ClipboardItem,
    },
    /// Copy the device ID and open Google's registration page.
    OpenRegistrationPage,
    /// Open Google account setup inside the running operating system.
    GoogleAccountAddOpen,
    /// Bring the separate operating-system window forward.
    GuestWindowToFront,
}

impl Command {
    /// Returns the matching Tauri command name.
    pub const fn tauri_name(&self) -> &'static str {
        match self {
            Self::HostCheckRefresh => "host_check_refresh",
            Self::WizardContinue => "wizard_continue",
            Self::WizardSkip => "wizard_skip",
            Self::WizardDefer => "wizard_defer",
            Self::OpenHelp { .. } => "open_help",
            Self::AppQuit => "app_quit",
            Self::WhpxEnable => "whpx_enable",
            Self::ArtifactDownloadStart => "artifact_download_start",
            Self::ArtifactDownloadCancel => "artifact_download_cancel",
            Self::GuestImageSelect { .. } => "guest_image_select",
            Self::GuestCreate { .. } => "guest_create",
            Self::GuestSelect { .. } => "guest_select",
            Self::GuestDelete { .. } => "guest_delete",
            Self::GuestReinstall { .. } => "guest_reinstall",
            Self::GuestStart => "guest_start",
            Self::GuestStop => "guest_stop",
            Self::GuestRestart => "guest_restart",
            Self::GuestRootSet { .. } => "guest_root_set",
            Self::GuestVolumeSet { .. } => "guest_volume_set",
            Self::StageRectChanged { .. } => "stage_rect_changed",
            Self::StageHidden => "stage_hidden",
            Self::ScreenshotSave => "screenshot_save",
            Self::AppInstallPick => "app_install_pick",
            Self::AppInstallCancel => "app_install_cancel",
            Self::AppUninstall { .. } => "app_uninstall",
            Self::AppLaunch { .. } => "app_launch",
            Self::InputProfileSelect { .. } => "input_profile_select",
            Self::InputSuspendToggle => "input_suspend_toggle",
            Self::InputOverlayToggle => "input_overlay_toggle",
            Self::InputProfileDelete { .. } => "input_profile_delete",
            Self::InputProfileSave { .. } => "input_profile_save",
            Self::InputBindingUpsert { .. } => "input_binding_upsert",
            Self::InputBindingRemove { .. } => "input_binding_remove",
            Self::InputEditorToggle => "input_editor_toggle",
            Self::InputAutoApplySet { .. } => "input_auto_apply_set",
            Self::InputSuspendHotkeySet { .. } => "input_suspend_hotkey_set",
            Self::DisplayPresetApply { .. } => "display_preset_apply",
            Self::DisplayCustomApply { .. } => "display_custom_apply",
            Self::DisplayRefreshSet { .. } => "display_refresh_set",
            Self::DisplayVsyncSet { .. } => "display_vsync_set",
            Self::StageFitSet { .. } => "stage_fit_set",
            Self::SettingsSave { .. } => "settings_save",
            Self::UpdateCheck => "update_check",
            Self::UpdateInstall => "update_install",
            Self::DiagnosticsExport => "diagnostics_export",
            Self::OpenLogsFolder => "open_logs_folder",
            Self::OpenScreenshotsFolder => "open_screenshots_folder",
            Self::OpenHomeFolder => "open_home_folder",
            Self::CopyToClipboard { .. } => "copy_to_clipboard",
            Self::OpenRegistrationPage => "open_registration_page",
            Self::GoogleAccountAddOpen => "google_account_add_open",
            Self::GuestWindowToFront => "guest_window_to_front",
        }
    }
}

/// Every Tauri command name, including the read-only snapshot command.
pub const TAURI_COMMANDS: &[&str] = &[
    "app_snapshot",
    "host_check_refresh",
    "wizard_continue",
    "wizard_skip",
    "wizard_defer",
    "open_help",
    "app_quit",
    "whpx_enable",
    "artifact_download_start",
    "artifact_download_cancel",
    "guest_image_select",
    "guest_create",
    "guest_select",
    "guest_delete",
    "guest_reinstall",
    "guest_start",
    "guest_stop",
    "guest_restart",
    "guest_root_set",
    "guest_volume_set",
    "stage_rect_changed",
    "stage_hidden",
    "screenshot_save",
    "app_install_pick",
    "app_install_cancel",
    "app_uninstall",
    "app_launch",
    "input_host_key",
    "input_profile_select",
    "input_suspend_toggle",
    "input_overlay_toggle",
    "input_profile_delete",
    "input_profile_save",
    "input_binding_upsert",
    "input_binding_remove",
    "input_editor_toggle",
    "input_auto_apply_set",
    "input_suspend_hotkey_set",
    "display_preset_apply",
    "display_custom_apply",
    "display_refresh_set",
    "display_vsync_set",
    "stage_fit_set",
    "settings_save",
    "update_check",
    "update_install",
    "diagnostics_export",
    "open_logs_folder",
    "open_screenshots_folder",
    "open_home_folder",
    "copy_to_clipboard",
    "open_registration_page",
    "google_account_add_open",
    "guest_window_to_front",
];

/// Full-snapshot event name.
pub const EVENT_SNAPSHOT: &str = "snapshot";
/// Frequent transfer-progress event name.
pub const EVENT_PROGRESS: &str = "progress";
