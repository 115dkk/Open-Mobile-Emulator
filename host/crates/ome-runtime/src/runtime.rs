// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use jiff::{Unit, Zoned};
use ome_adb::AdbSession;
use ome_artifacts::{ArtifactStore, Manifest};
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

use crate::adapters::AdbShellRunner;
use crate::desktop::Desktop;
use crate::guest_store::{
    GuestRecord, GuestStore, GuestStoreError, StoredProbeItem, StoredProbeState,
};
use crate::home::OmeHome;
use crate::issues;
use crate::settings::{Settings, SettingsError, SettingsStore};
use crate::{
    AppIssue, AppPhase, AppSnapshot, AppsView, Blocker, BlockerKind, CONTRACT_VERSION, Capability,
    CapabilityId, CapabilityReport, ClipboardItem, Command, CustomDisplay, DisplayPreset,
    DisplayView, ExitKind, GuestImageSummary, GuestState, GuestSummary, GuestView, HelpTopic,
    HostCheckId, HostReport, HostRow, HostStatus, HostingMode, ImageDistribution, ImageStatus,
    ImageTranslator, ImagesView, InputView, LastExit, Notice, NoticeLevel, Orientation,
    SettingsView, Size, StageFit, StageRect, UpdateState, UpdateView, VsyncMode, WizardStep,
    WizardView,
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
    /// Restores top-level window state.
    fn detach(&mut self) -> Result<(), HostingIssue>;
    /// Activates the hosted or separate window.
    fn to_front(&mut self) -> Result<(), HostingIssue>;
    /// Reports live embedded state.
    fn is_attached(&self) -> bool;
}

impl WindowPlacement for GuestWindowHost {
    fn attach(&mut self, target: HostingTarget) -> Result<(), HostingIssue> {
        GuestWindowHost::attach(self, target)
    }
    fn place(&mut self, rect: ome_window_host::Rect) -> Result<(), HostingIssue> {
        GuestWindowHost::place(self, rect)
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

/// Native product runtime and owner of snapshot projection and command decisions.
pub struct AppRuntime {
    home: OmeHome,
    deps: RuntimeDeps,
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
    hosting: HostingMode,
    hosted_pid: Option<u32>,
    host_window: Option<u64>,
    last_stage_rect: Option<StageRect>,
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
        Ok(Self {
            home,
            deps,
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
            hosting: HostingMode::None,
            hosted_pid: None,
            host_window: None,
            last_stage_rect: None,
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
        })
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
                available: self.boot_completed,
                items: Vec::new(),
                install: None,
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
            Command::GuestSelect { id } => self.select_guest(id),
            Command::GuestDelete { id } => self.delete_guest(id),
            Command::GuestStart => self.start_guest(),
            Command::GuestStop => self.stop_guest(),
            Command::GuestRestart => self.restart_guest(),
            Command::GuestVolumeSet { index } => self.set_guest_volume(index),
            Command::StageRectChanged { rect } => self.place_guest_window(rect),
            Command::ScreenshotSave => self.save_screenshot(),
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
            Command::AppQuit
            | Command::AppInstallCancel
            | Command::WhpxEnable
            | Command::ArtifactDownloadStart
            | Command::ArtifactDownloadCancel
            | Command::GuestCreate { .. }
            | Command::GuestReinstall { .. }
            | Command::GuestRootSet { .. }
            | Command::AppInstallPick
            | Command::AppUninstall { .. }
            | Command::AppLaunch { .. }
            | Command::UpdateCheck
            | Command::UpdateInstall
            | Command::DiagnosticsExport => Err(issues::not_wired()),
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
        self.try_place_guest_window();
        Ok(())
    }

    fn try_place_guest_window(&mut self) {
        if self.guest_state != GuestState::Running {
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
                let _ = self.deps.window_host.detach();
                if self.restart_pending {
                    self.restart_pending = false;
                    if let Err(issue) = self.start_guest() {
                        self.issue = Some(issue);
                    }
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
                let _ = self.deps.window_host.detach();
            }
            ome_supervisor::GuestState::Stopping => {}
        }
    }

    /// Advances one non-blocking adb boot/account poll.
    pub fn tick(&mut self) {
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
    }

    fn select_image(&mut self, id: String) -> Result<(), AppIssue> {
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
            inspected_at: None,
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
                .deps
                .artifacts
                .as_ref()
                .and_then(ArtifactStore::installer)
                .is_some_and(|artifact| {
                    self.deps.artifacts.as_ref().is_some_and(|store| {
                        matches!(
                            store.verify(&artifact.name),
                            ome_artifacts::Verification::Verified
                        )
                    })
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
            download: None,
            image_id: self.selected_image.clone(),
            install_guide: self
                .selected_profile()
                .map(|profile| profile.install_guide.clone())
                .unwrap_or_default(),
            disk_size_gib: 32,
            disk_free_bytes: None,
        }
    }

    fn current_blocker(&self) -> Option<Blocker> {
        self.blocker.or_else(|| {
            (self.phase == AppPhase::Main && self.feature_state == FeatureState::Disabled)
                .then_some(Blocker {
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

fn guest_store_issue(_error: GuestStoreError) -> AppIssue {
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
    use std::sync::{Arc, Mutex};

    use ome_adb::{Output, RecordedRunner};
    use ome_guest_image::{
        Attempt, DeviceId, DisplayInfo, GuestFamily, PackageEntry, ShellCommand,
    };
    use ome_host_check::{AdbFound, HostCheckId as ProbeId, ProbeValue, QemuFound, TableProbe};
    use ome_supervisor::LogPaths;

    use super::*;
    use crate::desktop::RecordingDesktop;
    use crate::{AdbAccess, CloseAction, GpuMode, SettingsInput};

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
        detach_count: Arc<Mutex<u32>>,
    }

    impl RecordingWindow {
        fn embedded() -> Self {
            Self {
                attach_result: Ok(()),
                attached: Arc::new(Mutex::new(false)),
                targets: Arc::new(Mutex::new(Vec::new())),
                placements: Arc::new(Mutex::new(Vec::new())),
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
        runtime
            .apply(Command::StageRectChanged { rect })
            .expect("keep separate placement");
        assert_eq!(separate_targets.lock().expect("targets lock").len(), 1);
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
