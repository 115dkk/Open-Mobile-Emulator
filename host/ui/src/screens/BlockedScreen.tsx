// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// S8: the whole window when Rust reports a blocker. Cause, next action, one button.
import { useState } from 'react';
import type { ScreenActions, ScreenProps } from '../actions';
import type { AppIssue } from '../contracts';
import { Button, Icon, IssueNotice } from '../components';
import { useEscapeKey } from './use-escape-key';
import { ESC_LINE, WhpxConsentBody } from './WhpxConsentBody';

function ConsentView({ issue, actions, onLeave }: {
  readonly issue: AppIssue | null;
  readonly actions: ScreenActions;
  readonly onLeave: () => void;
}) {
  useEscapeKey(onLeave);
  return (
    <main className="ome-blocked ome-blocked-consent">
      <div className="ome-blocked-column ome-blocked-column-wide">
        {issue !== null && <IssueNotice issue={issue} />}
        <WhpxConsentBody />
        <div className="ome-blocked-actions ome-blocked-actions-end">
          <p className="ome-footer-note">{ESC_LINE}</p>
          <Button size="large" onClick={onLeave}>지금은 건너뛰기</Button>
          <Button size="large" variant="primary" onClick={actions.whpxEnable}>활성화</Button>
        </div>
      </div>
    </main>
  );
}

export function BlockedScreen({ snapshot, actions }: ScreenProps) {
  const [consent, setConsent] = useState(false);
  const kind = snapshot.blocker?.kind;
  if (kind === undefined) return null;

  if (kind === 'hypervisorPlatformOff' && consent) {
    return <ConsentView issue={snapshot.issue} actions={actions} onLeave={() => { setConsent(false); }} />;
  }

  return (
    <main className="ome-blocked">
      <div className="ome-blocked-column">
        {snapshot.issue !== null && <IssueNotice issue={snapshot.issue} />}
        <span className="ome-blocked-icon"><Icon name="alert-triangle" size={32} /></span>
        {kind === 'virtualizationOff' && (<>
          <h1 className="ome-page-title">이 PC의 BIOS 설정에서 가상화가 꺼져 있습니다.</h1>
          <p className="ome-lead">PC를 다시 시작해 BIOS 설정에서 가상화(Intel VT-x 또는 AMD-V)를 켠 뒤 이 앱을 다시 여십시오.</p>
          <div className="ome-blocked-actions">
            <Button size="large" icon="external-link" onClick={() => actions.openHelp('virtualizationBios')}>켜는 방법</Button>
            <Button size="large" variant="primary" icon="refresh" onClick={actions.hostCheckRefresh}>다시 확인</Button>
          </div>
          <p className="ome-caption">BIOS 메뉴 이름은 PC 제조사마다 다릅니다.</p>
        </>)}
        {kind === 'qemuMissing' && (<>
          <h1 className="ome-page-title">가상 머신 구성 요소를 찾을 수 없습니다.</h1>
          <p className="ome-lead">앱을 다시 설치하십시오.</p>
          <div className="ome-blocked-actions">
            <Button size="large" variant="primary" icon="refresh" onClick={actions.hostCheckRefresh}>다시 확인</Button>
          </div>
        </>)}
        {kind === 'hypervisorPlatformOff' && (<>
          <h1 className="ome-page-title">하이퍼바이저가 꺼져 있습니다.</h1>
          <div className="ome-blocked-actions">
            <Button size="large" variant="primary" onClick={() => { setConsent(true); }}>활성화</Button>
          </div>
        </>)}
      </div>
    </main>
  );
}
