// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Slider } from 'open-mobile-emulator';

/** Memory: the readout next to the track shows the unit. */
export function Memory() {
  return (
    <div style={{ width: 320 }}>
      <Slider label="메모리" min={4} max={60} value={8} format={(v) => `${v} GiB`} onChange={() => {}} />
    </div>
  );
}

/** Processor cores. */
export function Cores() {
  return (
    <div style={{ width: 320 }}>
      <Slider label="프로세서 코어" min={2} max={16} value={4} format={(v) => `${v}개`} onChange={() => {}} />
    </div>
  );
}

/** Disabled. */
export function Disabled() {
  return (
    <div style={{ width: 320 }}>
      <Slider label="메모리" min={4} max={60} value={8} disabled format={(v) => `${v} GiB`} onChange={() => {}} />
    </div>
  );
}
