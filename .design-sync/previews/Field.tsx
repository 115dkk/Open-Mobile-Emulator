// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Field } from 'open-mobile-emulator';

/** A plain text field with a visible label. */
export function Text() {
  return (
    <div style={{ width: 320 }}>
      <Field label="프로필 이름" value="내 프로필 1" onChange={() => {}} />
    </div>
  );
}

/** Number input with a unit suffix and the allowed range as the hint. */
export function NumberWithHint() {
  return (
    <div style={{ width: 200 }}>
      <Field label="폭" type="number" value="1920" suffix="px" min={640} max={7680} hint="640~7680" onChange={() => {}} />
    </div>
  );
}

/** Search box: the label is hidden but stays the accessible name; the icon leads. */
export function Search() {
  return (
    <div style={{ width: 320 }}>
      <Field label="앱 찾기" labelHidden type="search" icon="search" placeholder="이름이나 패키지로 찾기" value="" onChange={() => {}} />
    </div>
  );
}

/** Invalid value: red frame, the hint says what is allowed. */
export function Invalid() {
  return (
    <div style={{ width: 200 }}>
      <Field label="DPI" type="number" value="60" suffix="DPI" invalid hint="120~640 사이의 값을 넣으십시오." onChange={() => {}} />
    </div>
  );
}

/** Disabled. */
export function Disabled() {
  return (
    <div style={{ width: 320 }}>
      <Field label="adb 주소" value="127.0.0.1:5555" disabled onChange={() => {}} />
    </div>
  );
}
