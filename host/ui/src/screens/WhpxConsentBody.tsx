// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// S1.2 consent content, shared by the wizard step and the blocked screen's 활성화 path (S8).
import { Icon } from '../components';

const POINTS = [
  '하이퍼바이저를 활성화합니다.',
  '활성화 시 관리자 권한이 필요합니다.',
  '활성화 후 다시 시작해야 할 수 있습니다. 작업을 미리 저장해두십시오.',
] as const;

export function WhpxConsentBody() {
  return (
    <>
      <h1 className="ome-page-title">하이퍼바이저 활성화</h1>
      <p className="ome-lead">Open Mobile Emulator를 사용하려면 하이퍼바이저를 활성화해야 합니다.</p>
      <ul className="ome-check-list">
        {POINTS.map((point) => (
          <li key={point}>
            <span className="ome-check-list-mark"><Icon name="check" size={16} /></span>
            <span>{point}</span>
          </li>
        ))}
      </ul>
      <div className="ome-caution">
        <span className="ome-caution-icon"><Icon name="alert-triangle" size={18} /></span>
        <p>활성화하면 같은 PC에서 커널 안티치트를 쓰는 게임이 실행되지 않을 수 있습니다.</p>
      </div>
    </>
  );
}

/** The exit line printed next to the leave button (DESIGN.md 7: the way out is written down). */
export const ESC_LINE = 'Esc 키로도 나갈 수 있습니다.';
