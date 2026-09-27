// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use jiff::{Unit, Zoned};
use ome_adb::{AdbSession, AppPackage};
use ome_artifacts::{ArtifactStore, Manifest, StoreError, StoreProgress};
use ome_guest_config::{GuestConfig, GuestPaths, QemuInstall, RawGuestConfig};
use ome_guest_image::{
    CapabilityProbe, Distribution, FamilyAdapter, GuestImageProfile, ImageStatus as ProfileStatus,
    ProbeItem, ProbeOutcome, ProbeState, ShellRunner, Translator, adapter_for, recommended_index,
    sort_newest_first,
};
use ome_host_check::{
    FeatureState, HardwareLimits, HostProbe, HostReadiness, Verdict, hardware_limits,
};
use ome_input::{InputProfile, is_keyboard_code, load_profiles};
use ome_supervisor::{GuestEvent, ProcessAdapter, QmpFactory, Supervisor};
use ome_window_host::{
    GuestWindowHost, HostingIssue, HostingTarget, StageGeometry, StageRect as NativeStageRect,
};
use ome_wizard::{Facts, Outcome, Step, WizardState, advance, can_continue, can_skip};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::adapters::AdbShellRunner;
use crate::desktop::Desktop;
use crate::guest_store::{
    GuestRecord, GuestStore, GuestStoreError, StoredProbeItem, StoredProbeState,
};
use crate::home::OmeHome;
use crate::issues;
use crate::operations::{ElevationError, WorkerDeps};
use crate::settings::{Settings, SettingsError, SettingsStore};
use crate::{
    AppIssue, AppItem, AppPhase, AppSnapshot, AppsView, Blocker, BlockerKind, CONTRACT_VERSION,
    Capability, CapabilityId, CapabilityReport, ClipboardItem, Command, CustomDisplay,
    DisplayPreset, DisplayView, ExitKind, GuestImageSummary, GuestState, GuestSummary, GuestView,
    HelpTopic, HostCheckId, HostReport, HostRow, HostStatus, HostingMode, ImageDistribution,
    ImageStatus, ImageTranslator, ImagesView, InputView, InstallProgress, LastExit, Notice,
    NoticeLevel, Orientation, Rect, SettingsView, Size, StageFit, StageRect, TransferProgress,
    TransferStage, UpdateAsset, UpdateState, UpdateView, VsyncMode, WizardStep, WizardView,
};

/// Process lifecycle seam owned by the runtime.
pub trait GuestProcess: Send {
    /// Starts one validated virtual-machine process.
    fn start(
        &mut self,
        config: GuestConfig,
        paths: GuestPaths,
        install: QemuInstall,
    ) -> Result<(), String>;
    /// Starts one installer-mode virtual-machine process.
    fn start_install(
        &mut self,
        config: GuestConfig,
        paths: GuestPaths,
        install: QemuInstall,
    ) -> Result<(), String>;
    /// Requests graceful shutdown.
    fn request_stop(&self);
    /// Returns the current supervisor state.
    fn state(&self) -> ome_supervisor::GuestState;
    /// Subscribes to lifecycle events.
    fn subscribe(&self) -> mpsc::Receiver<GuestEvent>;
}

impl<A, Q> GuestProcess for Supervisor<A, Q>
where
    A: ProcessAdapter,
    Q: QmpFactory,
{
    fn start(
        &mut self,
        config: GuestConfig,
        paths: GuestPaths,
        install: QemuInstall,
    ) -> Result<(), String> {
        Supervisor::start(self, config, paths, install).map_err(|error| error.to_string())
    }

    fn start_install(
        &mut self,
        config: GuestConfig,
        paths: GuestPaths,
        install: QemuInstall,
    ) -> Result<(), String> {
        Supervisor::start(self, config, paths, install).map_err(|error| error.to_string())
    }

    fn request_stop(&self) {
        let _ = Supervisor::request_stop(self);
    }

    fn state(&self) -> ome_supervisor::GuestState {
        Supervisor::state(self)
    }

    fn subscribe(&self) -> mpsc::Receiver<GuestEvent> {
        Supervisor::subscribe(self)
    }
}

/// Guest-window seam used by deterministic runtime tests.
pub trait WindowPlacement: Send {
    /// Attaches one process window to its native parent.
    fn attach(&mut self, target: HostingTarget) -> Result<(), HostingIssue>;
    /// Places the attached window.
    fn place(&mut self, rect: ome_window_host::Rect) -> Result<(), HostingIssue>;
    /// Hides the attached child window.
    fn hide(&mut self) -> Result<(), HostingIssue>;
    /// Shows the attached child window without activation.
    fn show(&mut self) -> Result<(), HostingIssue>;
    /// Restores top-level window state.
    fn detach(&mut self) -> Result<(), HostingIssue>;
    /// Activates the hosted or separate window.
    fn to_front(&mut self) -> Result<(), HostingIssue>;
    /// Reports live embedded state.
    fn is_attached(&self) -> bool;
    /// Returns the hosted guest client rectangle in physical screen pixels when known.
    fn client_screen_rect(&self) -> Option<Rect> {
        None
    }
}

impl WindowPlacement for GuestWindowHost {
    fn attach(&mut self, target: HostingTarget) -> Result<(), HostingIssue> {
        GuestWindowHost::attach(self, target)
    }
    fn place(&mut self, rect: ome_window_host::Rect) -> Result<(), HostingIssue> {
        GuestWindowHost::place(self, rect)
    }
    fn hide(&mut self) -> Result<(), HostingIssue> {
        GuestWindowHost::hide(self)
    }
    fn show(&mut self) -> Result<(), HostingIssue> {
        GuestWindowHost::show(self)
    }
    fn detach(&mut self) -> Result<(), HostingIssue> {
        GuestWindowHost::detach(self)
    }
    fn to_front(&mut self) -> Result<(), HostingIssue> {
        GuestWindowHost::to_front(self)
    }
    fn is_attached(&self) -> bool {
        GuestWindowHost::is_attached(self)
    }
    fn client_screen_rect(&self) -> Option<Rect> {
        let native = GuestWindowHost::guest_window(self)?
            .client_screen_rect()
            .ok()?;
        Some(Rect {
            x: native.x,
            y: native.y,
            width: native.width,
            height: native.height,
        })
    }
}

/// Runtime dependencies supplied by the native shell.
pub struct RuntimeDeps {
    /// Read-only host observation adapter.
    pub probe: Box<dyn HostProbe>,
    /// Verified artifact store when manifest loading succeeded.
    pub artifacts: Option<ArtifactStore>,
    /// Operating-system adb session when the shell discovered the executable.
    pub adb: Option<AdbSession>,
    /// QEMU lifecycle supervisor.
    pub supervisor: Option<Box<dyn GuestProcess>>,
    /// Trusted native desktop operations.
    pub desktop: Box<dyn Desktop>,
    /// Native window host.
    pub window_host: Box<dyn WindowPlacement>,
    /// Optional generation adapter override used by deterministic tests.
    pub family_adapter: Option<Box<dyn FamilyAdapter>>,
    /// Trusted directory containing image profiles.
    pub images_dir: Option<PathBuf>,
    /// Trusted external-artifact manifest used for profile sizes.
    pub artifacts_manifest: Option<PathBuf>,
    /// Product version shown in snapshots.
    pub product_version: String,
}

impl std::fmt::Debug for RuntimeDeps {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeDeps")
            .field("artifacts", &self.artifacts.is_some())
            .field("adb", &self.adb.is_some())
            .field("supervisor", &self.supervisor.is_some())
            .field("images_dir", &self.images_dir)
            .field("artifacts_manifest", &self.artifacts_manifest)
            .field("product_version", &self.product_version)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
enum WorkerEvent {
    ArtifactProgress(TransferProgress),
    ArtifactFinished(Result<(), AppIssue>),
    InstallProgress(InstallProgress),
    InstallFinished(Result<Vec<AppItem>, AppIssue>),
    UpdateChecked(Result<UpdateRelease, AppIssue>),
    UpdateProgress(TransferProgress),
    UpdateDownloaded(Result<DownloadedUpdate, AppIssue>),
}

#[derive(Clone, Debug)]
struct UpdateRelease {
    version: String,
    notes_url: Option<String>,
    installer: ReleaseAsset,
    checksum: Option<ReleaseAsset>,
}

#[derive(Clone, Debug)]
struct DownloadedUpdate {
    version: String,
    path: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
struct ReleaseDocument {
    tag_name: String,
    html_url: String,
    body: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Native product runtime and owner of snapshot projection and command decisions.
pub struct AppRuntime {
    home: OmeHome,
    deps: RuntimeDeps,
    workers: WorkerDeps,
    settings_store: SettingsStore,
    guest_store: GuestStore,
    settings: Settings,
    guests: Vec<GuestRecord>,
    active_guest: Option<String>,
    limits: HardwareLimits,
    host: HostReport,
    blocker: Option<Blocker>,
    feature_state: FeatureState,
    wizard: WizardState,
    phase: AppPhase,
    image_profiles: Vec<GuestImageProfile>,
    artifact_manifest: Option<Manifest>,
    selected_image: Option<String>,
    input_profiles: Vec<InputProfile>,
    active_input: Option<String>,
    input_suspended: bool,
    input_editing: bool,
    input_auto_apply: bool,
    suspend_hotkey: String,
    input_overlay_visible: bool,
    display_fit: StageFit,
    active_display: Option<String>,
    custom_display: Option<CustomDisplay>,
    refresh_rate_hz: Option<u32>,
    vsync: VsyncMode,
    guest_state: GuestState,
    pid: Option<u32>,
    boot_completed: bool,
    adb_connected: bool,
    adb_connect_attempted: bool,
    boot_started: Option<Instant>,
    last_account_poll: Option<Instant>,
    apps: Vec<AppItem>,
    install_progress: Option<InstallProgress>,
    install_cancel: Option<Arc<AtomicBool>>,
    artifact_progress: Option<TransferProgress>,
    artifact_cancel: Option<Arc<AtomicBool>>,
    worker_tx: mpsc::Sender<WorkerEvent>,
    worker_rx: mpsc::Receiver<WorkerEvent>,
    update_release: Option<UpdateRelease>,
    update_download: Option<DownloadedUpdate>,
    update_exit_requested: bool,
    installing_guest: bool,
    hosting: HostingMode,
    hosted_pid: Option<u32>,
    host_window: Option<u64>,
    last_stage_rect: Option<StageRect>,
    stage_visible: bool,
    resolution: Option<Size>,
    last_exit: Option<LastExit>,
    started_at: Option<String>,
    capabilities: CapabilityReport,
    device_id: Option<String>,
    device_id_decimal: Option<String>,
    google_accounts: Option<u32>,
    registration_opened_at: Option<String>,
    add_account_supported: bool,
    root_enabled: Option<bool>,
    media_volume: Option<u32>,
    restart_pending: bool,
    boot_timeout_pending: bool,
    #[cfg(test)]
    boot_timeout_override: Option<Duration>,
    notices: Vec<Notice>,
    update_state: UpdateState,
    issue: Option<AppIssue>,
}

impl std::fmt::Debug for AppRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AppRuntime")
            .field("home", &self.home)
            .field("phase", &self.phase)
            .field("wizard", &self.wizard)
            .finish_non_exhaustive()
    }
}

impl AppRuntime {
    /// Opens runtime storage and loads trusted settings, profiles, guests, and host-derived limits.
    pub fn open(home: OmeHome, deps: RuntimeDeps) -> Result<Self, AppIssue> {
        home.ensure().map_err(|_| issues::home_unavailable())?;
        let limits = hardware_limits(
            deps.probe.total_memory_bytes().ok(),
            deps.probe.logical_processors().ok(),
        );
        let settings_store = SettingsStore::new(home.as_path());
        let settings = settings_store.load(&limits).map_err(settings_issue)?;
        let guest_store =
            GuestStore::new(home.subdir("vm").map_err(|_| issues::home_unavailable())?);
        let guests = guest_store.load_all().map_err(guest_store_issue)?;
        let saved_active = guest_store.load_active().map_err(guest_store_issue)?;
        let active_guest = saved_active
            .filter(|id| guests.iter().any(|guest| guest.id == *id))
            .or_else(|| guests.first().map(|guest| guest.id.clone()));
        let mut image_profiles = match deps.images_dir.as_deref() {
            Some(directory) => GuestImageProfile::load_all(directory)
                .map_err(|_| issues::image_profiles_invalid())?,
            None => Vec::new(),
        };
        sort_newest_first(&mut image_profiles);
        let artifact_manifest = deps
            .artifacts_manifest
            .as_deref()
            .map(Manifest::load)
            .transpose()
            .map_err(|_| issues::image_profiles_invalid())?;
        if let Some(manifest) = artifact_manifest.as_ref()
            && image_profiles.iter().any(|profile| {
                !manifest
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.name == profile.artifact)
            })
        {
            return Err(issues::image_profiles_invalid());
        }
        let today = today_utc();
        let selected_image = active_guest
            .as_deref()
            .and_then(|id| guests.iter().find(|guest| guest.id == id))
            .map(|guest| guest.image_id.clone())
            .or_else(|| {
                recommended_index(&image_profiles, &today)
                    .map(|index| image_profiles[index].id.clone())
            })
            .or_else(|| image_profiles.first().map(|profile| profile.id.clone()));
        let active_record = active_guest
            .as_deref()
            .and_then(|id| guests.iter().find(|guest| guest.id == id));
        let capabilities = active_record
            .map(|guest| capability_report(&guest.capabilities))
            .unwrap_or_default();
        let device_id = active_record
            .and_then(|guest| guest.device_id.as_ref())
            .map(|id| id.hex.clone());
        let device_id_decimal = active_record
            .and_then(|guest| guest.device_id.as_ref())
            .map(|id| id.decimal.clone());
        let registration_opened_at =
            active_record.and_then(|guest| guest.registration_opened_at.clone());
        let root_enabled = active_record.and_then(|guest| guest.root_enabled);
        let google_accounts = active_record.and_then(|guest| guest.capabilities.google_accounts);
        let media_volume = active_record.and_then(|guest| guest.capabilities.media_volume);
        let resolution = active_record.and_then(|guest| {
            guest.capabilities.display.map(|display| Size {
                width: display.width,
                height: display.height,
            })
        });
        let add_account_supported = active_record.is_some_and(|guest| {
            adapter_for(guest.api_level)
                .add_google_account_command()
                .is_some()
        });
        let mut input_profiles = bundled_profiles();
        let directory =
            home_profiles_directory(home.as_path()).map_err(|_| issues::home_unavailable())?;
        if directory.is_dir() {
            let user_profiles =
                load_profiles(&directory).map_err(|_| issues::invalid_input_profile())?;
            input_profiles.extend(user_profiles);
        }
        let active_input = input_profiles.first().map(|profile| profile.id.clone());
        let input_overlay_visible = settings.binding_overlay_default;
        let feature_state = deps
            .probe
            .hypervisor_platform()
            .unwrap_or(FeatureState::Unknown);
        let update_state = UpdateState::Idle;
        let apps = active_record.map_or_else(Vec::new, stored_apps);
        let (worker_tx, worker_rx) = mpsc::channel();
        let mut runtime = Self {
            home,
            deps,
            workers: WorkerDeps::default(),
            settings_store,
            guest_store,
            settings,
            guests,
            active_guest,
            limits,
            host: empty_host_report(),
            blocker: None,
            feature_state,
            wizard: WizardState::default(),
            phase: AppPhase::Wizard,
            image_profiles,
            artifact_manifest,
            selected_image,
            input_profiles,
            active_input,
            input_suspended: false,
            input_editing: false,
            input_auto_apply: true,
            suspend_hotkey: "F12".to_owned(),
            input_overlay_visible,
            display_fit: StageFit::FitWindow,
            active_display: Some("hd-720".to_owned()),
            custom_display: None,
            refresh_rate_hz: None,
            vsync: VsyncMode::Off,
            guest_state: GuestState::Stopped,
            pid: None,
            boot_completed: false,
            adb_connected: false,
            adb_connect_attempted: false,
            boot_started: None,
            last_account_poll: None,
            apps,
            install_progress: None,
            install_cancel: None,
            artifact_progress: None,
            artifact_cancel: None,
            worker_tx,
            worker_rx,
            update_release: None,
            update_download: None,
            update_exit_requested: false,
            installing_guest: false,
            hosting: HostingMode::None,
            hosted_pid: None,
            host_window: None,
            last_stage_rect: None,
            stage_visible: true,
            resolution,
            last_exit: None,
            started_at: None,
            capabilities,
            device_id,
            device_id_decimal,
            google_accounts,
            registration_opened_at,
            add_account_supported,
            root_enabled,
            media_volume,
            restart_pending: false,
            boot_timeout_pending: false,
            #[cfg(test)]
            boot_timeout_override: None,
            notices: Vec::new(),
            update_state,
            issue: None,
        };
        runtime.refresh_host();
        Ok(runtime)
    }

    /// Returns the complete read-only projection without performing I/O.
    pub fn snapshot(&self) -> AppSnapshot {
        let selected_guest = self.selected_guest();
        let selected_profile = selected_guest
            .and_then(|guest| self.profile_by_id(&guest.image_id))
            .or_else(|| self.selected_profile());
        AppSnapshot {
            contract_version: CONTRACT_VERSION,
            product_version: self.deps.product_version.clone(),
            phase: self.phase,
            blocker: self.current_blocker(),
            host: self.host.clone(),
            wizard: self.wizard_view(),
            images: self.images_view(),
            guest: GuestView {
                state: self.guest_state,
                boot_completed: self.boot_completed,
                adb_connected: self.adb_connected,
                hosting: self.hosting,
                resolution: self.resolution,
                last_exit: self.last_exit.clone(),
                fps: None,
                started_at: self.started_at.clone(),
                image_id: selected_guest
                    .map(|guest| guest.image_id.clone())
                    .or_else(|| self.selected_image.clone()),
                android_version: selected_guest
                    .map(|guest| guest.android_version.clone())
                    .or_else(|| selected_profile.map(|profile| profile.android_version.clone())),
                api_level: selected_guest
                    .map(|guest| guest.api_level)
                    .or_else(|| selected_profile.map(|profile| profile.api_level)),
                capabilities: self.capabilities.clone(),
                device_id: self.device_id.clone(),
                device_id_decimal: self.device_id_decimal.clone(),
                google_accounts: self.google_accounts,
                registration_opened_at: self.registration_opened_at.clone(),
                add_account_supported: self.add_account_supported,
                pid: self.pid,
                adb_address: Some(self.adb_address()),
                root_enabled: self.root_enabled,
                media_volume: self.media_volume,
            },
            apps: AppsView {
                available: self.guest_state == GuestState::Running && self.boot_completed,
                items: self.apps.clone(),
                install: self.install_progress.clone(),
            },
            input: InputView {
                profiles: self.input_profiles.clone(),
                active_id: self.active_input.clone(),
                suspended: self.input_suspended,
                editing: self.input_editing,
                auto_apply: self.input_auto_apply,
                foreground_package: None,
                multitouch: capability_value(&self.capabilities, CapabilityId::Multitouch),
                suspend_hotkey: self.suspend_hotkey.clone(),
                overlay_visible: self.input_overlay_visible,
            },
            display: DisplayView {
                presets: self.display_presets(),
                active_id: self.active_display.clone(),
                custom: self.custom_display,
                fit: self.display_fit,
                refresh_rate_hz: self.refresh_rate_hz,
                refresh_rates: refresh_rates(self.refresh_rate_hz),
                refresh_supported: false,
                vsync: self.vsync,
                vsync_supported: false,
            },
            settings: SettingsView {
                memory_mib: self.settings.memory_mib,
                memory_mib_min: self.limits.memory_mib_min,
                memory_mib_max: self.limits.memory_mib_max,
                vcpus: self.settings.vcpus,
                vcpus_max: self.limits.vcpus_max,
                gpu_mode: self.settings.gpu_mode,
                close_action: self.settings.close_action,
                show_fps: self.settings.show_fps,
                auto_update_check: self.settings.auto_update_check,
                home_dir: self.home.as_path().to_string_lossy().into_owned(),
                disk_usage_bytes: self
                    .guests
                    .iter()
                    .map(|guest| guest.disk_bytes)
                    .sum::<u64>()
                    .into(),
                adb_access: self.settings.adb_access,
                binding_overlay_default: self.settings.binding_overlay_default,
            },
            update: UpdateView {
                current_version: self.deps.product_version.clone(),
                state: self.update_state.clone(),
            },
            notices: self.notices.clone(),
            issue: self.issue.clone(),
        }
    }

    /// Applies one typed command and returns the resulting complete snapshot.
    pub fn apply(&mut self, command: Command) -> Result<AppSnapshot, AppIssue> {
        match self.apply_inner(command) {
            Ok(()) => {
                self.issue = None;
                Ok(self.snapshot())
            }
            Err(issue) => {
                self.issue = Some(issue.clone());
                Err(issue)
            }
        }
    }

    fn apply_inner(&mut self, command: Command) -> Result<(), AppIssue> {
        match command {
            Command::HostCheckRefresh => {
                self.refresh_host();
                Ok(())
            }
            Command::WizardContinue => self.wizard_continue(),
            Command::WizardSkip => self.wizard_skip(),
            Command::WizardDefer => {
                self.phase = AppPhase::Main;
                Ok(())
            }
            Command::GuestImageSelect { id } => self.select_image(id),
            Command::WhpxEnable => self.enable_whpx(),
            Command::ArtifactDownloadStart => self.start_artifact_download(),
            Command::ArtifactDownloadCancel => {
                self.cancel_artifact_download();
                Ok(())
            }
            Command::GuestCreate { image_id, size_gib } => self.create_guest(image_id, size_gib),
            Command::GuestReinstall { name } => self.reinstall_guest(name),
            Command::GuestSelect { id } => self.select_guest(id),
            Command::GuestDelete { id } => self.delete_guest(id),
            Command::GuestStart => self.start_guest(),
            Command::GuestStop => self.stop_guest(),
            Command::GuestRestart => self.restart_guest(),
            Command::GuestVolumeSet { index } => self.set_guest_volume(index),
            Command::StageRectChanged { rect } => self.place_guest_window(rect),
            Command::StageHidden => self.hide_guest_window(),
            Command::ScreenshotSave => self.save_screenshot(),
            Command::AppInstallCancel => {
                self.cancel_app_install();
                Ok(())
            }
            Command::AppUninstall { package } => self.uninstall_app(&package),
            Command::AppLaunch { package } => self.launch_app(&package),
            Command::UpdateCheck => self.start_update_check(),
            Command::UpdateInstall => self.start_update_install(),
            Command::DiagnosticsExport => self.export_diagnostics(),
            Command::OpenHelp { topic } => self.open_help(topic),
            Command::OpenHomeFolder => self.open_fixed_directory(None),
            Command::OpenLogsFolder => self.open_fixed_directory(Some("logs")),
            Command::OpenScreenshotsFolder => self.open_fixed_directory(Some("screenshots")),
            Command::CopyToClipboard { item } => self.copy_to_clipboard(item),
            Command::OpenRegistrationPage => self.open_registration_page(),
            Command::GoogleAccountAddOpen => self.open_google_account_add(),
            Command::GuestWindowToFront => self
                .deps
                .window_host
                .to_front()
                .map_err(|_| issues::window_unavailable()),
            Command::InputProfileSelect { id } => self.select_input(id),
            Command::InputSuspendToggle => {
                self.input_suspended = !self.input_suspended;
                Ok(())
            }
            Command::InputOverlayToggle => {
                self.input_overlay_visible = !self.input_overlay_visible;
                Ok(())
            }
            Command::InputProfileDelete { id } => self.delete_input(id),
            Command::InputProfileSave { profile } => self.save_input(profile),
            Command::InputBindingUpsert {
                profile_id,
                binding,
            } => self.upsert_binding(profile_id, binding),
            Command::InputBindingRemove { profile_id, id } => self.remove_binding(profile_id, id),
            Command::InputEditorToggle => {
                self.input_editing = !self.input_editing;
                Ok(())
            }
            Command::InputAutoApplySet { enabled } => {
                self.input_auto_apply = enabled;
                Ok(())
            }
            Command::InputSuspendHotkeySet { code } => {
                if !is_keyboard_code(&code) {
                    return Err(issues::invalid_suspend_hotkey());
                }
                self.suspend_hotkey = code;
                Ok(())
            }
            Command::DisplayPresetApply { id } => self.apply_display_preset(id),
            Command::DisplayCustomApply { size, density_dpi } => {
                self.apply_custom_display(size, density_dpi)
            }
            Command::DisplayRefreshSet { hz } => {
                if hz.is_some_and(|value| !(30..=240).contains(&value)) {
                    return Err(issues::invalid_display());
                }
                self.refresh_rate_hz = hz;
                Ok(())
            }
            Command::DisplayVsyncSet { mode } => {
                self.vsync = mode;
                Ok(())
            }
            Command::StageFitSet { fit } => {
                self.display_fit = fit;
                Ok(())
            }
            Command::SettingsSave { settings } => {
                let validated =
                    Settings::validate(settings, &self.limits).map_err(settings_issue)?;
                self.settings_store
                    .save(&validated)
                    .map_err(settings_issue)?;
                self.settings = validated;
                Ok(())
            }
            Command::AppQuit | Command::GuestRootSet { .. } | Command::AppInstallPick => {
                Err(issues::not_wired())
            }
        }
    }

    /// Replaces process, elevation, and HTTP adapters before native commands run.
    pub fn set_worker_deps(&mut self, workers: WorkerDeps) {
        self.workers = workers;
    }

    /// Starts the configured one-time startup update check.
    pub fn start_auto_update_check(&mut self) -> Result<AppSnapshot, AppIssue> {
        if self.settings.auto_update_check && matches!(self.update_state, UpdateState::Idle) {
            self.start_update_check()?;
        }
        Ok(self.snapshot())
    }

    /// Starts installation for validated native paths supplied only by the shell.
    pub fn install_apps(&mut self, paths: Vec<PathBuf>) -> Result<AppSnapshot, AppIssue> {
        self.require_booted()?;
        let paths = validate_app_paths(paths)?;
        if paths.is_empty() {
            return Ok(self.snapshot());
        }
        if self.install_cancel.is_some() {
            return Err(issues::operation_in_progress());
        }
        let adb = self
            .deps
            .adb
            .as_ref()
            .cloned()
            .ok_or_else(issues::operating_system_connection_unavailable)?;
        let total = u64::try_from(paths.len()).unwrap_or(u64::MAX);
        let cancel = Arc::new(AtomicBool::new(false));
        self.install_cancel = Some(Arc::clone(&cancel));
        self.install_progress = Some(install_progress(
            paths[0]
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("앱 파일"),
            TransferStage::Waiting,
            0,
            total,
        ));
        let sender = self.worker_tx.clone();
        thread::Builder::new()
            .name("ome-app-install".to_owned())
            .spawn(move || {
                let mut done = 0_u64;
                let mut failure = None;
                for path in paths {
                    if done > 0 && cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let label = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("앱 파일")
                        .to_owned();
                    let _ = sender.send(WorkerEvent::InstallProgress(install_progress(
                        &label,
                        TransferStage::Transferring,
                        done,
                        total,
                    )));
                    let package = match AppPackage::open(&path) {
                        Ok(package) => package,
                        Err(_) => {
                            failure = Some(issues::app_package_invalid());
                            break;
                        }
                    };
                    if adb.install(&package).is_err() {
                        failure = Some(issues::app_install_failed());
                        break;
                    }
                    done = done.saturating_add(1);
                }
                let result = if let Some(issue) = failure {
                    Err(issue)
                } else if cancel.load(Ordering::Acquire) {
                    Ok(Vec::new())
                } else {
                    adb.packages()
                        .map(package_items)
                        .map_err(|_| issues::app_list_unavailable())
                };
                let _ = sender.send(WorkerEvent::InstallFinished(result));
            })
            .map_err(|_| issues::worker_unavailable())?;
        Ok(self.snapshot())
    }

    fn require_booted(&self) -> Result<(), AppIssue> {
        if self.guest_state == GuestState::Running && self.boot_completed {
            Ok(())
        } else {
            Err(issues::operating_system_not_running())
        }
    }

    fn cancel_app_install(&self) {
        if let Some(cancel) = &self.install_cancel {
            cancel.store(true, Ordering::Release);
        }
    }

    fn uninstall_app(&mut self, package: &str) -> Result<(), AppIssue> {
        self.require_booted()?;
        let adb = self
            .deps
            .adb
            .as_ref()
            .ok_or_else(issues::operating_system_connection_unavailable)?;
        adb.uninstall(package)
            .map_err(|_| issues::app_uninstall_failed())?;
        self.refresh_apps()
    }

    fn launch_app(&self, package: &str) -> Result<(), AppIssue> {
        self.require_booted()?;
        self.deps
            .adb
            .as_ref()
            .ok_or_else(issues::operating_system_connection_unavailable)?
            .launch(package)
            .map_err(|_| issues::app_launch_failed())
    }

    fn refresh_apps(&mut self) -> Result<(), AppIssue> {
        let packages = self
            .deps
            .adb
            .as_ref()
            .ok_or_else(issues::operating_system_connection_unavailable)?
            .packages()
            .map_err(|_| issues::app_list_unavailable())?;
        self.apps = package_items(packages);
        Ok(())
    }

    fn start_artifact_download(&mut self) -> Result<(), AppIssue> {
        if self.artifact_cancel.is_some() {
            return Err(issues::operation_in_progress());
        }
        let profile = self
            .selected_profile()
            .ok_or_else(issues::image_not_found)?;
        let artifact_name = profile.artifact.clone();
        let store = self
            .deps
            .artifacts
            .as_ref()
            .cloned()
            .ok_or_else(issues::artifact_store_unavailable)?;
        if let Some(verified) = store.verified(&artifact_name) {
            let label = verified
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("운영체제 이미지")
                .to_owned();
            self.artifact_progress = Some(transfer_progress(
                TransferStage::Verified,
                verified.size_bytes,
                Some(verified.size_bytes),
                None,
                label,
            ));
            return Ok(());
        }
        let artifact = store
            .metadata(&artifact_name)
            .ok_or_else(issues::image_not_found)?;
        let label = artifact.filename.clone();
        let total = artifact.size_bytes;
        let initial = store.partial_len(&artifact_name).min(total);
        let cancel = Arc::new(AtomicBool::new(false));
        self.artifact_cancel = Some(Arc::clone(&cancel));
        self.artifact_progress = Some(transfer_progress(
            TransferStage::Waiting,
            initial,
            Some(total),
            None,
            label.clone(),
        ));
        let sender = self.worker_tx.clone();
        thread::Builder::new()
            .name("ome-artifact-download".to_owned())
            .spawn(move || {
                let started = Instant::now();
                let mut done = initial;
                let result = store.ensure_cancellable(&artifact_name, &mut |event| match event {
                    StoreProgress::Bytes(bytes) => {
                        done = done.saturating_add(bytes).min(total);
                        let elapsed = started.elapsed().as_secs_f64();
                        let rate = (elapsed > 0.0).then(|| {
                            u64::try_from((done.saturating_sub(initial) as f64 / elapsed) as u128)
                                .unwrap_or(u64::MAX)
                        });
                        let _ = sender.send(WorkerEvent::ArtifactProgress(transfer_progress(
                            TransferStage::Transferring,
                            done,
                            Some(total),
                            rate,
                            label.clone(),
                        )));
                        !cancel.load(Ordering::Acquire)
                    }
                    StoreProgress::Verifying => {
                        let _ = sender.send(WorkerEvent::ArtifactProgress(transfer_progress(
                            TransferStage::Verifying,
                            total,
                            Some(total),
                            None,
                            label.clone(),
                        )));
                        !cancel.load(Ordering::Acquire)
                    }
                });
                let mapped = match result {
                    Ok(_) => {
                        let _ = sender.send(WorkerEvent::ArtifactProgress(transfer_progress(
                            TransferStage::Verified,
                            total,
                            Some(total),
                            None,
                            label,
                        )));
                        Ok(())
                    }
                    Err(
                        StoreError::Cancelled
                        | StoreError::Fetch(ome_artifacts::FetchError::Cancelled),
                    ) => Err(issues::operation_cancelled()),
                    Err(_) => Err(issues::artifact_download_failed()),
                };
                let _ = sender.send(WorkerEvent::ArtifactFinished(mapped));
            })
            .map_err(|_| issues::worker_unavailable())?;
        Ok(())
    }

    fn cancel_artifact_download(&self) {
        if let Some(cancel) = &self.artifact_cancel {
            cancel.store(true, Ordering::Release);
        }
    }

    fn enable_whpx(&mut self) -> Result<(), AppIssue> {
        match self.workers.elevation.enable_whpx() {
            Ok(0) => {
                self.feature_state = FeatureState::Enabled;
                self.wizard = advance(self.wizard.clone(), Outcome::WhpxEnabledNeedsReboot);
                self.wizard = advance(self.wizard.clone(), Outcome::Continue);
                Ok(())
            }
            Ok(3010) => {
                self.feature_state = FeatureState::Enabled;
                self.wizard = advance(self.wizard.clone(), Outcome::WhpxEnabledNeedsReboot);
                Ok(())
            }
            Ok(1223) => Err(issues::whpx_enable_declined()),
            Ok(code) => {
                eprintln!("ome-setup exited with code {code}");
                Err(issues::whpx_enable_failed(code))
            }
            Err(ElevationError::Declined) => Err(issues::whpx_enable_declined()),
            Err(ElevationError::Failed(_)) => Err(issues::whpx_enable_unavailable()),
        }
    }

    fn create_guest(&mut self, image_id: String, size_gib: u32) -> Result<(), AppIssue> {
        self.create_guest_internal(image_id, size_gib, None)
    }

    fn create_guest_internal(
        &mut self,
        image_id: String,
        size_gib: u32,
        forced_id: Option<String>,
    ) -> Result<(), AppIssue> {
        if !matches!(self.guest_state, GuestState::Stopped | GuestState::Failed) {
            return Err(issues::operating_system_running());
        }
        if !matches!(size_gib, 32 | 64 | 128) {
            return Err(issues::invalid_disk_size());
        }
        let profile = self
            .profile_by_id(&image_id)
            .cloned()
            .ok_or_else(issues::image_not_found)?;
        let store = self
            .deps
            .artifacts
            .as_ref()
            .ok_or_else(issues::artifact_store_unavailable)?;
        let verified = store
            .verified(&profile.artifact)
            .ok_or_else(issues::artifact_not_verified)?;
        let found = self
            .deps
            .probe
            .qemu()
            .ok()
            .flatten()
            .ok_or_else(issues::qemu_unavailable)?;
        let system_exe = PathBuf::from(&found.program);
        let qemu_img = system_exe
            .parent()
            .ok_or_else(issues::qemu_unavailable)?
            .join("qemu-img.exe");
        let firmware_code = found
            .firmware_code
            .map(PathBuf::from)
            .ok_or_else(issues::firmware_unavailable)?;
        let template = found
            .firmware_vars_template
            .map(PathBuf::from)
            .ok_or_else(issues::firmware_unavailable)?;
        let id = forced_id.unwrap_or(self.next_guest_id(&image_id)?);
        let directory = self.guest_store.guest_dir(&id).map_err(guest_store_issue)?;
        fs::create_dir(&directory).map_err(|_| issues::guest_storage_unavailable())?;
        let result = (|| {
            let disk = directory.join("disk.qcow2");
            let args = [
                OsString::from("create"),
                OsString::from("-f"),
                OsString::from("qcow2"),
                disk.as_os_str().to_owned(),
                OsString::from(format!("{size_gib}G")),
            ];
            if self
                .workers
                .process
                .run(&qemu_img, &args)
                .map_err(|_| issues::guest_create_failed())?
                != 0
                || !disk.is_file()
            {
                return Err(issues::guest_create_failed());
            }
            let firmware_vars = directory.join("efivars.fd");
            fs::copy(template, &firmware_vars).map_err(|_| issues::firmware_unavailable())?;
            let record = GuestRecord {
                id: id.clone(),
                image_id: profile.id.clone(),
                android_version: profile.android_version.clone(),
                api_level: profile.api_level,
                disk_bytes: u64::from(size_gib) << 30,
                created_at: Some(local_rfc3339()),
                last_started_at: None,
                capabilities: Default::default(),
                device_id: None,
                registration_opened_at: None,
                root_enabled: None,
            };
            self.guest_store.save(&record).map_err(guest_store_issue)?;
            let config = self.build_guest_config(&record, &profile)?;
            let paths = GuestPaths {
                disk,
                firmware_code,
                firmware_vars,
                iso: Some(verified.path),
            };
            self.deps
                .supervisor
                .as_deref_mut()
                .ok_or_else(issues::process_unavailable)?
                .start_install(config, paths, QemuInstall { system_exe })
                .map_err(|_| issues::process_start_failed())?;
            self.guests.push(record);
            self.guests.sort_by(|left, right| left.id.cmp(&right.id));
            self.active_guest = Some(id.clone());
            self.selected_image = Some(profile.id);
            self.guest_store
                .save_active(Some(&id))
                .map_err(guest_store_issue)?;
            if let Some(position) = self.selected_guest_index() {
                self.load_guest_projection(position);
            }
            self.installing_guest = true;
            self.boot_started = Some(Instant::now());
            self.boot_completed = false;
            self.adb_connected = false;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&directory);
        }
        result
    }

    fn reinstall_guest(&mut self, name: String) -> Result<(), AppIssue> {
        if !matches!(self.guest_state, GuestState::Stopped | GuestState::Failed) {
            return Err(issues::operating_system_running());
        }
        let record = self
            .guests
            .iter()
            .find(|guest| guest.id == name)
            .cloned()
            .ok_or_else(issues::guest_not_found)?;
        let size_gib = u32::try_from(record.disk_bytes >> 30)
            .unwrap_or(32)
            .clamp(32, 128);
        self.guest_store.delete(&name).map_err(guest_store_issue)?;
        self.guests.retain(|guest| guest.id != name);
        self.active_guest = None;
        self.create_guest_internal(record.image_id, size_gib, Some(name))
    }

    fn next_guest_id(&self, image_id: &str) -> Result<String, AppIssue> {
        let base = safe_guest_id(image_id).ok_or_else(issues::image_not_found)?;
        if !self.guests.iter().any(|guest| guest.id == base) {
            return Ok(base);
        }
        for suffix in 2..=9999 {
            let candidate = format!("{base}-{suffix}");
            if !self.guests.iter().any(|guest| guest.id == candidate) {
                return Ok(candidate);
            }
        }
        Err(issues::guest_create_failed())
    }

    fn export_diagnostics(&mut self) -> Result<(), AppIssue> {
        let logs = self
            .home
            .subdir("logs")
            .map_err(|_| issues::home_unavailable())?;
        let mut host_logs = Vec::new();
        let mut qemu_logs = Vec::new();
        for entry in fs::read_dir(&logs).map_err(|_| issues::diagnostics_failed())? {
            let path = entry.map_err(|_| issues::diagnostics_failed())?.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if name.contains(".stdout.") || name.contains(".stderr.") || name.contains(".cmd.") {
                qemu_logs.push(path);
            } else {
                host_logs.push(path);
            }
        }
        host_logs.sort();
        qemu_logs.sort();
        let logcat = if self.guest_state == GuestState::Running && self.boot_completed {
            self.deps
                .adb
                .as_ref()
                .and_then(|adb| adb.logcat_tail().ok())
        } else {
            None
        };
        let host_report =
            serde_json::to_string(&self.host).map_err(|_| issues::diagnostics_failed())?;
        let settings =
            serde_json::to_string(&self.settings).map_err(|_| issues::diagnostics_failed())?;
        let environment = vec![
            (
                "productVersion".to_owned(),
                self.deps.product_version.clone(),
            ),
            ("hostReportJson".to_owned(), host_report),
            ("settingsJson".to_owned(), settings),
            ("guestState".to_owned(), format!("{:?}", self.guest_state)),
            ("memoryMiB".to_owned(), self.settings.memory_mib.to_string()),
            ("vcpus".to_owned(), self.settings.vcpus.to_string()),
            (
                "gpuMode".to_owned(),
                format!("{:?}", self.settings.gpu_mode),
            ),
            (
                "closeAction".to_owned(),
                format!("{:?}", self.settings.close_action),
            ),
            (
                "adbAccess".to_owned(),
                format!("{:?}", self.settings.adb_access),
            ),
        ];
        let out_dir = self
            .home
            .subdir("diagnostics")
            .map_err(|_| issues::home_unavailable())?;
        let path = ome_diagnostics::DiagnosticBundle::collect(ome_diagnostics::BundleRequest {
            host_logs,
            qemu_logs,
            logcat,
            environment,
            out_dir: out_dir.clone(),
        })
        .map_err(|_| issues::diagnostics_failed())?;
        self.deps
            .desktop
            .open_path(&out_dir)
            .map_err(|_| issues::desktop_unavailable())?;
        self.push_notice(Notice {
            at: local_rfc3339(),
            level: NoticeLevel::Info,
            message: format!("진단 묶음을 {}에 저장했습니다.", path.display()),
        });
        Ok(())
    }

    fn start_update_check(&mut self) -> Result<(), AppIssue> {
        if matches!(
            self.update_state,
            UpdateState::Checking | UpdateState::Downloading { .. }
        ) {
            return Err(issues::operation_in_progress());
        }
        self.update_state = UpdateState::Checking;
        let version = self.deps.product_version.clone();
        let sender = self.worker_tx.clone();
        let fetch = Arc::clone(&self.workers.http);
        thread::Builder::new()
            .name("ome-update-check".to_owned())
            .spawn(move || {
                let result = fetch_release_document(fetch.as_ref())
                    .and_then(|document| release_from_document(document, &version));
                let _ = sender.send(WorkerEvent::UpdateChecked(result));
            })
            .map_err(|_| issues::worker_unavailable())?;
        Ok(())
    }

    fn start_update_install(&mut self) -> Result<(), AppIssue> {
        if matches!(
            self.update_state,
            UpdateState::Checking | UpdateState::Downloading { .. }
        ) {
            return Err(issues::operation_in_progress());
        }
        if let Some(download) = self.update_download.clone() {
            self.deps
                .desktop
                .launch_installer(&download.path)
                .map_err(|_| issues::update_launch_failed())?;
            self.update_state = UpdateState::ReadyToInstall {
                version: download.version,
            };
            self.update_exit_requested = true;
            return Ok(());
        }
        let release = self
            .update_release
            .clone()
            .ok_or_else(issues::update_not_available)?;
        let checksum = release
            .checksum
            .clone()
            .ok_or_else(issues::update_unverified)?;
        let directory = self.home.as_path().join("updates");
        fs::create_dir_all(&directory).map_err(|_| issues::update_download_failed())?;
        self.update_state = UpdateState::Downloading {
            progress: transfer_progress(
                TransferStage::Waiting,
                0,
                Some(release.installer.size),
                None,
                release.installer.name.clone(),
            ),
        };
        let sender = self.worker_tx.clone();
        let fetch = Arc::clone(&self.workers.http);
        thread::Builder::new()
            .name("ome-update-download".to_owned())
            .spawn(move || {
                let result = download_update_release(
                    &release,
                    &checksum,
                    &directory,
                    &sender,
                    fetch.as_ref(),
                );
                let _ = sender.send(WorkerEvent::UpdateDownloaded(result));
            })
            .map_err(|_| issues::worker_unavailable())?;
        Ok(())
    }

    /// Takes the one-shot request to close after launching a verified updater.
    pub fn take_update_exit_requested(&mut self) -> bool {
        std::mem::take(&mut self.update_exit_requested)
    }

    /// Applies completed background work without waiting.
    pub fn poll_workers(&mut self) {
        self.drain_worker_events();
    }

    fn drain_worker_events(&mut self) {
        while let Ok(event) = self.worker_rx.try_recv() {
            match event {
                WorkerEvent::ArtifactProgress(progress) => self.artifact_progress = Some(progress),
                WorkerEvent::ArtifactFinished(result) => {
                    self.artifact_cancel = None;
                    if let Err(issue) = result {
                        let cancelled = issue.code == "operation_cancelled";
                        self.artifact_progress =
                            self.artifact_progress.take().map(|mut progress| {
                                progress.stage = if cancelled {
                                    TransferStage::Cancelled
                                } else {
                                    TransferStage::Failed
                                };
                                progress
                            });
                        if !cancelled {
                            self.issue = Some(issue);
                        }
                    }
                }
                WorkerEvent::InstallProgress(progress) => self.install_progress = Some(progress),
                WorkerEvent::InstallFinished(result) => {
                    let cancelled = self
                        .install_cancel
                        .as_ref()
                        .is_some_and(|cancel| cancel.load(Ordering::Acquire));
                    self.install_cancel = None;
                    match result {
                        Ok(items) => {
                            if !items.is_empty() {
                                self.apps = items;
                            }
                            if let Some(progress) = &mut self.install_progress {
                                progress.stage = if cancelled {
                                    TransferStage::Cancelled
                                } else {
                                    TransferStage::Verified
                                };
                                if !cancelled {
                                    progress.done_items = progress.total_items;
                                    progress.done_bytes = progress.total_items;
                                    progress.ratio = Some(1.0);
                                }
                            }
                        }
                        Err(issue) => {
                            if let Some(progress) = &mut self.install_progress {
                                progress.stage = TransferStage::Failed;
                            }
                            self.issue = Some(issue);
                        }
                    }
                }
                WorkerEvent::UpdateChecked(result) => match result {
                    Ok(release) => {
                        let state = UpdateState::Available {
                            version: release.version.clone(),
                            notes_url: release.notes_url.clone(),
                            asset: UpdateAsset {
                                name: release.installer.name.clone(),
                                size_bytes: release.installer.size,
                            },
                        };
                        self.update_release = Some(release);
                        self.update_state = state;
                    }
                    Err(issue) if issue.code == "update_up_to_date" => {
                        self.update_release = None;
                        self.update_state = UpdateState::UpToDate {
                            checked_at: local_rfc3339(),
                        };
                    }
                    Err(issue) => self.update_state = UpdateState::Failed { issue },
                },
                WorkerEvent::UpdateProgress(progress) => {
                    self.update_state = UpdateState::Downloading { progress };
                }
                WorkerEvent::UpdateDownloaded(result) => match result {
                    Ok(download) => {
                        let version = download.version.clone();
                        let path = download.path.clone();
                        if self.deps.desktop.launch_installer(&path).is_ok() {
                            self.update_state = UpdateState::ReadyToInstall { version };
                            self.update_exit_requested = true;
                        } else {
                            self.update_download = Some(download);
                            self.update_state = UpdateState::Failed {
                                issue: issues::update_launch_failed(),
                            };
                        }
                    }
                    Err(issue) => self.update_state = UpdateState::Failed { issue },
                },
            }
        }
    }

    fn select_guest(&mut self, id: String) -> Result<(), AppIssue> {
        if !matches!(self.guest_state, GuestState::Stopped | GuestState::Failed) {
            return Err(issues::operating_system_running());
        }
        let position = self
            .guests
            .iter()
            .position(|guest| guest.id == id)
            .ok_or_else(issues::guest_not_found)?;
        self.active_guest = Some(id.clone());
        self.selected_image = Some(self.guests[position].image_id.clone());
        self.load_guest_projection(position);
        self.guest_store
            .save_active(Some(&id))
            .map_err(guest_store_issue)
    }

    fn delete_guest(&mut self, id: String) -> Result<(), AppIssue> {
        if self.active_guest.as_deref() == Some(id.as_str())
            && !matches!(self.guest_state, GuestState::Stopped | GuestState::Failed)
        {
            return Err(issues::operating_system_running());
        }
        let position = self
            .guests
            .iter()
            .position(|guest| guest.id == id)
            .ok_or_else(issues::guest_not_found)?;
        self.guest_store.delete(&id).map_err(guest_store_issue)?;
        self.guests.remove(position);
        if self.active_guest.as_deref() == Some(id.as_str()) {
            self.active_guest = self.guests.first().map(|guest| guest.id.clone());
            self.guest_store
                .save_active(self.active_guest.as_deref())
                .map_err(guest_store_issue)?;
            if let Some(position) = self.selected_guest_index() {
                self.selected_image = Some(self.guests[position].image_id.clone());
                self.load_guest_projection(position);
            } else {
                self.clear_guest_projection();
            }
        }
        Ok(())
    }

    fn start_guest(&mut self) -> Result<(), AppIssue> {
        let selected = self
            .selected_guest()
            .ok_or_else(issues::guest_not_found)?
            .clone();
        let profile = self
            .profile_by_id(&selected.image_id)
            .ok_or_else(issues::image_not_found)?
            .clone();
        let config = self.build_guest_config(&selected, &profile)?;
        let (paths, install) = self.guest_launch_paths(&selected)?;
        self.ensure_adb_session();
        let supervisor = self
            .deps
            .supervisor
            .as_deref_mut()
            .ok_or_else(issues::process_unavailable)?;
        supervisor
            .start(config, paths, install)
            .map_err(|_| issues::process_start_failed())?;
        if let Some(position) = self.selected_guest_index()
            && self.guest_store.save(&self.guests[position]).is_err()
        {
            self.push_notice(Notice {
                at: local_rfc3339(),
                level: NoticeLevel::Warning,
                message: "운영체제 정보를 저장하지 못했습니다. 저장 공간과 앱의 파일 접근 권한을 확인하십시오."
                    .to_owned(),
            });
        }
        self.boot_started = Some(Instant::now());
        self.adb_connect_attempted = false;
        self.boot_completed = false;
        self.adb_connected = false;
        self.restart_pending = false;
        Ok(())
    }

    fn stop_guest(&mut self) -> Result<(), AppIssue> {
        let supervisor = self
            .deps
            .supervisor
            .as_deref()
            .ok_or_else(issues::process_unavailable)?;
        if matches!(supervisor.state(), ome_supervisor::GuestState::Stopped) {
            return Ok(());
        }
        supervisor.request_stop();
        Ok(())
    }

    fn restart_guest(&mut self) -> Result<(), AppIssue> {
        if matches!(self.guest_state, GuestState::Stopped | GuestState::Failed) {
            return self.start_guest();
        }
        self.restart_pending = true;
        self.stop_guest()
    }

    fn build_guest_config(
        &self,
        guest: &GuestRecord,
        profile: &GuestImageProfile,
    ) -> Result<GuestConfig, AppIssue> {
        let mut extra_args = Vec::new();
        let profile_virgl = profile
            .qemu_overrides
            .iter()
            .any(|value| matches!(value, ome_guest_image::QemuOverride::VirtioVgaGl));
        let gpu = match self.settings.gpu_mode {
            crate::GpuMode::Virgl if profile_virgl => "virgl",
            crate::GpuMode::Virgl => "virtio",
            crate::GpuMode::Software => "std",
        };
        let adb_bind = match self.settings.adb_access {
            crate::AdbAccess::Localhost => "localhost",
            crate::AdbAccess::Network => "network",
        };
        // Disk boots obtain the profile's kernel arguments from the installed GRUB entry.
        // QEMU rejects `-append` unless `-kernel` performs a direct kernel boot.
        let display_size = self.custom_display.map(|display| display.size).or_else(|| {
            self.active_display.as_deref().and_then(|id| {
                display_presets_for(self.current_orientation())
                    .into_iter()
                    .find(|preset| preset.id == id)
                    .map(|preset| preset.size)
            })
        });
        if self.vsync != VsyncMode::Off {
            let interval = match self.vsync {
                VsyncMode::Off => 0,
                VsyncMode::On => 1,
                VsyncMode::Adaptive => -1,
            };
            extra_args.extend([
                "-display".to_owned(),
                format!("sdl,show-cursor=on,gl=on,swap-interval={interval}"),
            ]);
        }
        GuestConfig::validate(RawGuestConfig {
            name: Some(guest.id.clone()),
            memory_mib: Some(i64::from(self.settings.memory_mib)),
            vcpus: Some(i64::from(self.settings.vcpus)),
            gpu: Some(gpu.to_owned()),
            accel: Some("whpx".to_owned()),
            cpu_model: Some("Skylake-Client-v4".to_owned()),
            qmp_port: Some(4444),
            adb_port: Some(5555),
            adb_bind: Some(adb_bind.to_owned()),
            refresh_rate_hz: self.refresh_rate_hz.map(i64::from),
            display_size: display_size.map(|size| (i64::from(size.width), i64::from(size.height))),
            audio: Some("dsound".to_owned()),
            display: Some("sdl".to_owned()),
            extra_args: Some(extra_args),
        })
        .map_err(|_| issues::invalid_guest_configuration())
    }

    fn ensure_adb_session(&mut self) {
        if self.deps.adb.is_some() {
            return;
        }
        let Some(found) = self.deps.probe.adb().ok().flatten() else {
            return;
        };
        self.deps.adb = AdbSession::new(
            found.program,
            "127.0.0.1:5555".to_owned(),
            Box::new(ome_adb::ProcessRunner),
        )
        .ok();
    }

    fn guest_launch_paths(
        &self,
        guest: &GuestRecord,
    ) -> Result<(GuestPaths, QemuInstall), AppIssue> {
        let found = self
            .deps
            .probe
            .qemu()
            .ok()
            .flatten()
            .ok_or_else(issues::qemu_unavailable)?;
        let firmware_code = found
            .firmware_code
            .map(PathBuf::from)
            .ok_or_else(issues::firmware_unavailable)?;
        let directory = self
            .guest_store
            .guest_dir(&guest.id)
            .map_err(guest_store_issue)?;
        let firmware_vars = directory.join("efivars.fd");
        if !firmware_vars.is_file() {
            let template = found
                .firmware_vars_template
                .map(PathBuf::from)
                .ok_or_else(issues::firmware_unavailable)?;
            fs::copy(template, &firmware_vars).map_err(|_| issues::firmware_unavailable())?;
        }
        Ok((
            GuestPaths {
                disk: directory.join("disk.qcow2"),
                firmware_code,
                firmware_vars,
                iso: None,
            },
            QemuInstall {
                system_exe: PathBuf::from(found.program),
            },
        ))
    }

    fn set_guest_volume(&mut self, index: u32) -> Result<(), AppIssue> {
        if index > 15 {
            return Err(issues::invalid_media_volume());
        }
        if self.guest_state != GuestState::Running || !self.boot_completed {
            return Err(issues::operating_system_not_running());
        }
        let command = self
            .with_family_adapter(|adapter| adapter.media_volume_set_command(index))
            .ok_or_else(issues::operating_system_unsupported)?;
        self.run_shell(&command)?;
        self.media_volume = Some(index);
        Ok(())
    }

    fn save_screenshot(&mut self) -> Result<(), AppIssue> {
        if self.guest_state != GuestState::Running || !self.boot_completed {
            return Err(issues::operating_system_not_running());
        }
        let bytes = self
            .deps
            .adb
            .as_ref()
            .ok_or_else(issues::operating_system_connection_unavailable)?
            .screencap()
            .map_err(|_| issues::operating_system_connection_unavailable())?;
        let directory = self
            .home
            .subdir("screenshots")
            .map_err(|_| issues::home_unavailable())?;
        fs::create_dir_all(&directory).map_err(|_| issues::home_unavailable())?;
        let filename = format!("{}.png", local_filename_time());
        let path = directory.join(filename);
        fs::write(&path, bytes).map_err(|_| issues::home_unavailable())?;
        self.push_notice(Notice {
            at: local_rfc3339(),
            level: NoticeLevel::Info,
            message: format!("운영체제 스크린샷을 {}에 저장했습니다.", path.display()),
        });
        Ok(())
    }

    fn open_help(&self, topic: HelpTopic) -> Result<(), AppIssue> {
        const RELEASES: &str = "https://github.com/115dkk/Open-Mobile-Emulator/releases";
        let url = match topic {
            HelpTopic::VirtualizationBios => {
                "https://github.com/115dkk/Open-Mobile-Emulator/blob/main/docs/help/virtualization-bios.md"
            }
            HelpTopic::HypervisorPlatform => {
                "https://github.com/115dkk/Open-Mobile-Emulator/blob/main/docs/help/hypervisor-platform.md"
            }
            HelpTopic::GoogleAccount => {
                "https://github.com/115dkk/Open-Mobile-Emulator/blob/main/docs/help/google-account.md"
            }
            HelpTopic::AdbSecurity => {
                "https://github.com/115dkk/Open-Mobile-Emulator/blob/main/docs/help/adb-security.md"
            }
            HelpTopic::QemuSource => RELEASES,
            HelpTopic::ThirdPartyNotices => {
                "https://github.com/115dkk/Open-Mobile-Emulator/blob/main/THIRD_PARTY.md"
            }
            HelpTopic::ReleaseNotes => match &self.update_state {
                UpdateState::Available {
                    notes_url: Some(url),
                    ..
                } if trusted_release_notes_url(url) => url.as_str(),
                _ => RELEASES,
            },
        };
        self.deps
            .desktop
            .open_url(url)
            .map_err(|_| issues::desktop_unavailable())
    }

    fn open_fixed_directory(&self, kind: Option<&str>) -> Result<(), AppIssue> {
        let path = match kind {
            Some(kind) => self
                .home
                .subdir(kind)
                .map_err(|_| issues::home_unavailable())?,
            None => self.home.as_path().to_path_buf(),
        };
        fs::create_dir_all(&path).map_err(|_| issues::home_unavailable())?;
        self.deps
            .desktop
            .open_path(&path)
            .map_err(|_| issues::desktop_unavailable())
    }

    fn copy_to_clipboard(&self, item: ClipboardItem) -> Result<(), AppIssue> {
        let adb_address = self.adb_address();
        let value = match item {
            ClipboardItem::DeviceId => self
                .device_id
                .as_deref()
                .ok_or_else(issues::value_unknown)?,
            ClipboardItem::AdbAddress => adb_address.as_str(),
        };
        self.deps
            .desktop
            .copy_text(value)
            .map_err(|_| issues::clipboard_unavailable())
    }

    fn open_registration_page(&mut self) -> Result<(), AppIssue> {
        const URL: &str = "https://www.google.com/android/uncertified/";
        let device_id = self
            .device_id
            .clone()
            .ok_or_else(issues::device_id_unknown)?;
        self.deps
            .desktop
            .copy_text(&device_id)
            .map_err(|_| issues::clipboard_unavailable())?;
        self.deps
            .desktop
            .open_url(URL)
            .map_err(|_| issues::desktop_unavailable())?;
        let opened_at = local_rfc3339();
        self.registration_opened_at = Some(opened_at.clone());
        if let Some(position) = self.selected_guest_index() {
            self.guests[position].registration_opened_at = Some(opened_at);
            self.guest_store
                .save(&self.guests[position])
                .map_err(guest_store_issue)?;
        }
        Ok(())
    }

    fn open_google_account_add(&self) -> Result<(), AppIssue> {
        if self.guest_state != GuestState::Running || !self.boot_completed {
            return Err(issues::operating_system_not_running());
        }
        let command = self
            .with_family_adapter(|adapter| adapter.add_google_account_command())
            .ok_or_else(issues::operating_system_unsupported)?;
        self.run_shell(&command)
    }

    fn place_guest_window(&mut self, rect: StageRect) -> Result<(), AppIssue> {
        self.last_stage_rect = Some(rect);
        self.stage_visible = true;
        self.try_place_guest_window();
        if self.hosting == HostingMode::Embedded {
            self.deps
                .window_host
                .show()
                .map_err(|_| issues::window_unavailable())?;
        }
        Ok(())
    }

    fn hide_guest_window(&mut self) -> Result<(), AppIssue> {
        self.stage_visible = false;
        if self.hosting == HostingMode::Embedded {
            self.deps
                .window_host
                .hide()
                .map_err(|_| issues::window_unavailable())?;
        }
        Ok(())
    }

    fn try_place_guest_window(&mut self) {
        if !self.stage_visible || self.guest_state != GuestState::Running {
            return;
        }
        let Some(parent_window) = self.host_window else {
            return;
        };
        let Some(guest_process_id) = self.pid else {
            return;
        };
        let Some(rect) = self.last_stage_rect else {
            return;
        };
        if self.hosted_pid != Some(guest_process_id) {
            if self
                .deps
                .window_host
                .attach(HostingTarget {
                    parent_window,
                    guest_process_id,
                })
                .is_err()
            {
                self.hosting = HostingMode::SeparateWindow;
                self.hosted_pid = Some(guest_process_id);
                return;
            }
            self.hosted_pid = Some(guest_process_id);
        }
        let native = NativeStageRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            scale_factor: rect.scale_factor,
        };
        if self
            .deps
            .window_host
            .place(StageGeometry::physical(&native))
            .is_ok()
            && self.deps.window_host.is_attached()
        {
            self.hosting = HostingMode::Embedded;
        } else {
            self.hosting = HostingMode::SeparateWindow;
        }
    }

    /// Supplies the native parent window after the shell creates it.
    pub fn set_host_window(&mut self, parent: u64) {
        self.host_window = (parent != 0).then_some(parent);
        self.try_place_guest_window();
    }

    /// Reports whether a stage currently exists in the webview.
    pub fn stage_visible(&self) -> bool {
        self.stage_visible
    }

    /// Returns the attached guest client rectangle in physical screen pixels when known.
    pub fn guest_client_screen_rect(&self) -> Option<Rect> {
        (self.stage_visible && self.hosting == HostingMode::Embedded)
            .then(|| self.deps.window_host.client_screen_rect())
            .flatten()
    }

    /// Subscribes to lifecycle events without exposing the concrete supervisor type.
    pub fn subscribe_guest_events(&self) -> Option<mpsc::Receiver<GuestEvent>> {
        self.deps.supervisor.as_deref().map(GuestProcess::subscribe)
    }

    /// Projects one supervisor event and performs deferred restart admission.
    pub fn ingest_guest_event(&mut self, event: GuestEvent) {
        let previous = self.guest_state;
        self.guest_state = convert_guest_state(event.state);
        let exit_kind = if self.boot_timeout_pending
            && matches!(
                event.state,
                ome_supervisor::GuestState::Failed | ome_supervisor::GuestState::Stopped
            ) {
            self.boot_timeout_pending = false;
            Some(ExitKind::BootTimeout)
        } else {
            match (previous, event.state) {
                (GuestState::Stopping, ome_supervisor::GuestState::Stopped) => {
                    Some(ExitKind::UserStop)
                }
                (GuestState::Starting, ome_supervisor::GuestState::Failed) => {
                    Some(ExitKind::StartFailed)
                }
                (GuestState::Running, ome_supervisor::GuestState::Failed) => Some(ExitKind::Crash),
                _ => None,
            }
        };
        if let Some(kind) = exit_kind {
            self.last_exit = Some(LastExit {
                kind,
                at: local_rfc3339(),
                log_path: event
                    .logs
                    .as_ref()
                    .map(|logs| logs.stderr.to_string_lossy().into_owned()),
            });
        }
        match event.state {
            ome_supervisor::GuestState::Starting => {
                self.pid = event.pid;
                self.boot_started = Some(Instant::now());
                self.boot_completed = false;
                self.adb_connected = false;
                self.adb_connect_attempted = false;
            }
            ome_supervisor::GuestState::Running => {
                self.pid = event.pid;
                self.started_at = Some(local_rfc3339());
                self.boot_started.get_or_insert_with(Instant::now);
                if let Some(position) = self.selected_guest_index() {
                    self.guests[position].last_started_at = self.started_at.clone();
                    if self.guest_store.save(&self.guests[position]).is_err() {
                        self.push_notice(Notice {
                            at: local_rfc3339(),
                            level: NoticeLevel::Warning,
                            message: "마지막 실행 시각을 저장하지 못했습니다. 저장 공간과 앱의 파일 접근 권한을 확인하십시오."
                                .to_owned(),
                        });
                    }
                }
                self.try_place_guest_window();
            }
            ome_supervisor::GuestState::Restarting => {
                self.pid = event.pid;
                self.last_exit = Some(LastExit {
                    kind: ExitKind::GuestReset,
                    at: local_rfc3339(),
                    log_path: event
                        .logs
                        .map(|logs| logs.stderr.to_string_lossy().into_owned()),
                });
                self.boot_completed = false;
                self.adb_connected = false;
                self.adb_connect_attempted = false;
                self.boot_started = Some(Instant::now());
                self.hosted_pid = None;
                self.hosting = HostingMode::None;
                self.stage_visible = true;
                let _ = self.deps.window_host.detach();
            }
            ome_supervisor::GuestState::Stopped => {
                self.boot_completed = false;
                self.adb_connected = false;
                self.adb_connect_attempted = false;
                self.boot_started = None;
                self.last_account_poll = None;
                self.hosting = HostingMode::None;
                self.pid = None;
                self.started_at = None;
                self.hosted_pid = None;
                self.stage_visible = true;
                let _ = self.deps.window_host.detach();
                if self.restart_pending {
                    self.restart_pending = false;
                    if let Err(issue) = self.start_guest() {
                        self.issue = Some(issue);
                    }
                } else if self.wizard.step == Step::FirstBoot
                    && let Err(issue) = self.start_guest()
                {
                    self.issue = Some(issue);
                }
            }
            ome_supervisor::GuestState::Failed => {
                self.boot_completed = false;
                self.adb_connected = false;
                self.adb_connect_attempted = false;
                self.boot_started = None;
                self.last_account_poll = None;
                self.hosting = HostingMode::None;
                self.pid = None;
                self.started_at = None;
                self.hosted_pid = None;
                self.restart_pending = false;
                self.stage_visible = true;
                let _ = self.deps.window_host.detach();
            }
            ome_supervisor::GuestState::Stopping => {}
        }
    }

    /// Advances one non-blocking adb boot/account poll.
    pub fn tick(&mut self) {
        self.drain_worker_events();
        if self.guest_state != GuestState::Running {
            return;
        }
        if !self.boot_completed {
            let boot_timeout = Duration::from_secs(180);
            #[cfg(test)]
            let boot_timeout = self.boot_timeout_override.unwrap_or(boot_timeout);
            if self
                .boot_started
                .is_some_and(|started| started.elapsed() >= boot_timeout)
            {
                self.boot_timeout_pending = true;
                if let Some(supervisor) = self.deps.supervisor.as_deref() {
                    supervisor.request_stop();
                }
                return;
            }
            let Some(adb) = self.deps.adb.as_ref() else {
                return;
            };
            if !self.adb_connect_attempted {
                self.adb_connect_attempted = true;
                self.adb_connected = adb.connect().is_ok();
                return;
            }
            if adb.boot_completed().unwrap_or(false) {
                self.adb_connected = true;
                self.boot_completed = true;
                self.run_capability_probe();
            }
            return;
        }
        if self
            .last_account_poll
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(15))
        {
            self.last_account_poll = Some(Instant::now());
            self.poll_google_accounts();
        }
    }

    fn run_capability_probe(&mut self) {
        let Some(adb) = self.deps.adb.as_ref() else {
            return;
        };
        let Some(api_level) = self.selected_guest().map(|guest| guest.api_level) else {
            return;
        };
        let runner = AdbShellRunner(adb);
        let (outcome, add_account_supported) =
            if let Some(adapter) = self.deps.family_adapter.as_deref() {
                (
                    CapabilityProbe.run(&runner, adapter),
                    adapter.add_google_account_command().is_some(),
                )
            } else {
                let adapter = adapter_for(api_level);
                (
                    CapabilityProbe.run(&runner, adapter.as_ref()),
                    adapter.add_google_account_command().is_some(),
                )
            };
        self.apply_probe_outcome(outcome, add_account_supported);
    }

    fn apply_probe_outcome(&mut self, outcome: ProbeOutcome, add_account_supported: bool) {
        let probed_at = local_rfc3339();
        self.capabilities = CapabilityReport {
            probed_at: Some(probed_at.clone()),
            items: outcome
                .items
                .iter()
                .map(|(item, state)| capability_item(*item, *state))
                .collect(),
        };
        self.device_id = outcome.device_id.as_ref().map(|id| id.hex.clone());
        self.device_id_decimal = outcome.device_id.as_ref().map(|id| id.decimal.clone());
        self.google_accounts = outcome.google_accounts;
        self.media_volume = outcome.media_volume;
        self.root_enabled = outcome.root_enabled;
        self.resolution = outcome.display.map(|display| Size {
            width: display.width,
            height: display.height,
        });
        self.add_account_supported = add_account_supported;
        self.apps = package_items(
            outcome
                .packages
                .iter()
                .map(|package| ome_adb::InstalledPackage {
                    package: package.package.clone(),
                    version_code: package.version_code,
                })
                .collect(),
        );
        if let Some(position) = self.selected_guest_index() {
            self.guests[position].apply_probe(&outcome, probed_at);
            if self.guest_store.save(&self.guests[position]).is_err() {
                self.push_notice(Notice {
                    at: local_rfc3339(),
                    level: NoticeLevel::Warning,
                    message: "운영체제 능력 조사 결과를 저장하지 못했습니다. 저장 공간과 앱의 파일 접근 권한을 확인하십시오."
                        .to_owned(),
                });
            }
        }
    }

    fn poll_google_accounts(&mut self) {
        let Some(adb) = self.deps.adb.as_ref() else {
            return;
        };
        let Some(command) = self.with_family_adapter(|adapter| adapter.google_accounts_command())
        else {
            return;
        };
        let runner = AdbShellRunner(adb);
        if let Ok(output) = runner.shell(&command) {
            self.google_accounts =
                self.with_family_adapter(|adapter| adapter.parse_google_accounts(&output.stdout));
        }
    }

    fn with_family_adapter<R>(&self, action: impl FnOnce(&dyn FamilyAdapter) -> R) -> R {
        if let Some(adapter) = self.deps.family_adapter.as_deref() {
            action(adapter)
        } else {
            let adapter = adapter_for(self.selected_guest().map_or(0, |guest| guest.api_level));
            action(adapter.as_ref())
        }
    }

    fn run_shell(&self, command: &ome_guest_image::ShellCommand) -> Result<(), AppIssue> {
        let adb = self
            .deps
            .adb
            .as_ref()
            .ok_or_else(issues::operating_system_connection_unavailable)?;
        let output = AdbShellRunner(adb)
            .shell(command)
            .map_err(|_| issues::operating_system_connection_unavailable())?;
        if output.exit_code == 0 {
            Ok(())
        } else {
            Err(issues::operating_system_connection_unavailable())
        }
    }

    fn push_notice(&mut self, notice: Notice) {
        self.notices.insert(0, notice);
        self.notices.truncate(5);
    }

    fn selected_guest(&self) -> Option<&GuestRecord> {
        let id = self.active_guest.as_deref()?;
        self.guests.iter().find(|guest| guest.id == id)
    }

    fn selected_guest_index(&self) -> Option<usize> {
        let id = self.active_guest.as_deref()?;
        self.guests.iter().position(|guest| guest.id == id)
    }

    fn profile_by_id(&self, id: &str) -> Option<&GuestImageProfile> {
        self.image_profiles.iter().find(|profile| profile.id == id)
    }

    fn adb_address(&self) -> String {
        match self.settings.adb_access {
            crate::AdbAccess::Localhost => "127.0.0.1:5555",
            crate::AdbAccess::Network => "0.0.0.0:5555",
        }
        .to_owned()
    }

    fn load_guest_projection(&mut self, position: usize) {
        let guest = &self.guests[position];
        self.capabilities = capability_report(&guest.capabilities);
        self.device_id = guest.device_id.as_ref().map(|id| id.hex.clone());
        self.device_id_decimal = guest.device_id.as_ref().map(|id| id.decimal.clone());
        self.registration_opened_at = guest.registration_opened_at.clone();
        self.root_enabled = guest.root_enabled;
        self.google_accounts = guest.capabilities.google_accounts;
        self.media_volume = guest.capabilities.media_volume;
        self.resolution = guest.capabilities.display.map(|display| Size {
            width: display.width,
            height: display.height,
        });
        self.add_account_supported = adapter_for(guest.api_level)
            .add_google_account_command()
            .is_some();
        self.apps = stored_apps(guest);
    }

    fn clear_guest_projection(&mut self) {
        self.capabilities = CapabilityReport::default();
        self.device_id = None;
        self.device_id_decimal = None;
        self.registration_opened_at = None;
        self.root_enabled = None;
        self.google_accounts = None;
        self.media_volume = None;
        self.resolution = None;
        self.add_account_supported = false;
        self.apps.clear();
    }

    fn select_image(&mut self, id: String) -> Result<(), AppIssue> {
        if self.artifact_cancel.is_some() {
            return Err(issues::operation_in_progress());
        }
        let profile = self
            .image_profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or_else(issues::image_not_found)?;
        self.selected_image = Some(profile.id.clone());
        Ok(())
    }

    fn select_input(&mut self, id: Option<String>) -> Result<(), AppIssue> {
        if id
            .as_ref()
            .is_some_and(|id| !self.input_profiles.iter().any(|profile| profile.id == *id))
        {
            return Err(issues::input_profile_not_found());
        }
        self.active_input = id;
        Ok(())
    }

    fn input_position(&self, id: &str) -> Result<usize, AppIssue> {
        self.input_profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(issues::input_profile_not_found)
    }

    fn delete_input(&mut self, id: String) -> Result<(), AppIssue> {
        let position = self.input_position(&id)?;
        if self.input_profiles[position].bundled {
            return Err(issues::input_profile_bundled());
        }
        self.input_profiles.remove(position);
        if self.active_input.as_deref() == Some(id.as_str()) {
            self.active_input = None;
        }
        Ok(())
    }

    fn save_input(&mut self, profile: InputProfile) -> Result<(), AppIssue> {
        profile
            .validate()
            .map_err(|_| issues::invalid_input_profile())?;
        if let Some(position) = self
            .input_profiles
            .iter()
            .position(|candidate| candidate.id == profile.id)
        {
            if self.input_profiles[position].bundled {
                return Err(issues::input_profile_bundled());
            }
            self.input_profiles[position] = profile;
        } else {
            if profile.bundled {
                return Err(issues::invalid_input_profile());
            }
            self.active_input = Some(profile.id.clone());
            self.input_profiles.push(profile);
        }
        Ok(())
    }

    fn upsert_binding(
        &mut self,
        profile_id: String,
        binding: crate::Binding,
    ) -> Result<(), AppIssue> {
        let position = self.input_position(&profile_id)?;
        if self.input_profiles[position].bundled {
            return Err(issues::input_profile_bundled());
        }
        let mut updated = self.input_profiles[position].clone();
        if let Some(binding_position) = updated
            .bindings
            .iter()
            .position(|candidate| candidate.id == binding.id)
        {
            updated.bindings[binding_position] = binding;
        } else {
            updated.bindings.push(binding);
        }
        updated
            .validate()
            .map_err(|_| issues::invalid_input_profile())?;
        self.input_profiles[position] = updated;
        Ok(())
    }

    fn remove_binding(&mut self, profile_id: String, id: String) -> Result<(), AppIssue> {
        let position = self.input_position(&profile_id)?;
        if self.input_profiles[position].bundled {
            return Err(issues::input_profile_bundled());
        }
        let Some(binding_position) = self.input_profiles[position]
            .bindings
            .iter()
            .position(|binding| binding.id == id)
        else {
            return Err(issues::invalid_input_profile());
        };
        self.input_profiles[position]
            .bindings
            .remove(binding_position);
        Ok(())
    }

    fn apply_display_preset(&mut self, id: String) -> Result<(), AppIssue> {
        let preset = display_presets_for(self.current_orientation())
            .into_iter()
            .find(|preset| preset.id == id)
            .ok_or_else(issues::display_preset_not_found)?;
        if let Some(adb) = self.deps.adb.as_ref() {
            adb.set_display_size(preset.size.width, preset.size.height)
                .and_then(|()| adb.set_display_density(preset.density_dpi))
                .map_err(|_| issues::operating_system_connection_unavailable())?;
        }
        self.active_display = Some(preset.id);
        self.custom_display = None;
        Ok(())
    }

    fn apply_custom_display(&mut self, size: Size, density_dpi: u32) -> Result<(), AppIssue> {
        validate_display(size, density_dpi)?;
        if let Some(adb) = self.deps.adb.as_ref() {
            adb.set_display_size(size.width, size.height)
                .and_then(|()| adb.set_display_density(density_dpi))
                .map_err(|_| issues::operating_system_connection_unavailable())?;
        }
        self.active_display = None;
        self.custom_display = Some(CustomDisplay { size, density_dpi });
        Ok(())
    }

    fn refresh_host(&mut self) {
        self.feature_state = self
            .deps
            .probe
            .hypervisor_platform()
            .unwrap_or(FeatureState::Unknown);
        self.limits = hardware_limits(
            self.deps.probe.total_memory_bytes().ok(),
            self.deps.probe.logical_processors().ok(),
        );
        let report = HostReadiness::inspect(self.deps.probe.as_ref());
        self.host = HostReport {
            rows: report.rows.into_iter().map(convert_host_row).collect(),
            ready: report.verdict != Verdict::Blocked,
            inspected_at: Some(local_rfc3339()),
        };
        self.blocker = blocker_from_host(&self.host);
    }

    fn wizard_continue(&mut self) -> Result<(), AppIssue> {
        let outcome = if self.wizard.step == Step::HostCheck {
            if self.feature_state == FeatureState::Enabled {
                if !self.host.ready {
                    return Err(issues::wizard_cannot_continue());
                }
                Outcome::WhpxAlreadyEnabled
            } else if matches!(
                self.feature_state,
                FeatureState::Disabled | FeatureState::Absent
            ) && self.host.ready
            {
                Outcome::Continue
            } else {
                return Err(issues::wizard_cannot_continue());
            }
        } else {
            Outcome::Continue
        };
        if self.wizard.step != Step::HostCheck
            && !can_continue(self.wizard.step, self.wizard_facts())
        {
            return Err(issues::wizard_cannot_continue());
        }
        if self.wizard.step == Step::GuestInstall && self.installing_guest {
            self.stop_guest()?;
            self.installing_guest = false;
            self.wizard = advance(self.wizard.clone(), outcome);
            return Ok(());
        }
        let before = self.wizard.step;
        self.wizard = advance(self.wizard.clone(), outcome);
        if self.wizard.step == before {
            return Err(issues::wizard_cannot_continue());
        }
        if self.wizard.step == Step::Done {
            self.phase = AppPhase::Main;
        }
        Ok(())
    }

    fn wizard_skip(&mut self) -> Result<(), AppIssue> {
        if !can_skip(self.wizard.step) {
            return Err(issues::wizard_cannot_skip());
        }
        self.wizard = advance(self.wizard.clone(), Outcome::Skip);
        if self.wizard.step == Step::Done {
            self.phase = AppPhase::Main;
        }
        Ok(())
    }

    fn wizard_facts(&self) -> Facts {
        Facts {
            host_ready: self.host.ready,
            whpx_consent: false,
            artifact_verified: self
                .selected_profile()
                .and_then(|profile| {
                    self.deps
                        .artifacts
                        .as_ref()
                        .map(|store| (store, profile.artifact.as_str()))
                })
                .is_some_and(|(store, name)| {
                    matches!(store.verify(name), ome_artifacts::Verification::Verified)
                }),
            guest_installed: !self.guests.is_empty(),
            guest_booted: self.boot_completed,
            app_install_resolved: true,
        }
    }

    fn wizard_view(&self) -> WizardView {
        let facts = self.wizard_facts();
        WizardView {
            step: convert_step(self.wizard.step),
            can_continue: if self.wizard.step == Step::HostCheck {
                self.host.ready
            } else {
                can_continue(self.wizard.step, facts)
            },
            can_skip: can_skip(self.wizard.step),
            download: self.artifact_progress.clone(),
            image_id: self.selected_image.clone(),
            install_guide: self
                .selected_profile()
                .map(|profile| profile.install_guide.clone())
                .unwrap_or_default(),
            disk_size_gib: 32,
            disk_free_bytes: self.deps.probe.free_disk_bytes().ok(),
        }
    }

    fn current_blocker(&self) -> Option<Blocker> {
        if self.phase != AppPhase::Main {
            return None;
        }
        self.blocker.or_else(|| {
            (self.feature_state == FeatureState::Disabled).then_some(Blocker {
                kind: BlockerKind::HypervisorPlatformOff,
            })
        })
    }

    fn current_orientation(&self) -> Orientation {
        if let Some(active_id) = self.active_display.as_deref()
            && let Some(preset) = display_presets_for(Orientation::Landscape)
                .into_iter()
                .find(|preset| preset.id == active_id)
        {
            return preset.orientation;
        }
        self.custom_display
            .map(|custom| orientation_for_size(custom.size))
            .unwrap_or(Orientation::Landscape)
    }

    fn display_presets(&self) -> Vec<DisplayPreset> {
        display_presets_for(self.current_orientation())
    }

    fn selected_profile(&self) -> Option<&GuestImageProfile> {
        let selected = self.selected_image.as_deref()?;
        self.image_profiles
            .iter()
            .find(|profile| profile.id == selected)
    }

    fn artifact_size(&self, name: &str) -> Option<u64> {
        self.artifact_manifest
            .as_ref()?
            .artifacts
            .iter()
            .find(|artifact| artifact.name == name)
            .map(|artifact| artifact.size_bytes)
    }

    fn images_view(&self) -> ImagesView {
        let recommended = recommended_index(&self.image_profiles, &today_utc());
        ImagesView {
            profiles: self
                .image_profiles
                .iter()
                .enumerate()
                .map(|(index, profile)| {
                    image_summary(
                        profile,
                        self.artifact_size(&profile.artifact),
                        index == recommended.unwrap_or(usize::MAX),
                    )
                })
                .collect(),
            guests: self
                .guests
                .iter()
                .map(|guest| GuestSummary {
                    name: guest.id.clone(),
                    image_id: guest.image_id.clone(),
                    android_version: guest.android_version.clone(),
                    disk_size_gib: u32::try_from(
                        guest.disk_bytes.saturating_add((1_u64 << 30) - 1) >> 30,
                    )
                    .unwrap_or(u32::MAX),
                    last_started_at: guest.last_started_at.clone(),
                    capabilities: capability_report(&guest.capabilities),
                })
                .collect(),
            active_guest: self.active_guest.clone(),
        }
    }
}

fn safe_guest_id(image_id: &str) -> Option<String> {
    let mut id = String::with_capacity(image_id.len().min(70));
    let mut separator = false;
    for byte in image_id.bytes() {
        let valid = byte.is_ascii_alphanumeric() || byte == b'_';
        if valid {
            if id.len() >= 70 {
                break;
            }
            id.push(char::from(byte));
            separator = false;
        } else if !separator && !id.is_empty() {
            if id.len() >= 70 {
                break;
            }
            id.push('-');
            separator = true;
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    (!id.is_empty()).then_some(id)
}

fn stored_apps(guest: &GuestRecord) -> Vec<AppItem> {
    package_items(
        guest
            .capabilities
            .packages
            .iter()
            .map(|package| ome_adb::InstalledPackage {
                package: package.package.clone(),
                version_code: package.version_code,
            })
            .collect(),
    )
}

fn package_items(packages: Vec<ome_adb::InstalledPackage>) -> Vec<AppItem> {
    let mut items = packages
        .into_iter()
        .map(|package| AppItem {
            label: package.package.clone(),
            version_name: None,
            version_code: package.version_code,
            package: package.package,
            installed_at: None,
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.package.cmp(&right.package));
    items
}

/// Accepts existing APK, XAPK, and APKS files without exposing paths to the webview.
pub fn validate_app_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, AppIssue> {
    let mut valid = Vec::with_capacity(paths.len());
    for path in paths {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        if !path.is_file() || !matches!(extension.as_deref(), Some("apk" | "xapk" | "apks")) {
            return Err(issues::app_package_invalid());
        }
        valid.push(path);
    }
    Ok(valid)
}

fn transfer_progress(
    stage: TransferStage,
    done_bytes: u64,
    total_bytes: Option<u64>,
    bytes_per_second: Option<u64>,
    label: String,
) -> TransferProgress {
    TransferProgress {
        stage,
        done_bytes,
        total_bytes,
        bytes_per_second,
        label,
    }
}

fn install_progress(
    label: &str,
    stage: TransferStage,
    done_items: u64,
    total_items: u64,
) -> InstallProgress {
    InstallProgress {
        label: label.to_owned(),
        stage,
        done_items,
        total_items,
        ratio: (total_items > 0).then_some(done_items as f64 / total_items as f64),
        done_bytes: done_items,
        total_bytes: Some(total_items),
    }
}

fn fetch_release_document(
    http: &dyn ome_artifacts::HttpFetch,
) -> Result<ReleaseDocument, AppIssue> {
    const URL: &str = "https://api.github.com/repos/115dkk/Open-Mobile-Emulator/releases/latest";
    let hosts = ome_artifacts::AllowedHosts(vec!["api.github.com".to_owned()]);
    if !hosts.permits(URL) {
        return Err(issues::update_check_failed());
    }
    let mut bytes = Vec::new();
    let outcome = http
        .fetch(URL, None, &mut bytes, &mut |_| true)
        .map_err(|_| issues::update_check_failed())?;
    if outcome
        .final_url
        .as_deref()
        .is_some_and(|url| !hosts.permits(url))
    {
        return Err(issues::update_check_failed());
    }
    if outcome.status == 404 {
        return Err(issues::update_up_to_date());
    }
    if outcome.status != 200 {
        return Err(issues::update_check_failed());
    }
    serde_json::from_slice(&bytes).map_err(|_| issues::update_check_failed())
}

fn release_from_document(
    document: ReleaseDocument,
    product_version: &str,
) -> Result<UpdateRelease, AppIssue> {
    let available = Version::parse(document.tag_name.trim_start_matches(['v', 'V']))
        .map_err(|_| issues::update_check_failed())?;
    let current = Version::parse(product_version).map_err(|_| issues::update_check_failed())?;
    if available <= current {
        return Err(issues::update_up_to_date());
    }
    if !trusted_release_notes_url(&document.html_url) {
        return Err(issues::update_check_failed());
    }
    let mut installers = document.assets.iter().filter(|asset| {
        let lower = asset.name.to_ascii_lowercase();
        !lower.ends_with(".sha256")
            && matches!(
                Path::new(&lower)
                    .extension()
                    .and_then(|value| value.to_str()),
                Some("exe" | "msi" | "msix")
            )
    });
    let installer = installers
        .next()
        .cloned()
        .ok_or_else(issues::update_up_to_date)?;
    if installers.next().is_some() {
        return Err(issues::update_check_failed());
    }
    if !is_safe_leaf(&installer.name) || !trusted_update_asset_url(&installer.browser_download_url)
    {
        return Err(issues::update_check_failed());
    }
    let checksum = document
        .assets
        .iter()
        .find(|asset| asset.name == format!("{}.sha256", installer.name))
        .cloned();
    if checksum.as_ref().is_some_and(|asset| {
        !is_safe_leaf(&asset.name) || !trusted_update_asset_url(&asset.browser_download_url)
    }) {
        return Err(issues::update_check_failed());
    }
    let notes_url =
        (!document.body.is_empty() || !document.html_url.is_empty()).then_some(document.html_url);
    Ok(UpdateRelease {
        version: available.to_string(),
        notes_url,
        installer,
        checksum,
    })
}

fn is_safe_leaf(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        && Path::new(name).components().count() == 1
}

fn trusted_update_asset_url(url: &str) -> bool {
    ome_artifacts::AllowedHosts(vec![
        "github.com".to_owned(),
        "objects.githubusercontent.com".to_owned(),
    ])
    .permits(url)
        && url
            .strip_prefix("https://github.com/")
            .is_none_or(|path| path.starts_with("115dkk/Open-Mobile-Emulator/releases/download/"))
}

fn fetch_to_bytes(
    http: &dyn ome_artifacts::HttpFetch,
    url: &str,
    hosts: &ome_artifacts::AllowedHosts,
) -> Result<Vec<u8>, AppIssue> {
    if !hosts.permits(url) {
        return Err(issues::update_download_failed());
    }
    let mut bytes = Vec::new();
    let outcome = http
        .fetch(url, None, &mut bytes, &mut |_| true)
        .map_err(|_| issues::update_download_failed())?;
    if outcome.status != 200
        || outcome
            .final_url
            .as_deref()
            .is_some_and(|final_url| !hosts.permits(final_url))
    {
        return Err(issues::update_download_failed());
    }
    Ok(bytes)
}

fn download_update_release(
    release: &UpdateRelease,
    checksum: &ReleaseAsset,
    directory: &Path,
    sender: &mpsc::Sender<WorkerEvent>,
    http: &dyn ome_artifacts::HttpFetch,
) -> Result<DownloadedUpdate, AppIssue> {
    if !is_safe_leaf(&release.installer.name) || !is_safe_leaf(&checksum.name) {
        return Err(issues::update_check_failed());
    }
    let hosts = ome_artifacts::AllowedHosts(vec![
        "github.com".to_owned(),
        "objects.githubusercontent.com".to_owned(),
    ]);
    let checksum_bytes = fetch_to_bytes(http, &checksum.browser_download_url, &hosts)?;
    let checksum_text =
        std::str::from_utf8(&checksum_bytes).map_err(|_| issues::update_unverified())?;
    let expected = checksum_text
        .split_whitespace()
        .find(|word| word.len() == 64 && word.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(issues::update_unverified)?;
    if !hosts.permits(&release.installer.browser_download_url) {
        return Err(issues::update_download_failed());
    }
    let staging = directory.join(format!("{}.part", release.installer.name));
    let final_path = directory.join(&release.installer.name);
    let mut file = std::fs::File::create(&staging).map_err(|_| issues::update_download_failed())?;
    let started = Instant::now();
    let mut done = 0_u64;
    let outcome = http
        .fetch(
            &release.installer.browser_download_url,
            None,
            &mut file,
            &mut |bytes| {
                done = done.saturating_add(bytes);
                let rate = (started.elapsed().as_secs_f64() > 0.0)
                    .then(|| (done as f64 / started.elapsed().as_secs_f64()) as u64);
                let _ = sender.send(WorkerEvent::UpdateProgress(transfer_progress(
                    TransferStage::Transferring,
                    done,
                    Some(release.installer.size),
                    rate,
                    release.installer.name.clone(),
                )));
                true
            },
        )
        .map_err(|_| issues::update_download_failed())?;
    if outcome.status != 200
        || outcome
            .final_url
            .as_deref()
            .is_some_and(|url| !hosts.permits(url))
        || done != release.installer.size
    {
        let _ = fs::remove_file(staging);
        return Err(issues::update_download_failed());
    }
    use std::io::{Read, Write};
    file.flush().map_err(|_| issues::update_download_failed())?;
    drop(file);
    let mut source = std::fs::File::open(&staging).map_err(|_| issues::update_download_failed())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|_| issues::update_download_failed())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    if !format!("{:x}", hasher.finalize()).eq_ignore_ascii_case(expected) {
        let _ = fs::remove_file(&staging);
        return Err(issues::update_unverified());
    }
    fs::rename(&staging, &final_path).map_err(|_| issues::update_download_failed())?;
    Ok(DownloadedUpdate {
        version: release.version.clone(),
        path: final_path,
    })
}

fn trusted_release_notes_url(url: &str) -> bool {
    url.strip_prefix("https://github.com/").is_some_and(|path| {
        path.starts_with("115dkk/Open-Mobile-Emulator/releases/")
            || path == "115dkk/Open-Mobile-Emulator/releases"
    })
}

fn local_rfc3339() -> String {
    Zoned::now()
        .round(Unit::Second)
        .unwrap_or_else(|_| Zoned::now())
        .strftime("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

fn local_filename_time() -> String {
    Zoned::now().strftime("%Y%m%d-%H%M%S").to_string()
}

fn convert_guest_state(state: ome_supervisor::GuestState) -> GuestState {
    match state {
        ome_supervisor::GuestState::Stopped => GuestState::Stopped,
        ome_supervisor::GuestState::Starting => GuestState::Starting,
        ome_supervisor::GuestState::Running => GuestState::Running,
        ome_supervisor::GuestState::Stopping => GuestState::Stopping,
        ome_supervisor::GuestState::Restarting => GuestState::Restarting,
        ome_supervisor::GuestState::Failed => GuestState::Failed,
    }
}

fn capability_item(item: ProbeItem, state: ProbeState) -> crate::CapabilityItem {
    crate::CapabilityItem {
        id: match item {
            ProbeItem::BootMarker => CapabilityId::BootMarker,
            ProbeItem::AppList => CapabilityId::AppList,
            ProbeItem::DisplaySize => CapabilityId::DisplaySize,
            ProbeItem::MediaVolume => CapabilityId::MediaVolume,
            ProbeItem::DeviceId => CapabilityId::DeviceId,
            ProbeItem::Screenshot => CapabilityId::Screenshot,
            ProbeItem::ForegroundApp => CapabilityId::ForegroundApp,
            ProbeItem::Multitouch => CapabilityId::Multitouch,
            ProbeItem::NativeBridge => CapabilityId::NativeBridge,
            ProbeItem::Root => CapabilityId::Root,
        },
        state: match state {
            ProbeState::Available => Capability::Available,
            ProbeState::Unavailable => Capability::Unavailable,
            ProbeState::Unknown => Capability::Unknown,
        },
    }
}

fn capability_report(stored: &crate::guest_store::StoredCapabilities) -> CapabilityReport {
    CapabilityReport {
        probed_at: stored.probed_at.clone(),
        items: stored
            .items
            .iter()
            .map(|item| crate::CapabilityItem {
                id: match item.id {
                    StoredProbeItem::BootMarker => CapabilityId::BootMarker,
                    StoredProbeItem::AppList => CapabilityId::AppList,
                    StoredProbeItem::DisplaySize => CapabilityId::DisplaySize,
                    StoredProbeItem::MediaVolume => CapabilityId::MediaVolume,
                    StoredProbeItem::DeviceId => CapabilityId::DeviceId,
                    StoredProbeItem::Screenshot => CapabilityId::Screenshot,
                    StoredProbeItem::ForegroundApp => CapabilityId::ForegroundApp,
                    StoredProbeItem::Multitouch => CapabilityId::Multitouch,
                    StoredProbeItem::NativeBridge => CapabilityId::NativeBridge,
                    StoredProbeItem::Root => CapabilityId::Root,
                },
                state: match item.state {
                    StoredProbeState::Available => Capability::Available,
                    StoredProbeState::Unavailable => Capability::Unavailable,
                    StoredProbeState::Unknown => Capability::Unknown,
                },
            })
            .collect(),
    }
}

fn capability_value(report: &CapabilityReport, id: CapabilityId) -> Capability {
    report
        .items
        .iter()
        .find(|item| item.id == id)
        .map_or(Capability::Unknown, |item| item.state)
}

fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400;
    let (year, month, day) = civil_from_days(i64::try_from(days).unwrap_or(i64::MAX));
    format!("{year:04}-{month:02}-{day:02}")
}

// Howard Hinnant's public-domain civil calendar conversion for days since 1970-01-01.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch.saturating_add(719_468);
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (
        year,
        u32::try_from(month).expect("calendar month is positive"),
        u32::try_from(day).expect("calendar day is positive"),
    )
}

fn empty_host_report() -> HostReport {
    HostReport {
        rows: Vec::new(),
        ready: false,
        inspected_at: None,
    }
}

fn bundled_profiles() -> Vec<InputProfile> {
    let embedded = include_str!("../../../presets/com.epidgames.trickcalrevive.json");
    InputProfile::parse(embedded)
        .map(|mut profile| {
            profile.bundled = true;
            vec![profile]
        })
        .unwrap_or_default()
}

fn home_profiles_directory(home: &Path) -> Result<PathBuf, std::io::Error> {
    let directory = home.join("profiles");
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

/// Loads validated input profiles from a trusted native directory.
pub fn load_input_directory(
    directory: impl AsRef<Path>,
) -> Result<Vec<InputProfile>, ome_input::ProfileError> {
    load_profiles(directory)
}

/// Returns the three fixed display presets assuming the default landscape orientation.
pub fn display_presets() -> Vec<DisplayPreset> {
    display_presets_for(Orientation::Landscape)
}

fn display_presets_for(current_orientation: Orientation) -> Vec<DisplayPreset> {
    vec![
        DisplayPreset {
            id: "hd-720".to_owned(),
            size: Size {
                width: 1280,
                height: 720,
            },
            density_dpi: 160,
            orientation: Orientation::Landscape,
            needs_reboot: current_orientation != Orientation::Landscape,
        },
        DisplayPreset {
            id: "full-hd".to_owned(),
            size: Size {
                width: 1920,
                height: 1080,
            },
            density_dpi: 240,
            orientation: Orientation::Landscape,
            needs_reboot: current_orientation != Orientation::Landscape,
        },
        DisplayPreset {
            id: "portrait-720".to_owned(),
            size: Size {
                width: 720,
                height: 1280,
            },
            density_dpi: 160,
            orientation: Orientation::Portrait,
            needs_reboot: current_orientation != Orientation::Portrait,
        },
    ]
}

fn orientation_for_size(size: Size) -> Orientation {
    if size.height > size.width {
        Orientation::Portrait
    } else {
        Orientation::Landscape
    }
}

fn refresh_rates(custom: Option<u32>) -> Vec<u32> {
    let mut rates = vec![60, 75, 90, 120, 144];
    if let Some(custom) = custom
        && !rates.contains(&custom)
    {
        rates.push(custom);
        rates.sort_unstable();
    }
    rates
}

fn validate_display(size: Size, density_dpi: u32) -> Result<(), AppIssue> {
    let valid_dimension = |value: u32| (640..=7680).contains(&value) && value.is_multiple_of(8);
    if !valid_dimension(size.width)
        || !valid_dimension(size.height)
        || !(120..=640).contains(&density_dpi)
    {
        Err(issues::invalid_display())
    } else {
        Ok(())
    }
}

fn image_summary(
    profile: &GuestImageProfile,
    size_bytes: Option<u64>,
    recommended: bool,
) -> GuestImageSummary {
    GuestImageSummary {
        id: profile.id.clone(),
        display_name: profile.display_name.clone(),
        android_version: profile.android_version.clone(),
        api_level: profile.api_level,
        distribution: match profile.distribution {
            Distribution::Bliss => ImageDistribution::Bliss,
            Distribution::AndroidX86 => ImageDistribution::AndroidX86,
            Distribution::SelfBuilt => ImageDistribution::SelfBuilt,
        },
        translator: match profile.translator {
            Translator::Houdini => ImageTranslator::Houdini,
            Translator::NdkTranslation => ImageTranslator::NdkTranslation,
            Translator::Digitalis => ImageTranslator::Digitalis,
            Translator::None => ImageTranslator::None,
        },
        size_bytes,
        status: match profile.status {
            ProfileStatus::Verified => ImageStatus::Verified,
            ProfileStatus::Candidate => ImageStatus::Candidate,
            ProfileStatus::Deprecated => ImageStatus::Deprecated,
        },
        released_at: profile.released_at.clone(),
        verified_games: u32::try_from(profile.verifications.len()).unwrap_or(u32::MAX),
        recommended,
    }
}

fn blocker_from_host(report: &HostReport) -> Option<Blocker> {
    let blocked = |id| {
        report
            .rows
            .iter()
            .any(|row| row.id == id && row.status == HostStatus::Blocked)
    };
    if blocked(HostCheckId::CpuVirtualization) {
        Some(Blocker {
            kind: BlockerKind::VirtualizationOff,
        })
    } else if blocked(HostCheckId::QemuPresent) || blocked(HostCheckId::FirmwarePresent) {
        Some(Blocker {
            kind: BlockerKind::QemuMissing,
        })
    } else if blocked(HostCheckId::HypervisorPlatform) {
        Some(Blocker {
            kind: BlockerKind::HypervisorPlatformOff,
        })
    } else {
        None
    }
}

fn guest_store_issue(error: GuestStoreError) -> AppIssue {
    eprintln!("operating-system storage failed: {error}");
    issues::guest_storage_unavailable()
}

fn settings_issue(error: SettingsError) -> AppIssue {
    match error {
        SettingsError::InvalidInput => issues::invalid_settings(),
        SettingsError::UnsupportedVersion
        | SettingsError::TooLarge
        | SettingsError::InvalidDocument
        | SettingsError::RecoveryRequired
        | SettingsError::Json(_) => issues::settings_invalid_document(),
        SettingsError::Io(_) => issues::settings_unavailable(),
    }
}

fn convert_host_row(row: ome_host_check::HostRow) -> HostRow {
    HostRow {
        id: match row.id {
            ome_host_check::HostCheckId::CpuVirtualization => HostCheckId::CpuVirtualization,
            ome_host_check::HostCheckId::HypervisorPlatform => HostCheckId::HypervisorPlatform,
            ome_host_check::HostCheckId::RebootPending => HostCheckId::RebootPending,
            ome_host_check::HostCheckId::WhpxAvailable => HostCheckId::WhpxAvailable,
            ome_host_check::HostCheckId::QemuPresent => HostCheckId::QemuPresent,
            ome_host_check::HostCheckId::FirmwarePresent => HostCheckId::FirmwarePresent,
            ome_host_check::HostCheckId::AdbPresent => HostCheckId::AdbPresent,
            ome_host_check::HostCheckId::DiskSpace => HostCheckId::DiskSpace,
        },
        status: match row.status {
            ome_host_check::HostStatus::Ready => HostStatus::Ready,
            ome_host_check::HostStatus::Attention => HostStatus::Attention,
            ome_host_check::HostStatus::Blocked => HostStatus::Blocked,
        },
        detail: row.detail,
    }
}

fn convert_step(step: Step) -> WizardStep {
    match step {
        Step::HostCheck => WizardStep::HostCheck,
        Step::WhpxConsent => WizardStep::WhpxConsent,
        Step::RebootPending => WizardStep::RebootPending,
        Step::ArtifactDownload => WizardStep::ArtifactDownload,
        Step::GuestInstall => WizardStep::GuestInstall,
        Step::FirstBoot => WizardStep::FirstBoot,
        Step::AppInstall => WizardStep::AppInstall,
        Step::Done => WizardStep::Done,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use crate::{ElevationLauncher, NativeProcessRunner};
    use ome_adb::{Output, RecordedRunner};
    use ome_artifacts::{FetchError, FetchOutcome, HttpFetch};
    use ome_guest_image::{
        Attempt, DeviceId, DisplayInfo, GuestFamily, PackageEntry, ShellCommand,
    };
    use ome_host_check::{AdbFound, HostCheckId as ProbeId, ProbeValue, QemuFound, TableProbe};
    use ome_supervisor::LogPaths;

    use super::*;
    use crate::desktop::RecordingDesktop;
    use crate::{AdbAccess, CloseAction, GpuMode, SettingsInput};

    #[derive(Clone, Debug)]
    struct FakeElevation(Result<u32, ElevationError>);

    impl ElevationLauncher for FakeElevation {
        fn enable_whpx(&self) -> Result<u32, ElevationError> {
            self.0.clone()
        }
    }

    type NativeCalls = Vec<(PathBuf, Vec<OsString>)>;

    #[derive(Clone, Debug, Default)]
    struct CreatingDiskRunner {
        calls: Arc<Mutex<NativeCalls>>,
    }

    impl NativeProcessRunner for CreatingDiskRunner {
        fn run(&self, program: &Path, args: &[OsString]) -> Result<i32, String> {
            self.calls
                .lock()
                .map_err(|_| "native calls lock failed".to_owned())?
                .push((program.to_path_buf(), args.to_vec()));
            let disk = args
                .get(3)
                .map(PathBuf::from)
                .ok_or_else(|| "missing disk argument".to_owned())?;
            fs::write(disk, [0_u8; 1]).map_err(|error| error.to_string())?;
            Ok(0)
        }
    }

    #[derive(Clone, Debug)]
    struct FixedHttp {
        body: Vec<u8>,
        final_url: String,
        status: u16,
    }

    impl HttpFetch for FixedHttp {
        fn fetch(
            &self,
            _url: &str,
            range_start: Option<u64>,
            sink: &mut dyn Write,
            progress: &mut dyn FnMut(u64) -> bool,
        ) -> Result<FetchOutcome, FetchError> {
            sink.write_all(&self.body).map_err(FetchError::Io)?;
            let count = u64::try_from(self.body.len()).expect("test body fits u64");
            if !progress(count) {
                return Err(FetchError::Cancelled);
            }
            Ok(FetchOutcome {
                status: if range_start.is_some() {
                    206
                } else {
                    self.status
                },
                final_url: Some(self.final_url.clone()),
                bytes_written: count,
            })
        }
    }

    fn ready_probe(feature: FeatureState) -> TableProbe {
        TableProbe::new()
            .with(ProbeId::CpuVirtualization, ProbeValue::Bool(true))
            .with(ProbeId::HypervisorPlatform, ProbeValue::Feature(feature))
            .with(ProbeId::RebootPending, ProbeValue::Bool(false))
            .with(ProbeId::WhpxAvailable, ProbeValue::Bool(true))
            .with(
                ProbeId::QemuPresent,
                ProbeValue::Qemu(Some(QemuFound {
                    version: "11.1".to_owned(),
                    source: "bundle".to_owned(),
                    program: "C:/qemu/qemu-system-x86_64.exe".to_owned(),
                    firmware_code: Some("C:/qemu/share/edk2-x86_64-code.fd".to_owned()),
                    firmware_vars_template: Some("C:/qemu/share/edk2-i386-vars.fd".to_owned()),
                })),
            )
            .with(ProbeId::FirmwarePresent, ProbeValue::Bool(true))
            .with(
                ProbeId::AdbPresent,
                ProbeValue::Adb(Some(AdbFound {
                    version: "37".to_owned(),
                    program: "C:/adb.exe".to_owned(),
                })),
            )
            .with(
                ProbeId::DiskSpace,
                ProbeValue::Bytes(50 * 1024 * 1024 * 1024),
            )
            .with_total_memory_bytes(ProbeValue::TotalMemoryBytes(64 * 1024 * 1024 * 1024))
            .with_logical_processors(ProbeValue::LogicalProcessors(16))
    }

    #[derive(Clone, Debug)]
    struct ScriptedGuest {
        state: Arc<Mutex<ome_supervisor::GuestState>>,
        starts: Arc<Mutex<Vec<(GuestConfig, GuestPaths, QemuInstall)>>>,
        stop_requests: Arc<Mutex<u32>>,
    }

    impl Default for ScriptedGuest {
        fn default() -> Self {
            Self {
                state: Arc::new(Mutex::new(ome_supervisor::GuestState::Stopped)),
                starts: Arc::new(Mutex::new(Vec::new())),
                stop_requests: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl GuestProcess for ScriptedGuest {
        fn start(
            &mut self,
            config: GuestConfig,
            paths: GuestPaths,
            install: QemuInstall,
        ) -> Result<(), String> {
            self.starts
                .lock()
                .map_err(|_| "starts lock failed".to_owned())?
                .push((config, paths, install));
            *self
                .state
                .lock()
                .map_err(|_| "state lock failed".to_owned())? =
                ome_supervisor::GuestState::Starting;
            Ok(())
        }

        fn start_install(
            &mut self,
            config: GuestConfig,
            paths: GuestPaths,
            install: QemuInstall,
        ) -> Result<(), String> {
            self.start(config, paths, install)
        }

        fn request_stop(&self) {
            *self.stop_requests.lock().expect("stop request lock") += 1;
        }

        fn state(&self) -> ome_supervisor::GuestState {
            *self.state.lock().expect("guest state lock")
        }

        fn subscribe(&self) -> mpsc::Receiver<GuestEvent> {
            let (_sender, receiver) = mpsc::channel();
            receiver
        }
    }

    #[derive(Clone, Debug)]
    struct RecordingWindow {
        attach_result: Result<(), HostingIssue>,
        attached: Arc<Mutex<bool>>,
        targets: Arc<Mutex<Vec<HostingTarget>>>,
        placements: Arc<Mutex<Vec<ome_window_host::Rect>>>,
        hide_count: Arc<Mutex<u32>>,
        show_count: Arc<Mutex<u32>>,
        detach_count: Arc<Mutex<u32>>,
    }

    impl RecordingWindow {
        fn embedded() -> Self {
            Self {
                attach_result: Ok(()),
                attached: Arc::new(Mutex::new(false)),
                targets: Arc::new(Mutex::new(Vec::new())),
                placements: Arc::new(Mutex::new(Vec::new())),
                hide_count: Arc::new(Mutex::new(0)),
                show_count: Arc::new(Mutex::new(0)),
                detach_count: Arc::new(Mutex::new(0)),
            }
        }

        fn separate() -> Self {
            Self {
                attach_result: Err(HostingIssue::Unwired),
                ..Self::embedded()
            }
        }
    }

    impl WindowPlacement for RecordingWindow {
        fn attach(&mut self, target: HostingTarget) -> Result<(), HostingIssue> {
            self.targets.lock().expect("targets lock").push(target);
            self.attach_result?;
            *self.attached.lock().expect("attached lock") = true;
            Ok(())
        }

        fn place(&mut self, rect: ome_window_host::Rect) -> Result<(), HostingIssue> {
            self.placements.lock().expect("placements lock").push(rect);
            Ok(())
        }

        fn hide(&mut self) -> Result<(), HostingIssue> {
            *self.hide_count.lock().expect("hide lock") += 1;
            Ok(())
        }

        fn show(&mut self) -> Result<(), HostingIssue> {
            *self.show_count.lock().expect("show lock") += 1;
            Ok(())
        }

        fn detach(&mut self) -> Result<(), HostingIssue> {
            *self.detach_count.lock().expect("detach lock") += 1;
            *self.attached.lock().expect("attached lock") = false;
            Ok(())
        }

        fn to_front(&mut self) -> Result<(), HostingIssue> {
            Ok(())
        }

        fn is_attached(&self) -> bool {
            *self.attached.lock().expect("attached lock")
        }

        fn client_screen_rect(&self) -> Option<Rect> {
            self.placements
                .lock()
                .expect("placements lock")
                .last()
                .map(|rect| Rect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                })
        }
    }

    #[derive(Debug)]
    struct FakeFamilyAdapter;

    impl FamilyAdapter for FakeFamilyAdapter {
        fn family(&self) -> GuestFamily {
            GuestFamily::Modern
        }
        fn boot_completed_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-boot"]))
        }
        fn parse_boot_completed(&self, output: &str) -> bool {
            output.trim() == "booted"
        }
        fn native_bridge_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-bridge"]))
        }
        fn parse_native_bridge(&self, output: &str) -> Option<String> {
            (output.trim() == "bridge.so").then(|| "bridge.so".to_owned())
        }
        fn packages_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-packages"]))
        }
        fn parse_packages(&self, output: &str) -> Vec<PackageEntry> {
            (output.trim() == "package")
                .then(|| PackageEntry {
                    package: "dev.ome.sample".to_owned(),
                    version_code: Some(1),
                })
                .into_iter()
                .collect()
        }
        fn app_label_command(&self, _package: &str) -> Option<ShellCommand> {
            None
        }
        fn parse_app_label(&self, _output: &str) -> Option<String> {
            None
        }
        fn foreground_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-foreground"]))
        }
        fn parse_foreground(&self, output: &str) -> Option<String> {
            (output.trim() == "foreground").then(|| "dev.ome.sample".to_owned())
        }
        fn media_volume_set_command(&self, index: u32) -> Option<ShellCommand> {
            Some(ShellCommand::new(["set-volume", &index.to_string()]))
        }
        fn media_volume_get_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-volume"]))
        }
        fn parse_media_volume(&self, output: &str) -> Option<u32> {
            output.trim().parse().ok()
        }
        fn display_size_command(&self, width: u32, height: u32) -> Option<ShellCommand> {
            Some(ShellCommand::new([
                "set-size",
                &format!("{width}x{height}"),
            ]))
        }
        fn display_size_reset_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["reset-size"]))
        }
        fn display_density_command(&self, density_dpi: u32) -> Option<ShellCommand> {
            Some(ShellCommand::new(["set-density", &density_dpi.to_string()]))
        }
        fn display_size_query_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-size"]))
        }
        fn display_density_query_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-density"]))
        }
        fn parse_display(&self, size: &str, density: &str) -> Option<DisplayInfo> {
            (size.trim() == "1280x720" && density.trim() == "160").then_some(DisplayInfo {
                width: 1280,
                height: 720,
                density_dpi: 160,
            })
        }
        fn device_id_attempts(&self) -> Vec<Attempt> {
            vec![Attempt {
                push: None,
                command: ShellCommand::new(["probe-device-id"]),
            }]
        }
        fn parse_device_id(&self, output: &str) -> Option<DeviceId> {
            DeviceId::from_decimal(output)
        }
        fn google_accounts_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-accounts"]))
        }
        fn parse_google_accounts(&self, output: &str) -> Option<u32> {
            output.trim().parse().ok()
        }
        fn add_google_account_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["open-add-account"]))
        }
        fn root_state_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-root"]))
        }
        fn parse_root_state(&self, output: &str) -> Option<bool> {
            match output.trim() {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            }
        }
        fn input_devices_command(&self) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-input"]))
        }
        fn parse_multitouch(&self, output: &str) -> Option<bool> {
            match output.trim() {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            }
        }
        fn screenshot_command(&self, _remote_path: &str) -> Option<ShellCommand> {
            Some(ShellCommand::new(["probe-screenshot"]))
        }
    }

    fn output(stdout: &str) -> Result<Output, ome_adb::RecordedError> {
        Ok(Output {
            exit_code: 0,
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    fn lifecycle_runtime(
        outputs: impl IntoIterator<Item = Result<Output, ome_adb::RecordedError>>,
        desktop: RecordingDesktop,
        window: RecordingWindow,
    ) -> (tempfile::TempDir, AppRuntime, ScriptedGuest, RecordedRunner) {
        let directory = tempfile::tempdir().expect("temp directory");
        let home_path = directory.path().join("home");
        let guest_dir = home_path.join("vm/default");
        fs::create_dir_all(&guest_dir).expect("guest directory");
        fs::write(guest_dir.join("disk.qcow2"), [0_u8; 17]).expect("disk");
        fs::write(guest_dir.join("efivars.fd"), [0_u8; 4]).expect("firmware vars");
        let firmware = directory.path().join("firmware.fd");
        fs::write(&firmware, [0_u8; 4]).expect("firmware code");
        let qemu = directory.path().join("qemu.exe");
        fs::write(&qemu, []).expect("qemu placeholder");
        let adb = directory.path().join("adb.exe");
        fs::write(&adb, []).expect("adb placeholder");
        let supervisor = ScriptedGuest::default();
        let supervisor_handle = supervisor.clone();
        let runner = RecordedRunner::new(outputs);
        let runner_handle = runner.clone();
        let probe = ready_probe(FeatureState::Enabled)
            .with(
                ProbeId::QemuPresent,
                ProbeValue::Qemu(Some(QemuFound {
                    version: "test".to_owned(),
                    source: "test".to_owned(),
                    program: qemu.to_string_lossy().into_owned(),
                    firmware_code: Some(firmware.to_string_lossy().into_owned()),
                    firmware_vars_template: None,
                })),
            )
            .with(
                ProbeId::AdbPresent,
                ProbeValue::Adb(Some(AdbFound {
                    version: "test".to_owned(),
                    program: adb.to_string_lossy().into_owned(),
                })),
            );
        let session = AdbSession::new(&adb, "127.0.0.1:5555".to_owned(), Box::new(runner))
            .expect("adb session");
        let runtime = AppRuntime::open(
            OmeHome::from_path(home_path).expect("home"),
            RuntimeDeps {
                probe: Box::new(probe),
                artifacts: None,
                adb: Some(session),
                supervisor: Some(Box::new(supervisor)),
                desktop: Box::new(desktop),
                window_host: Box::new(window),
                family_adapter: Some(Box::new(FakeFamilyAdapter)),
                images_dir: None,
                artifacts_manifest: None,
                product_version: "0.1.0".to_owned(),
            },
        )
        .expect("runtime");
        let mut runtime = runtime;
        runtime.image_profiles.push(GuestImageProfile {
            id: "bliss-16.9.7-android-13".to_owned(),
            display_name: "안드로이드 13".to_owned(),
            android_version: "13".to_owned(),
            api_level: 33,
            distribution: Distribution::Bliss,
            artifact: "test-artifact".to_owned(),
            translator: Translator::NdkTranslation,
            boot_args: vec!["quiet".to_owned()],
            grub_entry_hint: "Virgl".to_owned(),
            install_guide: vec![String::new(); 6],
            qemu_overrides: Vec::new(),
            status: ProfileStatus::Verified,
            released_at: Some("2024-10-11".to_owned()),
            verifications: Vec::new(),
        });
        runtime.selected_image = Some("bliss-16.9.7-android-13".to_owned());
        (directory, runtime, supervisor_handle, runner_handle)
    }

    fn guest_event(
        state: ome_supervisor::GuestState,
        pid: Option<u32>,
        stderr: Option<&Path>,
    ) -> GuestEvent {
        GuestEvent {
            state,
            pid,
            logs: stderr.map(|stderr| LogPaths {
                stdout: stderr.with_extension("stdout.log"),
                stderr: stderr.to_path_buf(),
                command: stderr.with_extension("cmd.log"),
            }),
        }
    }

    fn runtime(feature: FeatureState) -> (tempfile::TempDir, AppRuntime) {
        let directory = tempfile::tempdir().expect("temp directory");
        let home = OmeHome::from_path(directory.path().join("home")).expect("home");
        let runtime = AppRuntime::open(
            home,
            RuntimeDeps {
                probe: Box::new(ready_probe(feature)),
                artifacts: None,
                adb: None,
                supervisor: None,
                desktop: Box::new(crate::UnavailableDesktop),
                window_host: Box::new(GuestWindowHost::default()),
                family_adapter: None,
                images_dir: None,
                artifacts_manifest: None,
                product_version: "0.1.0".to_owned(),
            },
        )
        .expect("runtime");
        (directory, runtime)
    }

    #[test]
    fn image_profiles_load_with_manifest_size_and_recommendation() {
        let directory = tempfile::tempdir().expect("temp directory");
        let images = directory.path().join("images");
        fs::create_dir(&images).expect("images directory");
        fs::write(
            images.join("profile.json"),
            r#"{
              "id":"test-13","display_name":"안드로이드 13","android_version":"13",
              "api_level":33,"distribution":"bliss","artifact":"test-artifact",
              "translator":"ndk_translation","boot_args":["quiet"],"grub_entry_hint":"Virgl",
              "install_guide":["1","2","3","4","5","6"],
              "qemu_overrides":["virtio_vga_gl"],"status":"verified",
              "released_at":"2024-10-11","verifications":[]
            }"#,
        )
        .expect("profile");
        let manifest = directory.path().join("artifacts.json");
        fs::write(
            &manifest,
            r#"{
              "schema_version":1,"allowed_hosts":["example.com"],"artifacts":[{
                "name":"test-artifact","version":"1","filename":"test.iso",
                "url":"https://example.com/test.iso","size_bytes":1234,
                "sha256":"0000000000000000000000000000000000000000000000000000000000000000",
                "license":"test","provenance_note":"test","fetched_by":"installer"
              }]
            }"#,
        )
        .expect("manifest");
        let home = OmeHome::from_path(directory.path().join("home")).expect("home");
        let runtime = AppRuntime::open(
            home,
            RuntimeDeps {
                probe: Box::new(ready_probe(FeatureState::Enabled)),
                artifacts: None,
                adb: None,
                supervisor: None,
                desktop: Box::new(crate::UnavailableDesktop),
                window_host: Box::new(GuestWindowHost::default()),
                family_adapter: None,
                images_dir: Some(images),
                artifacts_manifest: Some(manifest),
                product_version: "0.1.0".to_owned(),
            },
        )
        .expect("runtime");
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.images.profiles[0].size_bytes, Some(1234));
        assert!(snapshot.images.profiles[0].recommended);
        assert_eq!(snapshot.wizard.image_id.as_deref(), Some("test-13"));
        assert_eq!(snapshot.wizard.install_guide.len(), 6);
    }

    #[test]
    fn open_runs_host_check_before_the_first_snapshot() {
        let (_directory, runtime) = runtime(FeatureState::Enabled);
        let snapshot = runtime.snapshot();
        assert!(snapshot.host.ready);
        assert_eq!(snapshot.host.rows.len(), 8);
        assert!(snapshot.host.inspected_at.is_some());
    }

    #[test]
    fn host_refresh_uses_limits_and_enabled_feature_skips_consent() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let snapshot = runtime
            .apply(Command::HostCheckRefresh)
            .expect("refresh succeeds");
        assert!(snapshot.host.ready);
        assert_eq!(snapshot.settings.memory_mib_max, 61_440);
        assert_eq!(snapshot.settings.vcpus_max, 16);
        let snapshot = runtime
            .apply(Command::WizardContinue)
            .expect("continue succeeds");
        assert_eq!(snapshot.wizard.step, WizardStep::ArtifactDownload);
    }

    #[test]
    fn blocked_host_derives_blocker_in_rust() {
        let directory = tempfile::tempdir().expect("temp directory");
        let probe = ready_probe(FeatureState::Enabled)
            .with(ProbeId::CpuVirtualization, ProbeValue::Bool(false));
        let home = OmeHome::from_path(directory.path().join("home")).expect("home");
        let mut runtime = AppRuntime::open(
            home,
            RuntimeDeps {
                probe: Box::new(probe),
                artifacts: None,
                adb: None,
                supervisor: None,
                desktop: Box::new(crate::UnavailableDesktop),
                window_host: Box::new(GuestWindowHost::default()),
                family_adapter: None,
                images_dir: None,
                artifacts_manifest: None,
                product_version: "0.1.0".to_owned(),
            },
        )
        .expect("runtime");
        let snapshot = runtime.apply(Command::HostCheckRefresh).expect("refresh");
        assert_eq!(snapshot.blocker, None);
        assert!(!snapshot.host.ready);
        let snapshot = runtime.apply(Command::WizardDefer).expect("leave wizard");
        assert_eq!(
            snapshot.blocker,
            Some(Blocker {
                kind: BlockerKind::VirtualizationOff
            })
        );
    }

    #[test]
    fn settings_save_persists_host_valid_input() {
        let (directory, mut runtime) = runtime(FeatureState::Disabled);
        runtime
            .apply(Command::SettingsSave {
                settings: SettingsInput {
                    memory_mib: 32_768,
                    vcpus: 12,
                    gpu_mode: GpuMode::Software,
                    close_action: CloseAction::StopGuest,
                    show_fps: true,
                    auto_update_check: false,
                    adb_access: AdbAccess::Network,
                    binding_overlay_default: false,
                },
            })
            .expect("save settings");
        let json =
            fs::read_to_string(directory.path().join("home/settings.json")).expect("settings file");
        assert!(json.contains("32768"));
        assert!(json.contains("network"));
    }

    #[test]
    fn input_state_commands_validate_and_update_snapshot() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let bundled = runtime.snapshot().input.profiles[0].clone();
        let issue = runtime
            .apply(Command::InputProfileDelete {
                id: bundled.id.clone(),
            })
            .expect_err("bundled profile is protected");
        assert_eq!(issue.code, "input_profile_bundled");

        let mut custom = bundled;
        custom.id = "custom".to_owned();
        custom.bundled = false;
        runtime
            .apply(Command::InputProfileSave {
                profile: custom.clone(),
            })
            .expect("save custom profile");
        let snapshot = runtime
            .apply(Command::InputProfileSelect {
                id: Some(custom.id.clone()),
            })
            .expect("select custom profile");
        assert_eq!(snapshot.input.active_id.as_deref(), Some("custom"));
        assert!(!snapshot.input.suspended);
        assert!(
            runtime
                .apply(Command::InputSuspendToggle)
                .expect("toggle")
                .input
                .suspended
        );
    }

    #[test]
    fn display_state_commands_validate_and_update_snapshot() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let snapshot = runtime
            .apply(Command::DisplayCustomApply {
                size: Size {
                    width: 1600,
                    height: 904,
                },
                density_dpi: 240,
            })
            .expect("custom display");
        assert_eq!(snapshot.display.active_id, None);
        assert_eq!(snapshot.display.custom.expect("custom").size.width, 1600);
        let snapshot = runtime
            .apply(Command::DisplayRefreshSet { hz: Some(100) })
            .expect("custom refresh");
        assert!(snapshot.display.refresh_rates.contains(&100));
        assert!(!snapshot.display.refresh_supported);
    }

    #[test]
    fn contract_three_commands_update_runtime_owned_state() {
        let (_directory, mut runtime) = runtime(FeatureState::Disabled);
        let wizard_step = runtime.snapshot().wizard.step;
        let snapshot = runtime.apply(Command::WizardDefer).expect("defer wizard");
        assert_eq!(snapshot.phase, AppPhase::Main);
        assert_eq!(snapshot.wizard.step, wizard_step);
        assert_eq!(
            snapshot.blocker,
            Some(Blocker {
                kind: BlockerKind::HypervisorPlatformOff,
            })
        );
        assert!(snapshot.input.overlay_visible);
        assert!(
            !runtime
                .apply(Command::InputOverlayToggle)
                .expect("toggle overlay")
                .input
                .overlay_visible
        );
    }

    #[test]
    fn new_input_profile_becomes_active_but_existing_save_keeps_selection() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let mut custom = runtime.snapshot().input.profiles[0].clone();
        let original = custom.id.clone();
        custom.id = "new-profile".to_owned();
        custom.bundled = false;
        let snapshot = runtime
            .apply(Command::InputProfileSave {
                profile: custom.clone(),
            })
            .expect("save new profile");
        assert_eq!(snapshot.input.active_id.as_deref(), Some("new-profile"));
        runtime
            .apply(Command::InputProfileSelect {
                id: Some(original.clone()),
            })
            .expect("select original");
        custom.name = "Renamed".to_owned();
        let snapshot = runtime
            .apply(Command::InputProfileSave { profile: custom })
            .expect("save existing profile");
        assert_eq!(snapshot.input.active_id.as_deref(), Some(original.as_str()));
    }

    #[test]
    fn display_reboot_requirement_depends_on_current_orientation() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let initial = runtime.snapshot();
        assert!(!initial.display.presets[0].needs_reboot);
        assert!(!initial.display.presets[1].needs_reboot);
        assert!(initial.display.presets[2].needs_reboot);
        let portrait = runtime
            .apply(Command::DisplayCustomApply {
                size: Size {
                    width: 720,
                    height: 1280,
                },
                density_dpi: 160,
            })
            .expect("portrait display");
        assert!(portrait.display.presets[0].needs_reboot);
        assert!(portrait.display.presets[1].needs_reboot);
        assert!(!portrait.display.presets[2].needs_reboot);
    }

    #[test]
    fn volume_validation_precedes_unwired_adapter() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let invalid = runtime
            .apply(Command::GuestVolumeSet { index: 16 })
            .expect_err("invalid volume");
        assert_eq!(invalid.code, "invalid_media_volume");
        let stopped = runtime
            .apply(Command::GuestVolumeSet { index: 15 })
            .expect_err("operating system is stopped");
        assert_eq!(stopped.code, "operating_system_not_running");
    }

    #[test]
    fn process_command_requires_an_installed_operating_system() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let issue = runtime
            .apply(Command::GuestStart)
            .expect_err("no operating system is installed");
        assert_eq!(issue.code, "operating_system_not_found");
        assert_eq!(runtime.snapshot().issue, Some(issue));
    }

    #[test]
    fn lifecycle_boot_probe_persists_values_and_google_account_command() {
        let desktop = RecordingDesktop::default();
        let window = RecordingWindow::embedded();
        let outputs = [
            output("connected"),
            output("1"),
            output("booted"),
            output("package"),
            output("1280x720"),
            output("160"),
            output("8"),
            output("1234567890"),
            output("captured"),
            output("removed"),
            output("foreground"),
            output("yes"),
            output("bridge.so"),
            output("no"),
            output("1"),
            output("opened"),
        ];
        let (directory, mut runtime, supervisor, runner) =
            lifecycle_runtime(outputs, desktop, window);
        runtime.apply(Command::GuestStart).expect("start request");
        assert_eq!(supervisor.starts.lock().expect("starts lock").len(), 1);
        assert!(
            directory
                .path()
                .join("home/vm/default/guest.json")
                .is_file(),
            "adopted metadata is saved after start admission"
        );
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Starting,
            None,
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(4242),
            None,
        ));
        runtime.tick();
        runtime.tick();
        let snapshot = runtime.snapshot();
        assert!(snapshot.guest.boot_completed);
        assert_eq!(snapshot.guest.pid, Some(4242));
        assert_eq!(snapshot.guest.device_id.as_deref(), Some("499602d2"));
        assert_eq!(
            snapshot.guest.device_id_decimal.as_deref(),
            Some("1234567890")
        );
        assert_eq!(snapshot.guest.google_accounts, Some(1));
        assert_eq!(snapshot.guest.media_volume, Some(8));
        assert_eq!(snapshot.guest.root_enabled, Some(false));
        assert!(snapshot.guest.add_account_supported);
        assert_eq!(
            snapshot.guest.resolution,
            Some(Size {
                width: 1280,
                height: 720
            })
        );
        runtime
            .apply(Command::GoogleAccountAddOpen)
            .expect("open account setup");
        let calls = runner.calls();
        let last = calls.last().expect("account call");
        assert_eq!(
            last.args,
            ["-s", "127.0.0.1:5555", "shell", "open-add-account"].map(OsString::from)
        );
        let metadata: serde_json::Value = serde_json::from_slice(
            &fs::read(directory.path().join("home/vm/default/guest.json")).expect("guest metadata"),
        )
        .expect("guest metadata JSON");
        assert_eq!(metadata["deviceId"]["hex"], "499602d2");
        assert_eq!(metadata["capabilities"]["googleAccounts"], 1);
        assert_eq!(metadata["capabilities"]["mediaVolume"], 8);
    }

    #[test]
    fn stop_failure_restart_and_timeout_classify_from_state_not_log_presence() {
        let (directory, mut runtime, supervisor, _runner) = lifecycle_runtime(
            std::iter::empty(),
            RecordingDesktop::default(),
            RecordingWindow::embedded(),
        );
        runtime.ingest_guest_event(guest_event(ome_supervisor::GuestState::Stopped, None, None));
        assert!(runtime.snapshot().guest.last_exit.is_none());
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Starting,
            None,
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(7),
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Stopping,
            Some(7),
            None,
        ));
        runtime.ingest_guest_event(guest_event(ome_supervisor::GuestState::Stopped, None, None));
        assert_eq!(
            runtime.snapshot().guest.last_exit.expect("user stop").kind,
            ExitKind::UserStop
        );

        let log = directory.path().join("crash.stderr.log");
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Starting,
            None,
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(8),
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Failed,
            None,
            Some(&log),
        ));
        let crash = runtime.snapshot().guest.last_exit.expect("crash");
        assert_eq!(crash.kind, ExitKind::Crash);
        assert_eq!(
            crash.log_path.as_deref(),
            Some(log.to_string_lossy().as_ref())
        );

        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Starting,
            None,
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(9),
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Restarting,
            Some(9),
            None,
        ));
        assert_eq!(
            runtime
                .snapshot()
                .guest
                .last_exit
                .expect("guest reset")
                .kind,
            ExitKind::GuestReset
        );
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Starting,
            None,
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(10),
            None,
        ));
        assert_eq!(runtime.snapshot().guest.pid, Some(10));

        runtime.boot_timeout_override = Some(Duration::ZERO);
        runtime.tick();
        assert_eq!(*supervisor.stop_requests.lock().expect("stop lock"), 1);
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Stopping,
            Some(10),
            None,
        ));
        runtime.ingest_guest_event(guest_event(ome_supervisor::GuestState::Stopped, None, None));
        assert_eq!(
            runtime
                .snapshot()
                .guest
                .last_exit
                .expect("boot timeout")
                .kind,
            ExitKind::BootTimeout
        );
    }

    #[test]
    fn explicit_restart_starts_again_after_stopped() {
        let (_directory, mut runtime, supervisor, _runner) = lifecycle_runtime(
            std::iter::empty(),
            RecordingDesktop::default(),
            RecordingWindow::embedded(),
        );
        runtime.apply(Command::GuestStart).expect("initial start");
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Starting,
            None,
            None,
        ));
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(11),
            None,
        ));
        *supervisor.state.lock().expect("state lock") = ome_supervisor::GuestState::Running;
        runtime
            .apply(Command::GuestRestart)
            .expect("restart request");
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Stopping,
            Some(11),
            None,
        ));
        *supervisor.state.lock().expect("state lock") = ome_supervisor::GuestState::Stopped;
        runtime.ingest_guest_event(guest_event(ome_supervisor::GuestState::Stopped, None, None));
        assert_eq!(supervisor.starts.lock().expect("starts lock").len(), 2);
    }

    #[test]
    fn stage_rect_waits_for_running_then_embeds_or_falls_back() {
        let embedded = RecordingWindow::embedded();
        let targets = Arc::clone(&embedded.targets);
        let placements = Arc::clone(&embedded.placements);
        let (_directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), RecordingDesktop::default(), embedded);
        runtime.set_host_window(77);
        let rect = StageRect {
            x: 1.0,
            y: 2.0,
            width: 300.0,
            height: 200.0,
            scale_factor: 1.5,
        };
        runtime
            .apply(Command::StageRectChanged { rect })
            .expect("store pre-running rect");
        assert!(targets.lock().expect("targets lock").is_empty());
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(22),
            None,
        ));
        assert_eq!(runtime.snapshot().guest.hosting, HostingMode::Embedded);
        assert_eq!(targets.lock().expect("targets lock").len(), 1);
        assert_eq!(placements.lock().expect("placements lock").len(), 1);
        assert_eq!(
            runtime.guest_client_screen_rect(),
            Some(Rect {
                x: 2,
                y: 3,
                width: 450,
                height: 300,
            })
        );
        runtime
            .apply(Command::StageRectChanged {
                rect: StageRect {
                    width: 400.0,
                    ..rect
                },
            })
            .expect("replace rect");
        assert_eq!(targets.lock().expect("targets lock").len(), 1);
        assert_eq!(placements.lock().expect("placements lock").len(), 2);

        let separate = RecordingWindow::separate();
        let separate_targets = Arc::clone(&separate.targets);
        let (_directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), RecordingDesktop::default(), separate);
        runtime.set_host_window(77);
        runtime
            .apply(Command::StageRectChanged { rect })
            .expect("store rect");
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(23),
            None,
        ));
        assert_eq!(
            runtime.snapshot().guest.hosting,
            HostingMode::SeparateWindow
        );
        assert_eq!(runtime.guest_client_screen_rect(), None);
        runtime
            .apply(Command::StageRectChanged { rect })
            .expect("keep separate placement");
        assert_eq!(separate_targets.lock().expect("targets lock").len(), 1);
    }

    #[test]
    fn stage_hidden_hides_embedded_window_and_next_rect_shows_it() {
        let embedded = RecordingWindow::embedded();
        let hides = Arc::clone(&embedded.hide_count);
        let shows = Arc::clone(&embedded.show_count);
        let placements = Arc::clone(&embedded.placements);
        let (_directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), RecordingDesktop::default(), embedded);
        runtime.set_host_window(77);
        let rect = StageRect {
            x: 1.0,
            y: 2.0,
            width: 300.0,
            height: 200.0,
            scale_factor: 1.5,
        };
        runtime
            .apply(Command::StageRectChanged { rect })
            .expect("store rect");
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(22),
            None,
        ));
        assert!(runtime.guest_client_screen_rect().is_some());

        runtime.apply(Command::StageHidden).expect("hide stage");
        assert_eq!(*hides.lock().expect("hide count"), 1);
        assert_eq!(runtime.guest_client_screen_rect(), None);

        runtime
            .apply(Command::StageRectChanged {
                rect: StageRect {
                    width: 400.0,
                    ..rect
                },
            })
            .expect("show stage");
        assert_eq!(*shows.lock().expect("show count"), 1);
        assert_eq!(placements.lock().expect("placements").len(), 2);
        assert!(runtime.guest_client_screen_rect().is_some());
    }

    #[test]
    fn stage_hidden_does_not_touch_a_separate_window() {
        let separate = RecordingWindow::separate();
        let hides = Arc::clone(&separate.hide_count);
        let shows = Arc::clone(&separate.show_count);
        let (_directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), RecordingDesktop::default(), separate);
        runtime.set_host_window(77);
        let rect = StageRect {
            x: 1.0,
            y: 2.0,
            width: 300.0,
            height: 200.0,
            scale_factor: 1.5,
        };
        runtime
            .apply(Command::StageRectChanged { rect })
            .expect("store rect");
        runtime.ingest_guest_event(guest_event(
            ome_supervisor::GuestState::Running,
            Some(23),
            None,
        ));
        assert_eq!(
            runtime.snapshot().guest.hosting,
            HostingMode::SeparateWindow
        );
        runtime.apply(Command::StageHidden).expect("accept hidden");
        runtime
            .apply(Command::StageRectChanged { rect })
            .expect("accept visible");
        assert_eq!(*hides.lock().expect("hide count"), 0);
        assert_eq!(*shows.lock().expect("show count"), 0);
    }

    #[test]
    fn desktop_folder_commands_create_and_open_fixed_home_directories() {
        let desktop = RecordingDesktop::default();
        let observed = desktop.clone();
        let (directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), desktop, RecordingWindow::embedded());
        for command in [
            Command::OpenHomeFolder,
            Command::OpenLogsFolder,
            Command::OpenScreenshotsFolder,
        ] {
            runtime.apply(command).expect("open fixed directory");
        }
        assert_eq!(
            observed.paths(),
            [
                directory.path().join("home"),
                directory.path().join("home/logs"),
                directory.path().join("home/screenshots"),
            ]
        );
    }

    #[test]
    fn registration_copies_hex_opens_url_and_persists_time() {
        let desktop = RecordingDesktop::default();
        let observed = desktop.clone();
        let (directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), desktop, RecordingWindow::embedded());
        runtime.device_id = Some("499602d2".to_owned());
        runtime
            .apply(Command::OpenRegistrationPage)
            .expect("registration page");
        assert_eq!(observed.texts(), ["499602d2"]);
        assert_eq!(
            observed.urls(),
            ["https://www.google.com/android/uncertified/"]
        );
        assert!(runtime.snapshot().guest.registration_opened_at.is_some());
        let metadata: serde_json::Value = serde_json::from_slice(
            &fs::read(directory.path().join("home/vm/default/guest.json")).expect("metadata"),
        )
        .expect("metadata JSON");
        assert!(metadata["registrationOpenedAt"].as_str().is_some());
    }

    #[test]
    fn drag_drop_validation_rejects_missing_and_unsupported_paths() {
        let directory = tempfile::tempdir().expect("temp directory");
        let apk = directory.path().join("sample.APK");
        fs::write(&apk, b"apk").expect("APK fixture");
        assert_eq!(
            validate_app_paths(vec![apk.clone()]).expect("valid APK"),
            [apk]
        );
        for invalid in [
            directory.path().join("missing.apk"),
            directory.path().join("sample.zip"),
        ] {
            assert_eq!(
                validate_app_paths(vec![invalid])
                    .expect_err("invalid path")
                    .code,
                "app_package_invalid"
            );
        }
    }

    #[test]
    fn app_install_reports_progress_and_refreshes_packages() {
        let package_dir = tempfile::tempdir().expect("package directory");
        let first = package_dir.path().join("first.apk");
        let second = package_dir.path().join("second.apk");
        fs::write(&first, b"apk").expect("first APK");
        fs::write(&second, b"apk").expect("second APK");
        let outputs = [
            output("Success"),
            output("Success"),
            output(
                "package:dev.ome.one versionCode:7
package:dev.ome.two versionCode:8",
            ),
        ];
        let (_home, mut runtime, _supervisor, runner) = lifecycle_runtime(
            outputs,
            RecordingDesktop::default(),
            RecordingWindow::embedded(),
        );
        runtime.guest_state = GuestState::Running;
        runtime.boot_completed = true;
        runtime
            .install_apps(vec![first, second])
            .expect("install admitted");
        for _ in 0..1000 {
            runtime.poll_workers();
            if runtime.install_cancel.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.apps.items.len(), 2);
        assert_eq!(snapshot.apps.items[0].version_code, Some(7));
        assert_eq!(
            snapshot.apps.install.expect("progress").stage,
            TransferStage::Verified
        );
        assert_eq!(
            runner
                .calls()
                .iter()
                .filter(|call| call.args.iter().any(|arg| arg == "install"))
                .count(),
            2
        );
    }

    #[test]
    fn app_install_cancel_stops_after_current_file() {
        let package_dir = tempfile::tempdir().expect("package directory");
        let first = package_dir.path().join("first.apk");
        let second = package_dir.path().join("second.apk");
        fs::write(&first, b"apk").expect("first APK");
        fs::write(&second, b"apk").expect("second APK");
        let (_home, mut runtime, _supervisor, runner) = lifecycle_runtime(
            [output("Success")],
            RecordingDesktop::default(),
            RecordingWindow::embedded(),
        );
        runtime.guest_state = GuestState::Running;
        runtime.boot_completed = true;
        runtime
            .install_apps(vec![first, second])
            .expect("install admitted");
        runtime.cancel_app_install();
        for _ in 0..1000 {
            runtime.poll_workers();
            if runtime.install_cancel.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(runtime.install_cancel.is_none(), "install worker completed");
        assert_eq!(
            runtime
                .snapshot()
                .apps
                .install
                .expect("cancelled progress")
                .stage,
            TransferStage::Cancelled
        );
        assert!(runner.calls().len() <= 1);
    }

    #[test]
    fn whpx_exit_codes_map_to_wizard_and_issues() {
        for (result, expected_step, expected_code) in [
            (Ok(0), WizardStep::ArtifactDownload, None),
            (Ok(3010), WizardStep::RebootPending, None),
            (
                Ok(1223),
                WizardStep::WhpxConsent,
                Some("hypervisor_enable_declined"),
            ),
            (
                Err(ElevationError::Declined),
                WizardStep::WhpxConsent,
                Some("hypervisor_enable_declined"),
            ),
        ] {
            let (_directory, mut runtime) = runtime(FeatureState::Disabled);
            runtime.wizard.step = Step::WhpxConsent;
            runtime.set_worker_deps(WorkerDeps {
                elevation: Arc::new(FakeElevation(result)),
                ..WorkerDeps::default()
            });
            let outcome = runtime.apply(Command::WhpxEnable);
            assert_eq!(runtime.snapshot().wizard.step, expected_step);
            assert_eq!(
                outcome.err().map(|issue| issue.code).as_deref(),
                expected_code
            );
        }
    }

    #[test]
    fn artifact_download_reports_verify_resume_and_cancel() {
        let body = b"artifact bytes".to_vec();
        let hash = format!("{:x}", Sha256::digest(&body));
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        runtime.image_profiles.push(GuestImageProfile {
            id: "test-image".to_owned(),
            display_name: "테스트 운영체제".to_owned(),
            android_version: "13".to_owned(),
            api_level: 33,
            distribution: Distribution::Bliss,
            artifact: "test-artifact".to_owned(),
            translator: Translator::NdkTranslation,
            boot_args: vec![],
            grub_entry_hint: "installer".to_owned(),
            install_guide: vec![],
            qemu_overrides: vec![],
            status: ProfileStatus::Verified,
            released_at: Some("2024-10-11".to_owned()),
            verifications: vec![],
        });
        runtime.selected_image = Some("test-image".to_owned());
        let artifact_dir = runtime
            .home
            .subdir("artifacts")
            .expect("artifact directory");
        fs::write(artifact_dir.join("test.iso.part"), &body[..4]).expect("partial");
        let manifest = Manifest::parse(&format!(
            r#"{{"schema_version":1,"allowed_hosts":["example.com"],"artifacts":[{{
            "name":"test-artifact","version":"1","filename":"test.iso",
            "url":"https://example.com/test.iso","size_bytes":{},"sha256":"{}",
            "license":"test","provenance_note":"test","fetched_by":"installer"}}]}}"#,
            body.len(),
            hash
        ))
        .expect("manifest");
        runtime.deps.artifacts = Some(ArtifactStore::new(
            manifest,
            &artifact_dir,
            Box::new(FixedHttp {
                body: body[4..].to_vec(),
                final_url: "https://example.com/test.iso".to_owned(),
                status: 200,
            }),
        ));
        runtime
            .apply(Command::ArtifactDownloadStart)
            .expect("download start");
        for _ in 0..1000 {
            runtime.poll_workers();
            if runtime.artifact_cancel.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let progress = runtime
            .snapshot()
            .wizard
            .download
            .expect("download progress");
        assert_eq!(progress.stage, TransferStage::Verified);
        assert_eq!(
            progress.done_bytes,
            u64::try_from(body.len()).expect("test size")
        );
        assert!(artifact_dir.join("test.iso").is_file());

        fs::remove_file(artifact_dir.join("test.iso")).expect("remove complete");
        runtime.deps.artifacts = Some(ArtifactStore::new(
            runtime.artifact_manifest.clone().unwrap_or_else(|| {
                Manifest::parse(&format!(
                    r#"{{"schema_version":1,"allowed_hosts":["example.com"],"artifacts":[{{
                "name":"test-artifact","version":"1","filename":"test.iso",
                "url":"https://example.com/test.iso","size_bytes":{},"sha256":"{}",
                "license":"test","provenance_note":"test","fetched_by":"installer"}}]}}"#,
                    body.len(),
                    hash
                ))
                .expect("manifest")
            }),
            &artifact_dir,
            Box::new(FixedHttp {
                body: body.clone(),
                final_url: "https://example.com/test.iso".to_owned(),
                status: 200,
            }),
        ));
        runtime
            .apply(Command::ArtifactDownloadStart)
            .expect("cancel start");
        runtime
            .apply(Command::ArtifactDownloadCancel)
            .expect("cancel request");
        for _ in 0..1000 {
            runtime.poll_workers();
            if runtime.artifact_cancel.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            runtime
                .snapshot()
                .wizard
                .download
                .expect("cancel progress")
                .stage,
            TransferStage::Cancelled
        );
        assert!(artifact_dir.join("test.iso.part").is_file());
    }

    #[test]
    fn guest_create_and_reinstall_write_layout_and_start_installer_iso() {
        let body = b"verified installer image".to_vec();
        let hash = format!("{:x}", Sha256::digest(&body));
        let (directory, mut runtime, supervisor, _runner) = lifecycle_runtime(
            std::iter::empty(),
            RecordingDesktop::default(),
            RecordingWindow::embedded(),
        );
        let qemu_dir = directory.path().join("qemu");
        fs::create_dir(&qemu_dir).expect("qemu directory");
        let system = qemu_dir.join("qemu-system-x86_64.exe");
        let image = qemu_dir.join("qemu-img.exe");
        let code = qemu_dir.join("code.fd");
        let vars = qemu_dir.join("vars.fd");
        for path in [&system, &image, &code, &vars] {
            fs::write(path, b"fixture").expect("qemu fixture");
        }
        runtime.deps.probe = Box::new(ready_probe(FeatureState::Enabled).with(
            ProbeId::QemuPresent,
            ProbeValue::Qemu(Some(QemuFound {
                version: "test".to_owned(),
                source: "test".to_owned(),
                program: system.to_string_lossy().into_owned(),
                firmware_code: Some(code.to_string_lossy().into_owned()),
                firmware_vars_template: Some(vars.to_string_lossy().into_owned()),
            })),
        ));
        runtime.image_profiles[0].artifact = "test-artifact".to_owned();
        let manifest = Manifest::parse(&format!(
            r#"{{"schema_version":1,"allowed_hosts":["example.com"],"artifacts":[{{
            "name":"test-artifact","version":"1","filename":"test.iso",
            "url":"https://example.com/test.iso","size_bytes":{},"sha256":"{}",
            "license":"test","provenance_note":"test","fetched_by":"installer"}}]}}"#,
            body.len(),
            hash
        ))
        .expect("manifest");
        let artifact_dir = directory.path().join("home/artifacts");
        fs::write(artifact_dir.join("test.iso"), body).expect("verified artifact");
        runtime.deps.artifacts = Some(ArtifactStore::new(
            manifest,
            artifact_dir,
            Box::new(ome_artifacts::UreqFetch::new()),
        ));
        let runner = CreatingDiskRunner::default();
        runtime.set_worker_deps(WorkerDeps {
            process: Arc::new(runner.clone()),
            ..WorkerDeps::default()
        });
        runtime.guests.clear();
        runtime.active_guest = None;
        runtime.wizard.step = Step::GuestInstall;
        runtime
            .apply(Command::GuestCreate {
                image_id: "bliss-16.9.7-android-13".to_owned(),
                size_gib: 32,
            })
            .expect("guest create");
        let id = "bliss-16-9-7-android-13";
        let guest_dir = directory.path().join("home/vm").join(id);
        assert!(guest_dir.join("disk.qcow2").is_file());
        assert_eq!(
            fs::read(guest_dir.join("efivars.fd")).expect("vars copy"),
            b"fixture"
        );
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(guest_dir.join("guest.json")).expect("guest JSON"))
                .expect("guest document");
        assert_eq!(document["id"], id);
        assert_eq!(document["diskBytes"], 32_u64 << 30);
        {
            let starts = supervisor.starts.lock().expect("starts lock");
            assert_eq!(starts.len(), 1);
            assert_eq!(
                starts[0].1.iso.as_deref(),
                Some(directory.path().join("home/artifacts/test.iso").as_path())
            );
        }
        assert_eq!(runner.calls.lock().expect("runner calls")[0].0, image);

        runtime.guest_state = GuestState::Stopped;
        *supervisor.state.lock().expect("state lock") = ome_supervisor::GuestState::Stopped;
        runtime
            .apply(Command::GuestReinstall {
                name: id.to_owned(),
            })
            .expect("reinstall");
        assert_eq!(runtime.active_guest.as_deref(), Some(id));
        assert!(guest_dir.join("guest.json").is_file());
        assert_eq!(supervisor.starts.lock().expect("starts lock").len(), 2);
    }

    #[test]
    fn diagnostics_bundle_contains_expected_entries_and_opens_folder() {
        let desktop = RecordingDesktop::default();
        let observed = desktop.clone();
        let (directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), desktop, RecordingWindow::embedded());
        let logs = directory.path().join("home/logs");
        fs::write(logs.join("host.log"), b"host").expect("host log");
        fs::write(logs.join("default.stdout.log"), b"qemu").expect("qemu log");
        runtime
            .apply(Command::DiagnosticsExport)
            .expect("diagnostics export");
        let diagnostics = directory.path().join("home/diagnostics");
        let archive = fs::read_dir(&diagnostics)
            .expect("diagnostics directory")
            .next()
            .expect("bundle entry")
            .expect("bundle path")
            .path();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(archive).expect("bundle")).expect("ZIP");
        assert!(zip.by_name("logs/host.log").is_ok());
        assert!(zip.by_name("qemu/default.stdout.log").is_ok());
        assert!(zip.by_name("environment.txt").is_ok());
        assert_eq!(observed.paths(), [diagnostics]);
    }

    #[test]
    fn update_check_maps_http_404_to_up_to_date() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        runtime.set_worker_deps(WorkerDeps {
            http: Arc::new(FixedHttp {
                body: Vec::new(),
                final_url:
                    "https://api.github.com/repos/115dkk/Open-Mobile-Emulator/releases/latest"
                        .to_owned(),
                status: 404,
            }),
            ..WorkerDeps::default()
        });
        runtime
            .apply(Command::UpdateCheck)
            .expect("start update check");
        for _ in 0..1000 {
            runtime.poll_workers();
            if !matches!(runtime.snapshot().update.state, UpdateState::Checking) {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(matches!(
            runtime.snapshot().update.state,
            UpdateState::UpToDate { .. }
        ));
    }

    #[test]
    fn release_without_usable_installer_is_up_to_date() {
        let document = ReleaseDocument {
            tag_name: "v0.2.0".to_owned(),
            html_url: "https://github.com/115dkk/Open-Mobile-Emulator/releases/tag/v0.2.0"
                .to_owned(),
            body: "notes".to_owned(),
            assets: vec![ReleaseAsset {
                name: "source.tar.gz".to_owned(),
                browser_download_url:
                    "https://github.com/115dkk/Open-Mobile-Emulator/releases/download/v0.2.0/source.tar.gz"
                        .to_owned(),
                size: 12,
            }],
        };
        assert_eq!(
            release_from_document(document, "0.1.0")
                .expect_err("no usable installer")
                .code,
            "update_up_to_date"
        );
    }

    #[test]
    fn update_check_parses_release_and_compares_semver() {
        let document = ReleaseDocument {
            tag_name: "v0.2.0".to_owned(),
            html_url: "https://github.com/115dkk/Open-Mobile-Emulator/releases/tag/v0.2.0".to_owned(),
            body: "notes".to_owned(),
            assets: vec![
                ReleaseAsset { name: "ome.exe".to_owned(), browser_download_url: "https://github.com/115dkk/Open-Mobile-Emulator/releases/download/v0.2.0/ome.exe".to_owned(), size: 12 },
                ReleaseAsset { name: "ome.exe.sha256".to_owned(), browser_download_url: "https://github.com/115dkk/Open-Mobile-Emulator/releases/download/v0.2.0/ome.exe.sha256".to_owned(), size: 64 },
            ],
        };
        let release = release_from_document(document.clone(), "0.1.9").expect("new release");
        assert_eq!(release.version, "0.2.0");
        assert!(release.checksum.is_some());
        assert_eq!(
            release_from_document(document, "0.2.0")
                .expect_err("up to date")
                .code,
            "update_up_to_date"
        );
    }

    #[test]
    fn update_install_refuses_release_without_checksum() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        runtime.update_release = Some(UpdateRelease {
            version: "0.2.0".to_owned(), notes_url: None,
            installer: ReleaseAsset {
                name: "ome.exe".to_owned(),
                browser_download_url: "https://github.com/115dkk/Open-Mobile-Emulator/releases/download/v0.2.0/ome.exe".to_owned(),
                size: 12,
            },
            checksum: None,
        });
        assert_eq!(
            runtime
                .apply(Command::UpdateInstall)
                .expect_err("unverified update")
                .code,
            "update_unverified"
        );
    }

    #[test]
    fn release_notes_accept_only_the_repository_release_urls() {
        for accepted in [
            "https://github.com/115dkk/Open-Mobile-Emulator/releases",
            "https://github.com/115dkk/Open-Mobile-Emulator/releases/tag/v1.0.0",
        ] {
            assert!(trusted_release_notes_url(accepted), "{accepted}");
        }
        for rejected in [
            "http://github.com/115dkk/Open-Mobile-Emulator/releases",
            "https://github.com/other/project/releases",
            "https://github.com/115dkk/Open-Mobile-Emulator.evil/releases",
            "https://example.com/release",
        ] {
            assert!(!trusted_release_notes_url(rejected), "{rejected}");
        }
    }

    #[test]
    fn fixed_help_urls_are_declared_in_network_document() {
        let desktop = RecordingDesktop::default();
        let observed = desktop.clone();
        let (_directory, mut runtime, _supervisor, _runner) =
            lifecycle_runtime(std::iter::empty(), desktop, RecordingWindow::embedded());
        for topic in [
            HelpTopic::VirtualizationBios,
            HelpTopic::HypervisorPlatform,
            HelpTopic::GoogleAccount,
            HelpTopic::AdbSecurity,
            HelpTopic::QemuSource,
            HelpTopic::ThirdPartyNotices,
            HelpTopic::ReleaseNotes,
        ] {
            runtime
                .apply(Command::OpenHelp { topic })
                .expect("open help URL");
        }
        runtime.device_id = Some("499602d2".to_owned());
        runtime
            .apply(Command::OpenRegistrationPage)
            .expect("registration URL");
        let network = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/NETWORK.md"),
        )
        .expect("NETWORK.md");
        assert!(network.contains("api.github.com"));
        assert!(network.contains("objects.githubusercontent.com"));
        for url in observed.urls() {
            let authority = url
                .strip_prefix("https://")
                .and_then(|rest| rest.split('/').next())
                .expect("fixed HTTPS URL");
            assert!(
                network.contains(authority),
                "NETWORK.md is missing {authority}"
            );
        }
    }
}
