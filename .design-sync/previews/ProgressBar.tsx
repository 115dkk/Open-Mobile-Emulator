// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { ProgressBar } from 'open-mobile-emulator';

/** A download at 62 percent. */
export function Transferring() {
  return (
    <div style={{ width: 600 }}>
      <ProgressBar value={0.62} label="다운로드" />
    </div>
  );
}

/** Unknown total: an empty track, never a guess. */
export function Unknown() {
  return (
    <div style={{ width: 600 }}>
      <ProgressBar value={null} label="설치" />
    </div>
  );
}

/** Thin variant inside a row (app installation). */
export function Thin() {
  return (
    <div style={{ width: 360 }}>
      <ProgressBar value={0.41} label="설치" size="thin" />
    </div>
  );
}

/** Complete. */
export function Complete() {
  return (
    <div style={{ width: 600 }}>
      <ProgressBar value={1} label="다운로드" />
    </div>
  );
}
