// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Button, Field, ScreenHeader } from 'open-mobile-emulator';

/** Title only, as on the display and settings screens. */
export function TitleOnly() {
  return (
    <div style={{ width: 900 }}>
      <ScreenHeader title="표시" />
    </div>
  );
}

/** The apps screen: search box and two buttons on the right. */
export function WithActions() {
  return (
    <div style={{ width: 900 }}>
      <ScreenHeader title="앱">
        <div style={{ width: 240 }}>
          <Field label="앱 찾기" labelHidden type="search" icon="search" placeholder="이름이나 패키지로 찾기" value="" onChange={() => {}} />
        </div>
        <Button icon="folder-open">스크린샷 폴더 열기</Button>
        <Button variant="primary" icon="plus">설치</Button>
      </ScreenHeader>
    </div>
  );
}
