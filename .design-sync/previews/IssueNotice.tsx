// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { IssueNotice } from 'open-mobile-emulator';

/** What happened, then what to do. The raw error is never shown. */
export function WithNextAction() {
  return (
    <div style={{ width: 640 }}>
      <IssueNotice
        issue={{
          code: 'boot_timeout',
          message: '운영체제 부팅에 실패했습니다.',
          nextAction: '다시 시작을 시도할 수 있습니다. 반복될 경우 로그를 첨부해 문제를 보고하십시오.',
        }}
      />
    </div>
  );
}

/** Message only. */
export function MessageOnly() {
  return (
    <div style={{ width: 640 }}>
      <IssueNotice issue={{ code: 'app_busy', message: '앱이 다른 작업을 처리하는 중입니다.', nextAction: null }} />
    </div>
  );
}
