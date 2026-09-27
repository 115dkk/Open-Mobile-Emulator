// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { StepList } from 'open-mobile-emulator';

/** First-boot checklist: done, active, pending, with a detail line under the active step. */
export function Vertical() {
  return (
    <div style={{ width: 320 }}>
      <StepList
        label="첫 부팅"
        items={[
          { id: 'vm', label: '가상 머신 시작', state: 'done' },
          { id: 'display', label: '화면 연결', state: 'done' },
          { id: 'boot', label: '운영체제 부팅', state: 'active', children: '약 30초 걸립니다.' },
          { id: 'probe', label: '기능 확인', state: 'pending' },
        ]}
      />
    </div>
  );
}

/** A failed step: the symbol carries the state, the words stay a noun phrase. */
export function Failed() {
  return (
    <div style={{ width: 320 }}>
      <StepList
        label="첫 부팅"
        items={[
          { id: 'vm', label: '가상 머신 시작', state: 'done' },
          { id: 'display', label: '화면 연결', state: 'done' },
          { id: 'boot', label: '운영체제 부팅', state: 'failed', children: '180초 안에 부팅 완료 신호가 오지 않았습니다.' },
          { id: 'probe', label: '기능 확인', state: 'pending' },
        ]}
      />
    </div>
  );
}

/** Horizontal form joins the steps with arrows (download progress). */
export function Horizontal() {
  return (
    <StepList
      label="진행"
      orientation="horizontal"
      items={[
        { id: 'transfer', label: '다운로드 중', state: 'active' },
        { id: 'verify', label: '무결성 확인', state: 'pending' },
        { id: 'done', label: '완료', state: 'pending' },
      ]}
    />
  );
}
