// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Rail } from 'open-mobile-emulator';

const frame = { height: 420, display: 'flex' };

/** The stage selected while the operating system runs (green dot). */
export function Running() {
  return (
    <div style={frame}>
      <Rail current="stage" onSelect={() => {}} guestState="running" version="0.1.0" />
    </div>
  );
}

/** Settings selected, operating system stopped (dimmed dot). */
export function Stopped() {
  return (
    <div style={frame}>
      <Rail current="settings" onSelect={() => {}} guestState="stopped" version="0.1.0" />
    </div>
  );
}

/** Starting: the dot pulses while work is in progress. */
export function Starting() {
  return (
    <div style={frame}>
      <Rail current="stage" onSelect={() => {}} guestState="starting" version="0.1.0" />
    </div>
  );
}

/** Failed: red dot. */
export function Failed() {
  return (
    <div style={frame}>
      <Rail current="stage" onSelect={() => {}} guestState="failed" version="0.1.0" />
    </div>
  );
}
