// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use ome_adb::AdbSession;
use ome_artifacts::ArtifactStore;
use ome_host_check::{FeatureState, HostProbe, HostReadiness, Verdict};
use ome_keymap::{KeymapProfile, load_presets};
use ome_wizard::{Facts, Outcome, Step, WizardState, advance, can_continue, can_skip};

use crate::home::OmeHome;
use crate::issues;
use crate::settings::{Settings, SettingsError, SettingsStore};
use crate::{
    AppIssue, AppPhase, AppSnapshot, AppsView, CONTRACT_VERSION, Command, DisplayPreset,
    DisplayView, GuestState, GuestView, HostCheckId, HostReport, HostRow, HostStatus, HostingMode,
    KeymapProfileSummary, KeymapView, Orientation, SettingsView, Size, StageFit, UpdateState,
    UpdateView, WizardStep, WizardView,
};

/// Runtime dependencies supplied by the native shell.
///
/// All adapters are native-owned and fixed at open time; no command carries executable paths,
/// network URLs, or arbitrary file-system paths from the webview.
pub struct RuntimeDeps {
    /// Read-only host observation adapter.
    pub probe: Box<dyn HostProbe>,
    /// Verified artifact store when manifest loading succeeded.
    pub artifacts: Option<ArtifactStore>,
    /// Guest adb session when the native shell discovered the executable.
    pub adb: Option<AdbSession>,
    /// Product version presented in every snapshot.
    pub product_version: String,
}

impl std::fmt::Debug for RuntimeDeps {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeDeps")
            .field("artifacts", &self.artifacts.is_some())
            .field("adb", &self.adb.is_some())
            .field("product_version", &self.product_version)
            .finish_non_exhaustive()
    }
}

/// Native product runtime and single owner of snapshot projection and command decisions.
pub struct AppRuntime {
    home: OmeHome,
    deps: RuntimeDeps,
    settings_store: SettingsStore,
    settings: Settings,
    host: HostReport,
    feature_state: FeatureState,
    wizard: WizardState,
    phase: AppPhase,
    keymaps: Vec<RuntimeKeymap>,
    active_keymap: Option<String>,
    keymap_enabled: bool,
    display_fit: StageFit,
    active_display: Option<String>,
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

#[derive(Clone, Debug)]
struct RuntimeKeymap {
    profile: KeymapProfile,
    bundled: bool,
}

impl AppRuntime {
    /// Opens runtime storage, loads settings, and initializes a conservative snapshot.
    ///
    /// Opening creates fixed OME home directories but performs no host mutation, network request,
    /// guest start, adb command, or browser action. Corrupt settings fail closed with an [`AppIssue`].
    pub fn open(home: OmeHome, deps: RuntimeDeps) -> Result<Self, AppIssue> {
        home.ensure().map_err(|_| issues::home_unavailable())?;
        let settings_store = SettingsStore::new(home.as_path());
        let settings = settings_store.load().map_err(settings_issue)?;
        let keymaps = bundled_presets()
            .into_iter()
            .map(|profile| RuntimeKeymap {
                profile,
                bundled: true,
            })
            .collect::<Vec<_>>();
        let active_keymap = keymaps.first().map(|entry| entry.profile.id.clone());
        let feature_state = deps
            .probe
            .hypervisor_platform()
            .unwrap_or(FeatureState::Unknown);
        Ok(Self {
            home,
            deps,
            settings_store,
            settings,
            host: empty_host_report(),
            feature_state,
            wizard: WizardState::default(),
            phase: AppPhase::Wizard,
            keymaps,
            active_keymap,
            keymap_enabled: true,
            display_fit: StageFit::FitWindow,
            active_display: Some("hd-720".to_owned()),
            issue: None,
        })
    }

    /// Returns the complete read-only projection without performing I/O or observation.
    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            contract_version: CONTRACT_VERSION,
            product_version: self.deps.product_version.clone(),
            phase: self.phase,
            host: self.host.clone(),
            wizard: self.wizard_view(),
            guest: GuestView {
                state: GuestState::Stopped,
                boot_completed: false,
                adb_connected: false,
                hosting: HostingMode::None,
                resolution: None,
                last_exit: None,
                fps: None,
                started_at: None,
            },
            apps: AppsView {
                available: false,
                items: Vec::new(),
                install: None,
            },
            keymap: KeymapView {
                profiles: self
                    .keymaps
                    .iter()
                    .map(|entry| KeymapProfileSummary {
                        id: entry.profile.id.clone(),
                        name: entry.profile.name.clone(),
                        bundled: entry.bundled,
                        binding_count: u32::try_from(entry.profile.bindings.len())
                            .unwrap_or(u32::MAX),
                    })
                    .collect(),
                active_id: self.active_keymap.clone(),
                enabled: self.keymap_enabled,
            },
            display: DisplayView {
                presets: display_presets(),
                active_id: self.active_display.clone(),
                fit: self.display_fit,
            },
            settings: SettingsView {
                memory_mib: self.settings.memory_mib,
                vcpus: self.settings.vcpus,
                gpu_mode: self.settings.gpu_mode,
                close_action: self.settings.close_action,
                show_fps: self.settings.show_fps,
                auto_update_check: self.settings.auto_update_check,
                home_dir: self.home.as_path().to_string_lossy().into_owned(),
                disk_usage_bytes: None,
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
    ///
    /// Wired commands mutate in-memory state only after their validation or persistence succeeds.
    /// Unwired commands leave state unchanged and return the stable `not_wired` issue. The last issue
    /// is retained in future snapshots until a successful command clears it.
    pub fn apply(&mut self, command: Command) -> Result<AppSnapshot, AppIssue> {
        let result = self.apply_inner(command);
        match result {
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
            Command::WizardRestart => {
                self.wizard = advance(self.wizard.clone(), Outcome::Restart);
                self.phase = AppPhase::Wizard;
                Ok(())
            }
            Command::WhpxEnable => Err(issues::not_wired()),
            Command::SettingsSave { settings } => {
                let validated = Settings::validate(settings).map_err(settings_issue)?;
                self.settings_store
                    .save(&validated)
                    .map_err(settings_issue)?;
                self.settings = validated;
                Ok(())
            }
            Command::KeymapSetActive { id } => {
                if id
                    .as_ref()
                    .is_some_and(|id| !self.keymaps.iter().any(|entry| entry.profile.id == *id))
                {
                    return Err(issues::keymap_not_found());
                }
                self.active_keymap = id;
                Ok(())
            }
            Command::KeymapSetEnabled { enabled } => {
                self.keymap_enabled = enabled;
                Ok(())
            }
            Command::KeymapDelete { id } => {
                let Some(position) = self.keymaps.iter().position(|entry| entry.profile.id == id)
                else {
                    return Err(issues::keymap_not_found());
                };
                if self.keymaps[position].bundled {
                    return Err(issues::keymap_bundled());
                }
                self.keymaps.remove(position);
                if self.active_keymap.as_deref() == Some(id.as_str()) {
                    self.active_keymap = None;
                }
                Ok(())
            }
            Command::StageFitSet { fit } => {
                self.display_fit = fit;
                Ok(())
            }
            Command::DisplayPresetApply { id } => {
                let preset = display_presets()
                    .into_iter()
                    .find(|preset| preset.id == id)
                    .ok_or_else(issues::display_preset_not_found)?;
                let Some(adb) = self.deps.adb.as_ref() else {
                    self.active_display = Some(preset.id);
                    return Ok(());
                };
                adb.set_display_size(preset.size.width, preset.size.height)
                    .and_then(|()| adb.set_display_density(preset.density_dpi))
                    .map_err(|_| issues::guest_connection_unavailable())?;
                self.active_display = Some(preset.id);
                Ok(())
            }
            Command::OpenRegistrationPage => Err(issues::not_wired()),
            Command::ArtifactDownloadStart
            | Command::ArtifactDownloadCancel
            | Command::GuestDiskCreate { .. }
            | Command::GuestStart
            | Command::GuestStop
            | Command::GuestRestart
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
            | Command::GuestWindowToFront => Err(issues::not_wired()),
        }
    }

    fn refresh_host(&mut self) {
        self.feature_state = self
            .deps
            .probe
            .hypervisor_platform()
            .unwrap_or(FeatureState::Unknown);
        let report = HostReadiness::inspect(self.deps.probe.as_ref());
        self.host = HostReport {
            rows: report.rows.into_iter().map(convert_host_row).collect(),
            ready: report.verdict != Verdict::Blocked,
            inspected_at: None,
        };
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
            registration_shown: true,
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
            gsf_id: None,
            disk_size_gib: 32,
            disk_free_bytes: None,
        }
    }
}

fn empty_host_report() -> HostReport {
    HostReport {
        rows: Vec::new(),
        ready: false,
        inspected_at: None,
    }
}

fn bundled_presets() -> Vec<KeymapProfile> {
    let embedded = include_str!("../../../presets/com.epidgames.trickcalrevive.json");
    KeymapProfile::parse(embedded).into_iter().collect()
}

/// Loads additional keymap profiles from a trusted native directory.
///
/// This helper returns only fully validated profiles and does not merge or overwrite IDs.
pub fn load_keymap_directory(
    directory: impl AsRef<std::path::Path>,
) -> Result<Vec<KeymapProfile>, ome_keymap::ProfileError> {
    load_presets(directory)
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
        Step::GoogleRegistration => WizardStep::GoogleRegistration,
        Step::AppInstall => WizardStep::AppInstall,
        Step::Done => WizardStep::Done,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ome_host_check::{HostCheckId as ProbeId, ProbeValue, QemuFound, TableProbe};

    use super::*;
    use crate::{CloseAction, GpuMode};

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
                product_version: "0.1.0".to_owned(),
            },
        )
        .expect("runtime");
        (directory, runtime)
    }

    #[test]
    fn host_refresh_and_enabled_feature_skip_consent() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let snapshot = runtime
            .apply(Command::HostCheckRefresh)
            .expect("refresh succeeds");
        assert!(snapshot.host.ready);
        let snapshot = runtime
            .apply(Command::WizardContinue)
            .expect("continue succeeds");
        assert_eq!(snapshot.wizard.step, WizardStep::ArtifactDownload);
    }

    #[test]
    fn disabled_feature_needs_attention_and_wizard_can_reach_consent() {
        let (_directory, mut runtime) = runtime(FeatureState::Disabled);
        let snapshot = runtime
            .apply(Command::HostCheckRefresh)
            .expect("refresh succeeds");
        assert!(snapshot.host.ready);
        let snapshot = runtime
            .apply(Command::WizardContinue)
            .expect("consent step is reachable");
        assert_eq!(snapshot.wizard.step, WizardStep::WhpxConsent);
        assert!(!snapshot.wizard.can_continue);
        let issue = runtime
            .apply(Command::WhpxEnable)
            .expect_err("setup broker is not wired");
        assert_eq!(issue.code, "not_wired");
        let issue = runtime
            .apply(Command::WizardContinue)
            .expect_err("unwired setup cannot complete consent");
        assert_eq!(issue.code, "wizard_cannot_continue");
        assert_eq!(runtime.snapshot().wizard.step, WizardStep::WhpxConsent);
    }

    #[test]
    fn settings_save_persists_only_valid_input() {
        let (directory, mut runtime) = runtime(FeatureState::Disabled);
        runtime
            .apply(Command::SettingsSave {
                settings: crate::SettingsInput {
                    memory_mib: 12_288,
                    vcpus: 6,
                    gpu_mode: GpuMode::Software,
                    close_action: CloseAction::StopGuest,
                    show_fps: true,
                    auto_update_check: false,
                },
            })
            .expect("save settings");
        let json =
            fs::read_to_string(directory.path().join("home/settings.json")).expect("settings file");
        assert!(json.contains("12288"));
        let issue = runtime
            .apply(Command::SettingsSave {
                settings: crate::SettingsInput {
                    memory_mib: 1024,
                    vcpus: 1,
                    gpu_mode: GpuMode::Virgl,
                    close_action: CloseAction::MinimizeToTray,
                    show_fps: false,
                    auto_update_check: true,
                },
            })
            .expect_err("invalid settings fail");
        assert_eq!(issue.code, "invalid_settings");
    }

    #[test]
    fn wizard_does_not_invent_completion_for_unwired_work() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        runtime
            .apply(Command::HostCheckRefresh)
            .expect("refresh succeeds");
        let snapshot = runtime
            .apply(Command::WizardContinue)
            .expect("enabled feature skips consent");
        assert_eq!(snapshot.wizard.step, WizardStep::ArtifactDownload);
        assert!(!snapshot.wizard.can_continue);
        let issue = runtime
            .apply(Command::WizardContinue)
            .expect_err("artifact completion is not invented");
        assert_eq!(issue.code, "wizard_cannot_continue");
    }

    #[test]
    fn display_id_updates_while_adb_is_absent() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let snapshot = runtime
            .apply(Command::DisplayPresetApply {
                id: "full-hd".to_owned(),
            })
            .expect("the preset is stored until adb is available");
        assert_eq!(snapshot.display.active_id.as_deref(), Some("full-hd"));
    }

    #[test]
    fn bundled_keymap_cannot_be_deleted() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let issue = runtime
            .apply(Command::KeymapDelete {
                id: "trickcal-default".to_owned(),
            })
            .expect_err("bundled profile is protected");
        assert_eq!(issue.code, "keymap_bundled");
    }

    #[test]
    fn unwired_command_uses_stable_issue() {
        let (_directory, mut runtime) = runtime(FeatureState::Enabled);
        let issue = runtime
            .apply(Command::GuestStart)
            .expect_err("guest start is not wired");
        assert_eq!(issue.code, "not_wired");
        assert_eq!(runtime.snapshot().issue, Some(issue));
    }
}
