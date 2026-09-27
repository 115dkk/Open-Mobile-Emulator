// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Dialog } from 'open-mobile-emulator';

/** Removing an app: the fact in one line, what is lost in the second, a red confirming button. */
export function RemoveApp() {
  return (
    <Dialog
      open
      title="샘플 앱 A를 제거합니다."
      message="앱 데이터도 함께 지워집니다."
      confirmLabel="제거"
      tone="danger"
      onConfirm={() => {}}
      onCancel={() => {}}
    />
  );
}

/** A default-tone confirmation with the primary button. */
export function Reinstall() {
  return (
    <Dialog
      open
      title="android-15를 다시 설치합니다."
      message="디스크를 지우고 같은 이미지로 처음부터 설치합니다."
      confirmLabel="다시 설치"
      onConfirm={() => {}}
      onCancel={() => {}}
    />
  );
}

/** Extra content between the text and the buttons. */
export function WithBody() {
  return (
    <Dialog
      open
      title="새 운영체제를 설치합니다."
      confirmLabel="설치"
      onConfirm={() => {}}
      onCancel={() => {}}
    >
      <p style={{ margin: 0, color: 'var(--muted)' }}>디스크 크기 64 GB · 안드로이드 13 · 2.26 GB 다운로드</p>
    </Dialog>
  );
}
