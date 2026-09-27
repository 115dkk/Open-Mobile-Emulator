// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Select, SettingRow, Toggle } from 'open-mobile-emulator';

/** Label, help sentence and a toggle in the 320px control column. */
export function WithToggle() {
  return (
    <div style={{ width: 816 }}>
      <SettingRow
        label="다른 PC에서 연결 허용"
        help="다시 시작해야 적용됩니다."
      >
        <Toggle checked={false} onChange={() => {}} label="다른 PC에서 연결 허용" />
      </SettingRow>
    </div>
  );
}

/** A select in the control column, with a two-line help sentence on the left. */
export function WithSelect() {
  return (
    <div style={{ width: 816 }}>
      <SettingRow
        label="창을 닫을 때"
        help="운영체제가 실행 중일 때 적용됩니다."
      >
        <Select
          label="창을 닫을 때"
          value="stop"
          onChange={() => {}}
          options={[
            { value: 'stop', label: '운영체제 끄기' },
            { value: 'tray', label: '트레이로 내리기' },
          ]}
        />
      </SettingRow>
    </div>
  );
}

/** Full-width content under the row: a path in monospace. */
export function WithBelow() {
  return (
    <div style={{ width: 816 }}>
      <SettingRow
        label="경로"
        help="운영체제 디스크와 로그, 스크린샷이 있는 폴더입니다."
        below={<code style={{ font: 'var(--type-mono)' }}>C:\Users\사용자\AppData\Local\OpenMobileEmulator</code>}
      >
        <span style={{ color: 'var(--muted)' }}>사용 중 38.2 GB</span>
      </SettingRow>
    </div>
  );
}

/** Several rows in a row, the way a settings section stacks them. */
export function Stacked() {
  return (
    <div style={{ width: 816 }}>
      <SettingRow label="매핑 표지 기본 표시">
        <Toggle checked onChange={() => {}} label="매핑 표지 기본 표시" />
      </SettingRow>
      <SettingRow label="게임별 자동 적용">
        <Toggle checked onChange={() => {}} label="게임별 자동 적용" />
      </SettingRow>
      <SettingRow label="fps 표시">
        <Toggle checked={false} onChange={() => {}} label="fps 표시" />
      </SettingRow>
    </div>
  );
}
