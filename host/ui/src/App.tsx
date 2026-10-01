// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Root: reading, blocked (S8), first-run wizard (S1), or the rail shell. Which one is Rust's call,
// expressed through `blocker` and `phase` in the snapshot. The product's own title bar sits above all
// of them, because the main window has no native frame.
import type { ControllerBridge } from './contracts';
import { IssueNotice, TitleBar } from './components';
import { BlockedScreen } from './screens/BlockedScreen';
import { Shell } from './screens/Shell';
import { WizardScreen } from './screens/WizardScreen';
import { useController } from './use-controller';

export interface AppProps {
  readonly bridge: ControllerBridge;
  /** Fixed title bar maximized state, for the QA gallery only. */
  readonly titleBarMaximized?: boolean | undefined;
}

export function App({ bridge, titleBarMaximized }: AppProps) {
  const { snapshot, issue, actions } = useController(bridge);

  if (snapshot === null) {
    return (
      <div className="ome-app">
        <TitleBar maximized={titleBarMaximized} />
        <div className="ome-app-loading">
          <p className="ome-loading" role="status">상태를 읽는 중입니다.</p>
          {issue !== null && <IssueNotice issue={issue} />}
        </div>
      </div>
    );
  }

  // A failed command keeps the last snapshot; its issue is shown through the same field.
  const view = snapshot.issue === issue ? snapshot : { ...snapshot, issue };

  return (
    <div className="ome-app">
      <TitleBar maximized={titleBarMaximized} />
      {view.blocker !== null ? <BlockedScreen snapshot={view} actions={actions} />
        : view.phase === 'wizard' ? <WizardScreen snapshot={view} actions={actions} />
          : <Shell snapshot={view} actions={actions} />}
    </div>
  );
}
