// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Segmented } from 'open-mobile-emulator';

/** Refresh-rate picker with the default marked in its label. */
export function RefreshRate() {
  return (
    <Segmented
      label="주사율"
      value="120"
      onChange={() => {}}
      options={[
        { value: '60', label: '60 Hz (기본)' },
        { value: '75', label: '75 Hz' },
        { value: '90', label: '90 Hz' },
        { value: '120', label: '120 Hz' },
        { value: '144', label: '144 Hz' },
        { value: 'custom', label: '사용자 지정' },
      ]}
    />
  );
}

/** Three options, one selected. */
export function Vsync() {
  return (
    <Segmented
      label="수직 동기화"
      value="on"
      onChange={() => {}}
      options={[
        { value: 'off', label: '끔' },
        { value: 'on', label: '켬' },
        { value: 'adaptive', label: '적응형' },
      ]}
    />
  );
}

/** Nothing selected yet, and one option disabled. */
export function NoneSelected() {
  return (
    <Segmented
      label="배율"
      value={null}
      onChange={() => {}}
      options={[
        { value: 'fit', label: '창에 맞춤' },
        { value: 'one', label: '1:1' },
        { value: 'custom', label: '사용자 지정', disabled: true },
      ]}
    />
  );
}

/** The whole group disabled. */
export function Disabled() {
  return (
    <Segmented
      label="수직 동기화"
      value="on"
      disabled
      onChange={() => {}}
      options={[
        { value: 'off', label: '끔' },
        { value: 'on', label: '켬' },
        { value: 'adaptive', label: '적응형' },
      ]}
    />
  );
}
