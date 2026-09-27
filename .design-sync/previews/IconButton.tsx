// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { IconButton } from 'open-mobile-emulator';

/** The stage toolbar: every icon-only button carries a label (tooltip and accessible name). */
export function Toolbar() {
  return (
    <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
      <IconButton icon="camera" label="스크린샷" />
      <IconButton icon="volume" label="볼륨" />
      <IconButton icon="pencil" label="매핑 편집" />
      <IconButton icon="refresh" label="다시 시작" />
      <IconButton icon="power" label="끄기" />
    </div>
  );
}

/** A toggle button in its pressed state (mapping badges shown). */
export function Pressed() {
  return (
    <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
      <IconButton icon="eye" label="매핑 표시" pressed />
      <IconButton icon="eye-off" label="매핑 숨김" pressed={false} />
    </div>
  );
}

/** Disabled. */
export function Disabled() {
  return <IconButton icon="camera" label="스크린샷" disabled />;
}
