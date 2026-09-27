// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { StageFrame } from 'open-mobile-emulator';

/** The darkest area of the stage before a window covers it. */
export function Empty() {
  return (
    <div style={{ width: 640, height: 360, display: 'flex' }}>
      <StageFrame onRect={() => {}}>
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--muted)' }}>
          운영체제 창이 여기에 놓입니다.
        </div>
      </StageFrame>
    </div>
  );
}
