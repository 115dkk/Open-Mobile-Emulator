// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Chip } from 'open-mobile-emulator';

/** The five tones. Status colors are for real states only; `muted` is the default. */
export function Tones() {
  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
      <Chip>자동 적용</Chip>
      <Chip tone="accent">현재</Chip>
      <Chip tone="success">검증됨</Chip>
      <Chip tone="warning">검증 전</Chip>
      <Chip tone="danger">지원 종료</Chip>
    </div>
  );
}

/** With a leading 14px icon. */
export function WithIcon() {
  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
      <Chip tone="success" icon="check">검증됨</Chip>
      <Chip tone="warning" icon="alert-triangle">검증 전</Chip>
      <Chip icon="smartphone">안드로이드 13</Chip>
    </div>
  );
}
