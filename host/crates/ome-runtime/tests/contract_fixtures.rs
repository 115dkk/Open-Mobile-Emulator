// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use ome_runtime::*;

#[test]
fn contract_fixtures_match_serialized_types() {
    let fixture_directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/contract");
    let snapshot = serde_json::to_string_pretty(&representative_snapshot()).expect("snapshot JSON");
    let commands = serde_json::to_string_pretty(&every_command()).expect("commands JSON");
    check_or_update(&fixture_directory.join("snapshot.sample.json"), &snapshot);
    check_or_update(&fixture_directory.join("commands.sample.json"), &commands);
}

fn check_or_update(path: &Path, expected: &str) {
    let expected = format!("{expected}\n");
    if std::env::var_os("OME_UPDATE_FIXTURES").as_deref() == Some(std::ffi::OsStr::new("1")) {
        fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create fixture directory");
        fs::write(path, expected).expect("write fixture");
        return;
    }
    let actual =
        fs::read_to_string(path).expect("fixture exists; set OME_UPDATE_FIXTURES=1 to create it");
    assert_eq!(actual, expected, "fixture {} is stale", path.display());
}

#[test]
fn tagged_variant_fields_are_camel_case() {
    assert_eq!(
        serde_json::to_value(Command::InputBindingRemove {
            profile_id: "custom".to_owned(),
            id: "binding".to_owned(),
        })
        .expect("command JSON"),
        serde_json::json!({
            "kind": "inputBindingRemove",
            "profileId": "custom",
            "id": "binding"
        })
    );
}

fn point(x: f64, y: f64) -> LogicalPoint {
    LogicalPoint { x, y }
}

fn representative_profile() -> InputProfile {
    InputProfile {
        id: "example-input".to_owned(),
        name: "Example input".to_owned(),
        bundled: false,
        target_package: Some("com.example.app".to_owned()),
        reference_aspect: ome_input::Size {
            width: 16,
            height: 9,
        },
        anchor: Anchor::Center,
        bindings: vec![
            Binding {
                id: "tap".to_owned(),
                trigger: Trigger::Key {
                    code: "Space".to_owned(),
                },
                action: BindingAction::Tap {
                    at: point(0.5, 0.5),
                    hold: true,
                },
            },
            Binding {
                id: "swipe".to_owned(),
                trigger: Trigger::MouseButton {
                    button: MouseButton::Left,
                },
                action: BindingAction::Swipe {
                    from: point(0.2, 0.5),
                    to: point(0.8, 0.5),
                    duration_ms: 240,
                },
            },
            Binding {
                id: "joystick".to_owned(),
                trigger: Trigger::KeySet {
                    up: "KeyW".to_owned(),
                    down: "KeyS".to_owned(),
                    left: "KeyA".to_owned(),
                    right: "KeyD".to_owned(),
                },
                action: BindingAction::Joystick {
                    center: point(0.25, 0.75),
                    radius: 0.15,
                },
            },
            Binding {
                id: "mouse".to_owned(),
                trigger: Trigger::MouseButton {
                    button: MouseButton::Right,
                },
                action: BindingAction::MouseTap { at: None },
            },
            Binding {
                id: "wheel".to_owned(),
                trigger: Trigger::Wheel {
                    direction: WheelDirection::Down,
                },
                action: BindingAction::WheelSwipe {
                    at: point(0.5, 0.5),
                    distance: 0.2,
                },
            },
            Binding {
                id: "pass".to_owned(),
                trigger: Trigger::Key {
                    code: "Enter".to_owned(),
                },
                action: BindingAction::PassThrough,
            },
        ],
    }
}

fn capabilities() -> CapabilityReport {
    CapabilityReport {
        probed_at: Some("2026-09-27T12:00:00Z".to_owned()),
        items: vec![
            CapabilityItem {
                id: CapabilityId::BootMarker,
                state: Capability::Available,
            },
            CapabilityItem {
                id: CapabilityId::AppList,
                state: Capability::Unavailable,
            },
            CapabilityItem {
                id: CapabilityId::DisplaySize,
                state: Capability::Unknown,
            },
            CapabilityItem {
                id: CapabilityId::MediaVolume,
                state: Capability::Available,
            },
            CapabilityItem {
                id: CapabilityId::DeviceId,
                state: Capability::Available,
            },
            CapabilityItem {
                id: CapabilityId::Screenshot,
                state: Capability::Available,
            },
            CapabilityItem {
                id: CapabilityId::ForegroundApp,
                state: Capability::Available,
            },
            CapabilityItem {
                id: CapabilityId::Multitouch,
                state: Capability::Unknown,
            },
            CapabilityItem {
                id: CapabilityId::NativeBridge,
                state: Capability::Available,
            },
            CapabilityItem {
                id: CapabilityId::Root,
                state: Capability::Available,
            },
        ],
    }
}

fn representative_snapshot() -> AppSnapshot {
    AppSnapshot {
        contract_version: CONTRACT_VERSION,
        product_version: "0.1.0".to_owned(),
        phase: AppPhase::Main,
        blocker: Some(Blocker {
            kind: BlockerKind::HypervisorPlatformOff,
        }),
        host: HostReport {
            rows: vec![HostRow {
                id: HostCheckId::CpuVirtualization,
                status: HostStatus::Ready,
                detail: "사용할 수 있습니다.".to_owned(),
            }],
            ready: true,
            inspected_at: Some("2026-09-27T12:00:00Z".to_owned()),
        },
        wizard: WizardView {
            step: WizardStep::Done,
            can_continue: false,
            can_skip: false,
            download: Some(TransferProgress {
                stage: TransferStage::Verified,
                done_bytes: 1024,
                total_bytes: Some(1024),
                bytes_per_second: Some(512),
                label: "guest.iso".to_owned(),
            }),
            image_id: Some("bliss-16.9.7-android-13".to_owned()),
            install_guide: vec!["Installation을 선택합니다.".to_owned()],
            disk_size_gib: 32,
            disk_free_bytes: Some(50 * 1024 * 1024 * 1024),
        },
        images: ImagesView {
            profiles: vec![GuestImageSummary {
                id: "bliss-16.9.7-android-13".to_owned(),
                display_name: "안드로이드 13".to_owned(),
                android_version: "13".to_owned(),
                api_level: 33,
                distribution: ImageDistribution::Bliss,
                translator: ImageTranslator::NdkTranslation,
                size_bytes: Some(2_429_550_592),
                status: ImageStatus::Verified,
                released_at: Some("2024-10-11".to_owned()),
                verified_games: 1,
                recommended: true,
            }],
            guests: vec![GuestSummary {
                name: "default".to_owned(),
                image_id: "bliss-16.9.7-android-13".to_owned(),
                android_version: "13".to_owned(),
                disk_size_gib: 32,
                last_started_at: Some("2026-09-27T11:55:00Z".to_owned()),
                capabilities: capabilities(),
            }],
            active_guest: Some("default".to_owned()),
        },
        guest: GuestView {
            state: GuestState::Running,
            boot_completed: true,
            adb_connected: true,
            hosting: HostingMode::Embedded,
            resolution: Some(Size {
                width: 1280,
                height: 720,
            }),
            last_exit: Some(LastExit {
                kind: ExitKind::BootTimeout,
                at: "2026-09-27T11:50:00Z".to_owned(),
                log_path: Some("logs/qemu-default.stderr.log".to_owned()),
            }),
            fps: Some(60),
            started_at: Some("2026-09-27T11:55:00Z".to_owned()),
            image_id: Some("bliss-16.9.7-android-13".to_owned()),
            android_version: Some("13".to_owned()),
            api_level: Some(33),
            capabilities: capabilities(),
            device_id: Some("499602d2".to_owned()),
            device_id_decimal: Some("1234567890".to_owned()),
            google_accounts: Some(1),
            registration_opened_at: Some("2026-09-27T12:00:00+09:00".to_owned()),
            add_account_supported: true,
            pid: Some(4242),
            adb_address: Some("127.0.0.1:5555".to_owned()),
            root_enabled: Some(false),
            media_volume: Some(8),
        },
        apps: AppsView {
            available: true,
            items: vec![AppItem {
                package: "com.example.app".to_owned(),
                label: "Example".to_owned(),
                version_name: Some("1.0".to_owned()),
                version_code: Some(100),
                installed_at: Some("2026-09-27T11:00:00Z".to_owned()),
            }],
            install: None,
        },
        input: InputView {
            profiles: vec![representative_profile()],
            active_id: Some("example-input".to_owned()),
            suspended: true,
            editing: true,
            auto_apply: true,
            foreground_package: Some("com.example.app".to_owned()),
            multitouch: Capability::Unavailable,
            suspend_hotkey: "F12".to_owned(),
            overlay_visible: true,
        },
        display: DisplayView {
            presets: display_presets(),
            active_id: None,
            custom: Some(CustomDisplay {
                size: Size {
                    width: 1600,
                    height: 904,
                },
                density_dpi: 240,
            }),
            fit: StageFit::FitWindow,
            refresh_rate_hz: Some(100),
            refresh_rates: vec![60, 75, 90, 100, 120, 144],
            refresh_supported: false,
            vsync: VsyncMode::Adaptive,
            vsync_supported: false,
        },
        settings: SettingsView {
            memory_mib: 8192,
            memory_mib_min: 4096,
            memory_mib_max: 61_440,
            vcpus: 4,
            vcpus_max: 16,
            gpu_mode: GpuMode::Virgl,
            close_action: CloseAction::StopGuest,
            show_fps: true,
            auto_update_check: true,
            home_dir: "C:\\Users\\Example\\AppData\\Local\\OpenMobileEmulator".to_owned(),
            disk_usage_bytes: Some(1024),
            adb_access: AdbAccess::Localhost,
            binding_overlay_default: true,
        },
        update: UpdateView {
            current_version: "0.1.0".to_owned(),
            state: UpdateState::Available {
                version: "0.2.0".to_owned(),
                notes_url: Some(
                    "https://github.com/115dkk/Open-Mobile-Emulator/releases".to_owned(),
                ),
                asset: UpdateAsset {
                    name: "Open-Mobile-Emulator-0.2.0.exe".to_owned(),
                    size_bytes: 2048,
                },
            },
        },
        notices: vec![Notice {
            at: "2026-09-27T12:00:00Z".to_owned(),
            level: NoticeLevel::Info,
            message: "가상 머신을 시작했습니다.".to_owned(),
        }],
        issue: Some(AppIssue {
            code: "not_wired".to_owned(),
            message: "요청한 기능은 아직 준비되지 않았습니다.".to_owned(),
            next_action: Some("다음 제품 업데이트 뒤 다시 시도하십시오.".to_owned()),
        }),
    }
}

fn every_command() -> Vec<Command> {
    vec![
        Command::HostCheckRefresh,
        Command::WizardContinue,
        Command::WizardSkip,
        Command::WizardDefer,
        Command::OpenHelp {
            topic: HelpTopic::HypervisorPlatform,
        },
        Command::AppQuit,
        Command::WhpxEnable,
        Command::ArtifactDownloadStart,
        Command::ArtifactDownloadCancel,
        Command::GuestImageSelect {
            id: "bliss-16.9.7-android-13".to_owned(),
        },
        Command::GuestCreate {
            image_id: "bliss-16.9.7-android-13".to_owned(),
            size_gib: 32,
        },
        Command::GuestSelect {
            id: "default".to_owned(),
        },
        Command::GuestDelete {
            id: "old".to_owned(),
        },
        Command::GuestReinstall {
            name: "default".to_owned(),
        },
        Command::GuestStart,
        Command::GuestStop,
        Command::GuestRestart,
        Command::GuestRootSet { enabled: true },
        Command::GuestVolumeSet { index: 8 },
        Command::StageRectChanged {
            rect: StageRect {
                x: 10.0,
                y: 20.0,
                width: 1280.0,
                height: 720.0,
                scale_factor: 1.25,
            },
        },
        Command::StageHidden,
        Command::ScreenshotSave,
        Command::AppInstallPick,
        Command::AppInstallCancel,
        Command::AppUninstall {
            package: "com.example.app".to_owned(),
        },
        Command::AppLaunch {
            package: "com.example.app".to_owned(),
        },
        Command::InputProfileSelect {
            id: Some("example-input".to_owned()),
        },
        Command::InputSuspendToggle,
        Command::InputOverlayToggle,
        Command::InputProfileDelete {
            id: "custom".to_owned(),
        },
        Command::InputProfileSave {
            profile: representative_profile(),
        },
        Command::InputBindingUpsert {
            profile_id: "example-input".to_owned(),
            binding: representative_profile().bindings[0].clone(),
        },
        Command::InputBindingRemove {
            profile_id: "example-input".to_owned(),
            id: "tap".to_owned(),
        },
        Command::InputEditorToggle,
        Command::InputAutoApplySet { enabled: true },
        Command::InputSuspendHotkeySet {
            code: "F12".to_owned(),
        },
        Command::DisplayPresetApply {
            id: "hd-720".to_owned(),
        },
        Command::DisplayCustomApply {
            size: Size {
                width: 1600,
                height: 904,
            },
            density_dpi: 240,
        },
        Command::DisplayRefreshSet { hz: Some(120) },
        Command::DisplayVsyncSet {
            mode: VsyncMode::On,
        },
        Command::StageFitSet {
            fit: StageFit::OneToOne,
        },
        Command::SettingsSave {
            settings: SettingsInput {
                memory_mib: 8192,
                vcpus: 4,
                gpu_mode: GpuMode::Virgl,
                close_action: CloseAction::StopGuest,
                show_fps: false,
                auto_update_check: true,
                adb_access: AdbAccess::Network,
                binding_overlay_default: false,
            },
        },
        Command::UpdateCheck,
        Command::UpdateInstall,
        Command::DiagnosticsExport,
        Command::OpenLogsFolder,
        Command::OpenScreenshotsFolder,
        Command::OpenHomeFolder,
        Command::CopyToClipboard {
            item: ClipboardItem::DeviceId,
        },
        Command::OpenRegistrationPage,
        Command::GoogleAccountAddOpen,
        Command::GuestWindowToFront,
    ]
}
