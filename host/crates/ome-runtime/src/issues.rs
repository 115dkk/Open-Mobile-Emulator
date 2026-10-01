// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use crate::AppIssue;

/// Returns the stable placeholder for work whose process or platform adapter is not connected.
pub fn not_wired() -> AppIssue {
    AppIssue {
        code: "not_wired".to_owned(),
        message: "요청한 기능은 아직 준비되지 않았습니다.".to_owned(),
        next_action: Some("다음 제품 업데이트 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue for invalid settings input.
pub fn invalid_settings() -> AppIssue {
    AppIssue {
        code: "invalid_settings".to_owned(),
        message: "설정값이 이 PC에서 허용하는 범위를 벗어나 저장하지 않았습니다.".to_owned(),
        next_action: Some(
            "화면에 표시된 메모리와 프로세서 범위 안에서 다시 지정하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue for settings storage failures.
pub fn settings_unavailable() -> AppIssue {
    AppIssue {
        code: "settings_unavailable".to_owned(),
        message: "설정을 저장하지 못해 이전 설정을 유지합니다.".to_owned(),
        next_action: Some(
            "저장 공간과 앱의 파일 접근 권한을 확인한 뒤 다시 시도하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue for corrupt, oversized, or unsupported settings documents.
pub fn settings_invalid_document() -> AppIssue {
    AppIssue {
        code: "settings_invalid_document".to_owned(),
        message: "저장된 설정을 읽지 못해 앱을 시작하지 않았습니다.".to_owned(),
        next_action: Some("설정 파일을 보관한 뒤 앱을 다시 설치하십시오.".to_owned()),
    }
}

/// Returns an issue when the OME home directory cannot be prepared.
pub fn home_unavailable() -> AppIssue {
    AppIssue {
        code: "home_unavailable".to_owned(),
        message: "앱 데이터 폴더를 준비하지 못했습니다.".to_owned(),
        next_action: Some("저장 공간과 폴더 접근 권한을 확인한 뒤 앱을 다시 여십시오.".to_owned()),
    }
}

/// Returns an issue when image profiles cannot be loaded.
pub fn image_profiles_invalid() -> AppIssue {
    AppIssue {
        code: "image_profiles_invalid".to_owned(),
        message: "운영체제 이미지 목록을 읽지 못해 앱을 시작하지 않았습니다.".to_owned(),
        next_action: Some("앱을 다시 설치한 뒤 다시 여십시오.".to_owned()),
    }
}

/// Returns an issue when a requested image profile does not exist.
pub fn image_not_found() -> AppIssue {
    AppIssue {
        code: "image_not_found".to_owned(),
        message: "선택한 운영체제 이미지를 찾지 못해 선택을 바꾸지 않았습니다.".to_owned(),
        next_action: Some("운영체제 이미지 목록을 다시 확인한 뒤 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when a requested input profile does not exist.
pub fn input_profile_not_found() -> AppIssue {
    AppIssue {
        code: "input_profile_not_found".to_owned(),
        message: "선택한 입력 프로필을 찾지 못해 설정을 바꾸지 않았습니다.".to_owned(),
        next_action: Some("입력 프로필 목록을 다시 확인한 뒤 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when a bundled input profile cannot be overwritten or deleted.
pub fn input_profile_bundled() -> AppIssue {
    AppIssue {
        code: "input_profile_bundled".to_owned(),
        message: "앱에 포함된 입력 프로필은 바꾸거나 삭제할 수 없습니다.".to_owned(),
        next_action: Some("프로필을 복제해 새 항목으로 저장하십시오.".to_owned()),
    }
}

/// Returns an issue when an input profile or binding fails validation.
pub fn invalid_input_profile() -> AppIssue {
    AppIssue {
        code: "invalid_input_profile".to_owned(),
        message: "입력 프로필에 허용되지 않는 값이 있어 저장하지 않았습니다.".to_owned(),
        next_action: Some(
            "키 중복, 좌표, 지속 시간과 반지름을 확인한 뒤 다시 저장하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when a suspend hotkey is not a valid keyboard code.
pub fn invalid_suspend_hotkey() -> AppIssue {
    AppIssue {
        code: "invalid_suspend_hotkey".to_owned(),
        message: "일시 중지 단축키가 올바르지 않아 바꾸지 않았습니다.".to_owned(),
        next_action: Some("키보드의 키 하나를 다시 누르십시오.".to_owned()),
    }
}

/// Returns an issue when a requested display preset does not exist.
pub fn display_preset_not_found() -> AppIssue {
    AppIssue {
        code: "display_preset_not_found".to_owned(),
        message: "선택한 화면 크기 설정을 찾지 못해 적용하지 않았습니다.".to_owned(),
        next_action: Some("화면 크기 목록에서 다시 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when custom display values are invalid.
pub fn invalid_display() -> AppIssue {
    AppIssue {
        code: "invalid_display".to_owned(),
        message: "화면 설정값이 허용 범위를 벗어나 적용하지 않았습니다.".to_owned(),
        next_action: Some(
            "크기와 밀도 또는 주사율을 허용 범위 안에서 다시 지정하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when a media-volume index is outside the operating-system range.
pub fn invalid_media_volume() -> AppIssue {
    AppIssue {
        code: "invalid_media_volume".to_owned(),
        message: "운영체제 볼륨 값이 허용 범위를 벗어나 적용하지 않았습니다.".to_owned(),
        next_action: Some("0부터 15까지의 값으로 다시 지정하십시오.".to_owned()),
    }
}

/// Returns an issue when applying a display setting requires an unavailable connection.
pub fn operating_system_connection_unavailable() -> AppIssue {
    AppIssue {
        code: "operating_system_connection_unavailable".to_owned(),
        message: "운영체제에 연결되지 않아 화면 크기를 적용하지 못했습니다.".to_owned(),
        next_action: Some(
            "가상 머신을 시작하고 운영체제 부팅이 끝난 뒤 다시 시도하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when wizard progression is not allowed.
pub fn wizard_cannot_continue() -> AppIssue {
    AppIssue {
        code: "wizard_cannot_continue".to_owned(),
        message: "현재 설정 작업이 끝나지 않아 다음 단계로 넘어가지 않았습니다.".to_owned(),
        next_action: Some("화면의 안내에 따라 현재 작업을 마친 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when a mandatory wizard step is asked to skip.
pub fn wizard_cannot_skip() -> AppIssue {
    AppIssue {
        code: "wizard_cannot_skip".to_owned(),
        message: "이 설정 단계는 건너뛸 수 없습니다.".to_owned(),
        next_action: Some("화면의 안내에 따라 설정을 마치십시오.".to_owned()),
    }
}

/// Returns an issue when no installed operating system matches the request.
pub fn guest_not_found() -> AppIssue {
    AppIssue {
        code: "operating_system_not_found".to_owned(),
        message: "선택한 운영체제를 찾지 못했습니다.".to_owned(),
        next_action: Some("설치된 운영체제 목록에서 다시 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when a destructive operation requires a stopped operating system.
pub fn operating_system_running() -> AppIssue {
    AppIssue {
        code: "operating_system_running".to_owned(),
        message: "운영체제가 실행 중이어서 요청한 작업을 하지 않았습니다.".to_owned(),
        next_action: Some("운영체제를 끈 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when a running, booted operating system is required.
pub fn operating_system_not_running() -> AppIssue {
    AppIssue {
        code: "operating_system_not_running".to_owned(),
        message: "운영체제가 실행 중이 아니어서 요청한 작업을 하지 못했습니다.".to_owned(),
        next_action: Some(
            "가상 머신을 시작하고 운영체제 부팅이 끝난 뒤 다시 시도하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when the selected generation lacks a command.
pub fn operating_system_unsupported() -> AppIssue {
    AppIssue {
        code: "operating_system_unsupported".to_owned(),
        message: "이 운영체제에서는 요청한 작업을 지원하지 않습니다.".to_owned(),
        next_action: Some(
            "다른 운영체제를 선택하거나 앱을 업데이트한 뒤 다시 시도하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when virtual-machine supervision is unavailable.
pub fn process_unavailable() -> AppIssue {
    AppIssue {
        code: "virtual_machine_unavailable".to_owned(),
        message: "가상 머신을 제어할 수 없어 요청한 작업을 하지 못했습니다.".to_owned(),
        next_action: Some("앱을 종료한 뒤 다시 실행하십시오.".to_owned()),
    }
}

/// Returns an issue when a start request was rejected synchronously.
pub fn process_start_failed() -> AppIssue {
    AppIssue {
        code: "virtual_machine_start_failed".to_owned(),
        message: "가상 머신이 시작하지 못했습니다.".to_owned(),
        next_action: Some(
            "다시 시작을 시도하십시오. 반복될 경우 로그를 첨부해 문제를 보고하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when validated runtime configuration could not be built.
pub fn invalid_guest_configuration() -> AppIssue {
    AppIssue {
        code: "invalid_virtual_machine_configuration".to_owned(),
        message: "가상 머신 설정값이 올바르지 않아 시작하지 않았습니다.".to_owned(),
        next_action: Some("운영체제 설정을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when QEMU discovery has no usable executable.
pub fn qemu_unavailable() -> AppIssue {
    AppIssue {
        code: "virtual_machine_files_missing".to_owned(),
        message: "가상 머신 구성 요소를 찾을 수 없습니다.".to_owned(),
        next_action: Some("앱을 다시 설치하십시오.".to_owned()),
    }
}

/// Returns an issue when UEFI firmware discovery is incomplete.
pub fn firmware_unavailable() -> AppIssue {
    AppIssue {
        code: "virtual_machine_firmware_missing".to_owned(),
        message: "가상 머신 시작에 필요한 펌웨어를 찾을 수 없습니다.".to_owned(),
        next_action: Some("앱을 다시 설치하십시오.".to_owned()),
    }
}

/// Returns an issue when per-operating-system metadata cannot be read or saved.
pub fn guest_storage_unavailable() -> AppIssue {
    AppIssue {
        code: "operating_system_storage_unavailable".to_owned(),
        message: "운영체제 정보를 저장하지 못했습니다.".to_owned(),
        next_action: Some(
            "저장 공간과 앱의 파일 접근 권한을 확인한 뒤 다시 시도하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when native desktop integration rejects a trusted destination.
pub fn desktop_unavailable() -> AppIssue {
    AppIssue {
        code: "desktop_open_failed".to_owned(),
        message: "요청한 페이지나 폴더를 열지 못했습니다.".to_owned(),
        next_action: Some("Windows 기본 앱 설정을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when clipboard access fails.
pub fn clipboard_unavailable() -> AppIssue {
    AppIssue {
        code: "clipboard_unavailable".to_owned(),
        message: "클립보드에 복사하지 못했습니다.".to_owned(),
        next_action: Some(
            "다른 앱이 클립보드를 사용 중인지 확인한 뒤 다시 시도하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when a snapshot-owned value is not available yet.
pub fn value_unknown() -> AppIssue {
    AppIssue {
        code: "value_unknown".to_owned(),
        message: "복사할 값을 아직 확인하지 못했습니다.".to_owned(),
        next_action: Some("운영체제 부팅이 끝난 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when the GSF ID has not been probed.
pub fn device_id_unknown() -> AppIssue {
    AppIssue {
        code: "device_id_unknown".to_owned(),
        message: "기기 ID를 아직 확인하지 못했습니다.".to_owned(),
        next_action: Some("운영체제 부팅이 끝난 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when the separate native window cannot be activated.
pub fn window_unavailable() -> AppIssue {
    AppIssue {
        code: "operating_system_window_unavailable".to_owned(),
        message: "운영체제 창을 앞으로 가져오지 못했습니다.".to_owned(),
        next_action: Some("작업 표시줄에서 운영체제 창을 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when another long-running native operation is active.
pub fn operation_in_progress() -> AppIssue {
    AppIssue {
        code: "operation_in_progress".to_owned(),
        message: "앱이 다른 작업을 처리하고 있어 요청을 시작하지 않았습니다.".to_owned(),
        next_action: Some("현재 작업이 끝난 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue after a user-requested cancellation.
pub fn operation_cancelled() -> AppIssue {
    AppIssue {
        code: "operation_cancelled".to_owned(),
        message: "요청한 작업을 취소했습니다.".to_owned(),
        next_action: None,
    }
}

/// Returns an issue when a native worker thread cannot start.
pub fn worker_unavailable() -> AppIssue {
    AppIssue {
        code: "app_worker_unavailable".to_owned(),
        message: "요청한 작업을 시작하지 못했습니다.".to_owned(),
        next_action: Some("앱을 종료한 뒤 다시 실행하십시오.".to_owned()),
    }
}

/// Returns an issue when package paths or archives fail validation.
pub fn app_package_invalid() -> AppIssue {
    AppIssue {
        code: "app_package_invalid".to_owned(),
        message: "선택한 앱 파일을 열지 못해 설치하지 않았습니다.".to_owned(),
        next_action: Some("APK, XAPK 또는 APKS 파일을 다시 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when the shared folder contains no transferable files.
pub fn shared_folder_empty() -> AppIssue {
    AppIssue {
        code: "shared_folder_empty".to_owned(),
        message: "공유 폴더에 보낼 파일이 없습니다.".to_owned(),
        next_action: Some("설정의 공유 폴더에 파일을 넣은 뒤 다시 누르십시오.".to_owned()),
    }
}

/// Returns an issue when one shared-folder file cannot be sent to the operating system.
pub fn shared_push_failed() -> AppIssue {
    AppIssue {
        code: "shared_push_failed".to_owned(),
        message: "파일을 가상 머신으로 보내지 못했습니다.".to_owned(),
        next_action: Some(
            "운영체제가 실행 중인지 확인한 뒤 다시 시도하십시오. 자세한 내용은 로그 폴더에 있습니다.".to_owned(),
        ),
    }
}

/// Returns an issue when adb rejects package installation.
pub fn app_install_failed() -> AppIssue {
    AppIssue {
        code: "app_install_failed".to_owned(),
        message: "앱을 운영체제에 설치하지 못했습니다.".to_owned(),
        next_action: Some("앱 파일과 저장 공간을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when installed packages cannot be listed.
pub fn app_list_unavailable() -> AppIssue {
    AppIssue {
        code: "app_list_unavailable".to_owned(),
        message: "설치된 앱 목록을 가져오지 못했습니다.".to_owned(),
        next_action: Some("운영체제 부팅이 끝난 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when one package cannot be removed.
pub fn app_uninstall_failed() -> AppIssue {
    AppIssue {
        code: "app_uninstall_failed".to_owned(),
        message: "앱을 운영체제에서 제거하지 못했습니다.".to_owned(),
        next_action: Some("운영체제 연결을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when one package cannot be launched.
pub fn app_launch_failed() -> AppIssue {
    AppIssue {
        code: "app_launch_failed".to_owned(),
        message: "앱을 운영체제에서 실행하지 못했습니다.".to_owned(),
        next_action: Some("앱이 설치되어 있는지 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when the trusted artifact store was not constructed.
pub fn artifact_store_unavailable() -> AppIssue {
    AppIssue {
        code: "artifact_store_unavailable".to_owned(),
        message: "운영체제 이미지 저장소를 준비하지 못했습니다.".to_owned(),
        next_action: Some("앱을 다시 설치한 뒤 다시 여십시오.".to_owned()),
    }
}

/// Returns an issue when an image transfer or digest verification fails.
pub fn artifact_download_failed() -> AppIssue {
    AppIssue {
        code: "artifact_download_failed".to_owned(),
        message: "운영체제 이미지를 다운로드하거나 검증하지 못했습니다.".to_owned(),
        next_action: Some("네트워크 연결을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when guest creation is requested before artifact verification.
pub fn artifact_not_verified() -> AppIssue {
    AppIssue {
        code: "artifact_not_verified".to_owned(),
        message: "운영체제 이미지의 무결성 확인이 끝나지 않아 설치하지 않았습니다.".to_owned(),
        next_action: Some("이미지 다운로드와 무결성 확인을 마친 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when the requested disk size is not one of the supported choices.
pub fn invalid_disk_size() -> AppIssue {
    AppIssue {
        code: "invalid_disk_size".to_owned(),
        message: "디스크 크기가 허용 범위를 벗어나 만들지 않았습니다.".to_owned(),
        next_action: Some("32, 64 또는 128 GiB를 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when installer boot files cannot be prepared from the verified image.
pub fn install_prepare_failed() -> AppIssue {
    AppIssue {
        code: "install_prepare_failed".to_owned(),
        message:
            "설치 파일을 준비하지 못했습니다. 이미지 파일을 다시 내려받은 뒤 다시 시도해 주십시오."
                .to_owned(),
        next_action: None,
    }
}

/// Returns an issue when qemu-img cannot create the virtual disk.
pub fn guest_create_failed() -> AppIssue {
    AppIssue {
        code: "operating_system_create_failed".to_owned(),
        message: "운영체제 디스크를 만들지 못했습니다.".to_owned(),
        next_action: Some("저장 공간을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when the user declines elevation.
pub fn whpx_enable_declined() -> AppIssue {
    AppIssue {
        code: "hypervisor_enable_declined".to_owned(),
        message: "관리자 권한 요청을 취소해 하이퍼바이저를 활성화하지 않았습니다.".to_owned(),
        next_action: Some(
            "활성화하려면 버튼을 다시 누르고 관리자 권한 요청을 승인하십시오.".to_owned(),
        ),
    }
}

/// Returns an issue when the fixed setup helper cannot start.
pub fn whpx_enable_unavailable() -> AppIssue {
    AppIssue {
        code: "hypervisor_enable_unavailable".to_owned(),
        message: "하이퍼바이저 활성화를 시작하지 못했습니다.".to_owned(),
        next_action: Some("앱을 다시 설치한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when the fixed setup helper exits unsuccessfully.
pub fn whpx_enable_failed(code: u32) -> AppIssue {
    AppIssue {
        code: "hypervisor_enable_failed".to_owned(),
        message: format!(
            "하이퍼바이저 활성화가 끝나지 않았습니다. 종료 코드 {code}를 로그에 기록했습니다."
        ),
        next_action: Some("Windows 기능 상태를 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when local diagnostic collection fails.
pub fn diagnostics_failed() -> AppIssue {
    AppIssue {
        code: "diagnostics_export_failed".to_owned(),
        message: "진단 묶음을 만들지 못했습니다.".to_owned(),
        next_action: Some("저장 공간과 폴더 접근 권한을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when release metadata cannot be read or trusted.
pub fn update_check_failed() -> AppIssue {
    AppIssue {
        code: "update_check_failed".to_owned(),
        message: "업데이트 정보를 확인하지 못했습니다.".to_owned(),
        next_action: Some("네트워크 연결을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Internal control result used to project an up-to-date state.
pub fn update_up_to_date() -> AppIssue {
    AppIssue {
        code: "update_up_to_date".to_owned(),
        message: "현재 버전이 최신입니다.".to_owned(),
        next_action: None,
    }
}

/// Returns an issue when no checked update is available.
pub fn update_not_available() -> AppIssue {
    AppIssue {
        code: "update_not_available".to_owned(),
        message: "설치할 업데이트가 없습니다.".to_owned(),
        next_action: Some("업데이트 확인을 먼저 실행하십시오.".to_owned()),
    }
}

/// Returns an issue when an update has no trusted sibling checksum.
pub fn update_unverified() -> AppIssue {
    AppIssue {
        code: "update_unverified".to_owned(),
        message: "업데이트 파일의 검증 정보를 찾지 못해 다운로드하지 않았습니다.".to_owned(),
        next_action: Some("검증 정보가 포함된 새 릴리스를 기다리십시오.".to_owned()),
    }
}

/// Returns an issue when an update download or digest check fails.
pub fn update_download_failed() -> AppIssue {
    AppIssue {
        code: "update_download_failed".to_owned(),
        message: "업데이트를 다운로드하거나 검증하지 못했습니다.".to_owned(),
        next_action: Some("네트워크 연결을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when the verified installer cannot launch.
pub fn update_launch_failed() -> AppIssue {
    AppIssue {
        code: "update_launch_failed".to_owned(),
        message: "검증한 업데이트 설치 파일을 실행하지 못했습니다.".to_owned(),
        next_action: Some("Windows 보안 설정을 확인한 뒤 다시 시도하십시오.".to_owned()),
    }
}
