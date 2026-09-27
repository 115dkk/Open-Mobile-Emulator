// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Toggle } from 'open-mobile-emulator';

const row = { display: 'flex', alignItems: 'center', gap: 12 };
const caption = { font: 'var(--type-caption)', color: 'var(--muted)', width: 96 };

/** On and off. */
export function States() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <span style={row}><span style={caption}>켬</span><Toggle checked onChange={() => {}} label="게임별 자동 적용" /></span>
      <span style={row}><span style={caption}>끔</span><Toggle checked={false} onChange={() => {}} label="fps 표시" /></span>
    </div>
  );
}

/** Disabled in both positions (a setting the installed image does not support). */
export function Disabled() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <span style={row}><span style={caption}>켬, 비활성</span><Toggle checked disabled onChange={() => {}} label="루트 권한" /></span>
      <span style={row}><span style={caption}>끔, 비활성</span><Toggle checked={false} disabled onChange={() => {}} label="루트 권한" /></span>
    </div>
  );
}
