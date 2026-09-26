// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use crate::AppIssue;

/// Returns the stable placeholder issue for work whose owning adapter has not been connected.
pub fn not_wired() -> AppIssue {
    AppIssue {
        code: "not_wired".to_owned(),
        message: "아직 준비되지 않은 기능입니다.".to_owned(),
        next_action: None,
    }
}

/// Returns an issue for invalid settings input while preserving the previous settings.
pub fn invalid_settings() -> AppIssue {
    AppIssue {
        code: "invalid_settings".to_owned(),
        message: "설정값이 허용 범위를 벗어나 저장하지 않았습니다.".to_owned(),
        next_action: Some("메모리는 4096~16384MB, 프로세서 수는 2~8로 지정하십시오.".to_owned()),
    }
}

/// Returns an issue for settings storage failures while preserving the previous settings.
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

/// Returns an issue when a requested keymap profile does not exist.
pub fn keymap_not_found() -> AppIssue {
    AppIssue {
        code: "keymap_not_found".to_owned(),
        message: "선택한 키 매핑을 찾지 못해 설정을 바꾸지 않았습니다.".to_owned(),
        next_action: Some("키 매핑 목록을 새로 확인한 뒤 다시 선택하십시오.".to_owned()),
    }
}

/// Returns an issue when a bundled keymap cannot be deleted.
pub fn keymap_bundled() -> AppIssue {
    AppIssue {
        code: "keymap_bundled".to_owned(),
        message: "앱에 포함된 키 매핑은 삭제할 수 없습니다.".to_owned(),
        next_action: Some(
            "사용하지 않으려면 키 매핑을 끄거나 다른 항목을 선택하십시오.".to_owned(),
        ),
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

/// Returns an issue when applying a display preset requires an unavailable guest connection.
pub fn guest_connection_unavailable() -> AppIssue {
    AppIssue {
        code: "guest_connection_unavailable".to_owned(),
        message: "게스트에 연결되지 않아 화면 크기를 적용하지 못했습니다.".to_owned(),
        next_action: Some("게스트를 시작하고 부팅이 끝난 뒤 다시 시도하십시오.".to_owned()),
    }
}

/// Returns an issue when wizard progression is not allowed in its current state.
pub fn wizard_cannot_continue() -> AppIssue {
    AppIssue {
        code: "wizard_cannot_continue".to_owned(),
        message: "현재 단계의 작업이 끝나지 않아 다음 단계로 넘어가지 않았습니다.".to_owned(),
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
