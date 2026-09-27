// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Button, EmptyState } from 'open-mobile-emulator';

/** The apps screen with nothing installed: title, one sentence, one action. */
export function Apps() {
  return (
    <div style={{ width: 640 }}>
      <EmptyState
        icon="apps"
        title="설치된 앱이 없습니다."
        action={<Button variant="primary" icon="plus">설치</Button>}
      >
        APK 파일을 창에 놓거나 설치 버튼을 누르십시오.
      </EmptyState>
    </div>
  );
}

/** Title only, no icon and no action. */
export function TitleOnly() {
  return (
    <div style={{ width: 640 }}>
      <EmptyState title="입력 프로필이 없습니다." />
    </div>
  );
}
