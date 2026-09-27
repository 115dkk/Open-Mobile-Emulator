// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Button } from 'open-mobile-emulator';

/** The one main action of a view. */
export function Primary() {
  return <Button variant="primary">계속</Button>;
}

/** Every variant side by side: primary, secondary (default), ghost, danger, danger-solid. */
export function Variants() {
  return (
    <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
      <Button variant="primary">계속</Button>
      <Button>다시 확인</Button>
      <Button variant="ghost">나중에 하기</Button>
      <Button variant="danger">제거</Button>
      <Button variant="danger-solid">삭제</Button>
    </div>
  );
}

/** With a leading icon (18px, stroke 2). */
export function WithIcon() {
  return (
    <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
      <Button variant="primary" icon="download">다운로드</Button>
      <Button icon="refresh">다시 확인</Button>
      <Button icon="folder-open">폴더 열기</Button>
    </div>
  );
}

/** The 44px wizard main button next to the default 40px size. */
export function Large() {
  return (
    <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
      <Button variant="primary" size="large">활성화</Button>
      <Button size="large">지금은 건너뛰기</Button>
    </div>
  );
}

/** Disabled state of the primary and secondary variants. */
export function Disabled() {
  return (
    <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
      <Button variant="primary" disabled>다음</Button>
      <Button disabled>취소</Button>
    </div>
  );
}
