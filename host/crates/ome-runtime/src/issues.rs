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
