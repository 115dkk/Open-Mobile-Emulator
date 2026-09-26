// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { AppIssue, AppPhase, ControllerBridge, GuestState } from './contracts';
import { useController } from './use-controller';

const PHASE_CAPTION: Record<AppPhase, string> = {
  wizard: '처음 설정',
  main: '무대',
};

const NEXT_ACTION: Record<GuestState, string> = {
  stopped: '시작할 수 있습니다.',
  starting: '가상 머신을 시작하는 중입니다.',
  running: '게스트가 실행 중입니다.',
  stopping: '게스트를 끄는 중입니다.',
  restarting: '게스트를 다시 시작하는 중입니다.',
  failed: '게스트가 예기치 않게 종료되었습니다.',
};

function canStart(state: GuestState): boolean {
  return state === 'stopped' || state === 'failed';
}

function AppHeader({ phase }: { readonly phase: AppPhase | null }) {
  return (
    <header className="app-header">
      <h1 className="app-title">Open Mobile Emulator</h1>
      {phase !== null && <p className="app-phase">{PHASE_CAPTION[phase]}</p>}
    </header>
  );
}

function AppNotice({ issue }: { readonly issue: AppIssue }) {
  return (
    <div className="app-notice" role="alert">
      <p className="app-notice-message">{issue.message}</p>
      {issue.nextAction !== null && <p className="app-notice-next">{issue.nextAction}</p>}
    </div>
  );
}

export function App({ bridge }: { readonly bridge: ControllerBridge }) {
  const { snapshot, issue, hostCheckRefresh, guestStart } = useController(bridge);

  if (snapshot === null) {
    return (
      <main className="app-page">
        <AppHeader phase={null} />
        <p className="app-loading" role="status">상태를 읽는 중입니다.</p>
        {issue !== null && <AppNotice issue={issue} />}
      </main>
    );
  }

  const state = snapshot.guest.state;

  return (
    <main className="app-page">
      <AppHeader phase={snapshot.phase} />
      <div className="app-status" role="status">
        <span className={`app-status-dot app-status-dot-${state}`} aria-hidden="true" />
        <span className="app-status-state">{state}</span>
        <span className="app-status-next">{NEXT_ACTION[state]}</span>
      </div>
      {issue !== null && <AppNotice issue={issue} />}
      <div className="app-actions">
        <button type="button" className="app-button app-button-secondary" onClick={() => { void hostCheckRefresh(); }}>
          다시 확인
        </button>
        <button
          type="button"
          className="app-button app-button-primary"
          disabled={!canStart(state)}
          onClick={() => { void guestStart(); }}
        >
          시작
        </button>
      </div>
    </main>
  );
}
