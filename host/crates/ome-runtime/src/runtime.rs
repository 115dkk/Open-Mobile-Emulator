// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ome_adb::AdbSession;
use ome_artifacts::{ArtifactStore, Manifest};
use ome_guest_image::{
    Distribution, GuestImageProfile, ImageStatus as ProfileStatus, Translator, recommended_index,
    sort_newest_first,
};
use ome_host_check::{
    FeatureState, HardwareLimits, HostProbe, HostReadiness, Verdict, hardware_limits,
};
use ome_input::{InputProfile, is_keyboard_code, load_profiles};
use ome_wizard::{Facts, Outcome, Step, WizardState, advance, can_continue, can_skip};

use crate::home::OmeHome;
use crate::issues;
use crate::settings::{Settings, SettingsError, SettingsStore};
use crate::{
    AppIssue, AppPhase, AppSnapshot, AppsView, Blocker, BlockerKind, CONTRACT_VERSION, Capability,
    CapabilityReport, Command, CustomDisplay, DisplayPreset, DisplayView, GuestImageSummary,
    GuestState, GuestView, HostCheckId, HostReport, HostRow, HostStatus, HostingMode,
    ImageDistribution, ImageStatus, ImageTranslator, ImagesView, InputView, Orientation,
    SettingsView, Size, StageFit, UpdateState, UpdateView, VsyncMode, WizardStep, WizardView,
};

/// Runtime dependencies supplied by the native shell.
pub struct RuntimeDeps {
    /// Read-only host observation adapter.
    pub probe: Box<dyn HostProbe>,
    /// Verified artifact store when manifest loading succeeded.
    pub artifacts: Option<ArtifactStore>,
    /// Operating-system adb session when the shell discovered the executable.
    pub adb: Option<AdbSession>,
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
    settings: Settings,
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
    display_fit: StageFit,
    active_display: Option<String>,
    custom_display: Option<CustomDisplay>,
    refresh_rate_hz: Option<u32>,
    vsync: VsyncMode,
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
    /// Opens runtime storage and loads trusted settings, profiles, and host-derived limits.
    pub fn open(home: OmeHome, deps: RuntimeDeps) -> Result<Self, AppIssue> {
        home.ensure().map_err(|_| issues::home_unavailable())?;
        let limits = hardware_limits(
            deps.probe.total_memory_bytes().ok(),
            deps.probe.logical_processors().ok(),
        );
        let settings_store = SettingsStore::new(home.as_path());
        let settings = settings_store.load(&limits).map_err(settings_issue)?;
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
        let selected_image = recommended_index(&image_profiles, &today)
            .map(|index| image_profiles[index].id.clone())
            .or_else(|| image_profiles.first().map(|profile| profile.id.clone()));
        let mut input_profiles = bundled_profiles();
        let directory =
            home_profiles_directory(home.as_path()).map_err(|_| issues::home_unavailable())?;
        if directory.is_dir() {
            let user_profiles =
                load_profiles(&directory).map_err(|_| issues::invalid_input_profile())?;
            input_profiles.extend(user_profiles);
        }
        let active_input = input_profiles.first().map(|profile| profile.id.clone());
        let feature_state = deps
            .probe
            .hypervisor_platform()
            .unwrap_or(FeatureState::Unknown);
        Ok(Self {
            home,
            deps,
            settings_store,
            settings,
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
            display_fit: StageFit::FitWindow,
            active_display: Some("hd-720".to_owned()),
            custom_display: None,
            refresh_rate_hz: None,
            vsync: VsyncMode::Off,
            issue: None,
        })
    }

    /// Returns the complete read-only projection without performing I/O.
    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            contract_version: CONTRACT_VERSION,
            product_version: self.deps.product_version.clone(),
            phase: self.phase,
            blocker: self.blocker,
            host: self.host.clone(),
            wizard: self.wizard_view(),
            images: self.images_view(),
            guest: GuestView {
                state: GuestState::Stopped,
                boot_completed: false,
                adb_connected: false,
                hosting: HostingMode::None,
                resolution: None,
                last_exit: None,
                fps: None,
                started_at: None,
                image_id: self.selected_image.clone(),
                android_version: self
                    .selected_profile()
                    .map(|profile| profile.android_version.clone()),
                api_level: self.selected_profile().map(|profile| profile.api_level),
                capabilities: CapabilityReport::default(),
                device_id: None,
                adb_address: Some(
                    match self.settings.adb_access {
                        crate::AdbAccess::Localhost => "127.0.0.1:5555",
                        crate::AdbAccess::Network => "0.0.0.0:5555",
                    }
                    .to_owned(),
                ),
                root_enabled: None,
            },
            apps: AppsView {
                available: false,
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
                multitouch: Capability::Unknown,
                suspend_hotkey: self.suspend_hotkey.clone(),
            },
            display: DisplayView {
                presets: display_presets(),
                active_id: self.active_display.clone(),
                custom: self.custom_display,
                fit: self.display_fit,
                refresh_rate_hz: self.refresh_rate_hz,
                refresh_rates: refresh_rates(self.refresh_rate_hz),
                // wiring: these become true when patched QEMU reports `refresh-rate` and
                // `swap-interval` display options.
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
                disk_usage_bytes: None,
                adb_access: self.settings.adb_access,
                binding_overlay_default: self.settings.binding_overlay_default,
            },
            update: UpdateView {
                current_version: self.deps.product_version.clone(),
                state: UpdateState::Idle,
            },
            notices: Vec::new(),
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
            Command::GuestImageSelect { id } => self.select_image(id),
            Command::InputProfileSelect { id } => self.select_input(id),
            Command::InputSuspendToggle => {
                self.input_suspended = !self.input_suspended;
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
            Command::WhpxEnable
            | Command::ArtifactDownloadStart
            | Command::ArtifactDownloadCancel
            | Command::GuestCreate { .. }
            | Command::GuestSelect { .. }
            | Command::GuestDelete { .. }
            | Command::GuestReinstall { .. }
            | Command::GuestStart
            | Command::GuestStop
            | Command::GuestRestart
            | Command::GuestRootSet { .. }
            | Command::StageRectChanged { .. }
            | Command::ScreenshotSave
            | Command::AppInstallPick
            | Command::AppUninstall { .. }
            | Command::AppLaunch { .. }
            | Command::UpdateCheck
            | Command::UpdateInstall
            | Command::DiagnosticsExport
            | Command::OpenLogsFolder
            | Command::OpenScreenshotsFolder
            | Command::OpenRegistrationPage
            | Command::GuestWindowToFront => Err(issues::not_wired()),
        }
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
        let preset = display_presets()
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
            guest_installed: false,
            guest_booted: false,
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
            guests: Vec::new(),
            active_guest: None,
        }
    }
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

/// Returns the three fixed display presets exposed by the contract.
pub fn display_presets() -> Vec<DisplayPreset> {
    vec![
        DisplayPreset {
            id: "hd-720".to_owned(),
            size: Size {
                width: 1280,
                height: 720,
            },
            density_dpi: 160,
            orientation: Orientation::Landscape,
            needs_reboot: true,
        },
        DisplayPreset {
            id: "full-hd".to_owned(),
            size: Size {
                width: 1920,
                height: 1080,
            },
            density_dpi: 240,
            orientation: Orientation::Landscape,
            needs_reboot: true,
        },
        DisplayPreset {
            id: "portrait-720".to_owned(),
            size: Size {
                width: 720,
                height: 1280,
            },
            density_dpi: 160,
            orientation: Orientation::Portrait,
            needs_reboot: true,
        },
    ]
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
    use std::fs;

    use ome_host_check::{HostCheckId as ProbeId, ProbeValue, QemuFound, TableProbe};

    use super::*;
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
                })),
            )
            .with(ProbeId::FirmwarePresent, ProbeValue::Bool(true))
            .with(ProbeId::AdbPresent, ProbeValue::Adb(Some("37".to_owned())))
            .with(
                ProbeId::DiskSpace,
                ProbeValue::Bytes(50 * 1024 * 1024 * 1024),
            )
            .with_total_memory_bytes(ProbeValue::TotalMemoryBytes(64 * 1024 * 1024 * 1024))
            .with_logical_processors(ProbeValue::LogicalProcessors(16))
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
    fn process_command_uses_stable_unwired_issue() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let issue = runtime
            .apply(Command::GuestStart)
            .expect_err("process adapter is not wired");
        assert_eq!(issue.code, "not_wired");
        assert_eq!(runtime.snapshot().issue, Some(issue));
    }
}
