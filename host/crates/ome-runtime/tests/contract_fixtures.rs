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
fn update_state_fields_are_camel_case() {
    assert_eq!(
        serde_json::to_value(UpdateState::UpToDate {
            checked_at: "2026-09-27T00:00:00Z".to_owned(),
        })
        .expect("update state JSON"),
        serde_json::json!({
            "kind": "upToDate",
            "checkedAt": "2026-09-27T00:00:00Z"
        })
    );
}

fn representative_snapshot() -> AppSnapshot {
    AppSnapshot {
        contract_version: CONTRACT_VERSION,
        product_version: "0.1.0".to_owned(),
        phase: AppPhase::Main,
        host: HostReport {
            rows: vec![HostRow {
                id: HostCheckId::CpuVirtualization,
                status: HostStatus::Ready,
                detail: "프로세서 가상화를 사용할 수 있습니다.".to_owned(),
            }],
            ready: true,
            inspected_at: Some("2026-09-26T12:00:00Z".to_owned()),
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
            gsf_id: Some("1234567890".to_owned()),
            disk_size_gib: 32,
            disk_free_bytes: Some(50 * 1024 * 1024 * 1024),
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
                kind: ExitKind::GuestReset,
                at: "2026-09-26T11:50:00Z".to_owned(),
                log_path: Some("logs/qemu-default.stderr.log".to_owned()),
            }),
            fps: Some(60),
            started_at: Some("2026-09-26T11:55:00Z".to_owned()),
        },
        apps: AppsView {
            available: true,
            items: vec![AppItem {
                package: "com.example.app".to_owned(),
                label: "Example".to_owned(),
                version_name: Some("1.0".to_owned()),
                installed_at: Some("2026-09-26T11:00:00Z".to_owned()),
            }],
            install: None,
        },
        keymap: KeymapView {
            profiles: vec![KeymapProfileSummary {
                id: "trickcal-default".to_owned(),
                name: "Default keymap".to_owned(),
                bundled: true,
                binding_count: 3,
            }],
            active_id: Some("trickcal-default".to_owned()),
            enabled: true,
        },
        display: DisplayView {
            presets: display_presets(),
            active_id: Some("hd-720".to_owned()),
            fit: StageFit::FitWindow,
        },
        settings: SettingsView {
            memory_mib: 8192,
            vcpus: 4,
            gpu_mode: GpuMode::Virgl,
            close_action: CloseAction::MinimizeToTray,
            show_fps: true,
            auto_update_check: true,
            home_dir: "C:\\Users\\Example\\AppData\\Local\\OpenMobileEmulator".to_owned(),
            disk_usage_bytes: Some(1024),
        },
        update: UpdateView {
            current_version: "0.1.0".to_owned(),
            state: UpdateState::Available {
                version: "0.2.0".to_owned(),
                notes_url: Some(
                    "https://github.com/115dkk/Open-Mobile-Emulator/releases".to_owned(),
                ),
            },
        },
        notices: vec![Notice {
            at: "2026-09-26T12:00:00Z".to_owned(),
            level: NoticeLevel::Info,
            message: "게스트가 시작되었습니다.".to_owned(),
        }],
        issue: Some(AppIssue {
            code: "not_wired".to_owned(),
            message: "아직 준비되지 않은 기능입니다.".to_owned(),
            next_action: None,
        }),
    }
}

fn every_command() -> Vec<Command> {
    vec![
        Command::HostCheckRefresh,
        Command::WizardContinue,
        Command::WizardSkip,
        Command::WizardRestart,
        Command::WhpxEnable,
        Command::ArtifactDownloadStart,
        Command::ArtifactDownloadCancel,
        Command::GuestDiskCreate { size_gib: 32 },
        Command::GuestStart,
        Command::GuestStop,
        Command::GuestRestart,
        Command::StageRectChanged {
            rect: StageRect {
                x: 10.0,
                y: 20.0,
                width: 1280.0,
                height: 720.0,
                scale_factor: 1.25,
            },
        },
        Command::ScreenshotSave,
        Command::AppInstallPick,
        Command::AppUninstall {
            package: "com.example.app".to_owned(),
        },
        Command::AppLaunch {
            package: "com.example.app".to_owned(),
        },
        Command::KeymapSetActive {
            id: Some("trickcal-default".to_owned()),
        },
        Command::KeymapSetEnabled { enabled: true },
        Command::KeymapDelete {
            id: "custom".to_owned(),
        },
        Command::DisplayPresetApply {
            id: "hd-720".to_owned(),
        },
        Command::StageFitSet {
            fit: StageFit::OneToOne,
        },
        Command::SettingsSave {
            settings: SettingsInput {
                memory_mib: 8192,
                vcpus: 4,
                gpu_mode: GpuMode::Virgl,
                close_action: CloseAction::MinimizeToTray,
                show_fps: false,
                auto_update_check: true,
            },
        },
        Command::UpdateCheck,
        Command::UpdateInstall,
        Command::DiagnosticsExport,
        Command::OpenLogsFolder,
        Command::OpenScreenshotsFolder,
        Command::OpenRegistrationPage,
        Command::GuestWindowToFront,
    ]
}
