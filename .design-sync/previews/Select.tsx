// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Select } from 'open-mobile-emulator';

/** Native select in the product's control frame. */
export function Basic() {
  return (
    <div style={{ width: 200 }}>
      <Select
        label="창을 닫을 때"
        value="stop"
        onChange={() => {}}
        options={[
          { value: 'stop', label: '운영체제 끄기' },
          { value: 'tray', label: '트레이로 내리기' },
        ]}
      />
    </div>
  );
}

/** Choosing the operating system to start. */
export function Guests() {
  return (
    <div style={{ width: 200 }}>
      <Select
        label="시작할 운영체제"
        value="android-13"
        onChange={() => {}}
        options={[
          { value: 'android-13', label: 'android-13' },
          { value: 'android-15', label: 'android-15' },
        ]}
      />
    </div>
  );
}

/** Disabled. */
export function Disabled() {
  return (
    <div style={{ width: 200 }}>
      <Select label="창을 닫을 때" value="stop" disabled onChange={() => {}} options={[{ value: 'stop', label: '운영체제 끄기' }]} />
    </div>
  );
}
