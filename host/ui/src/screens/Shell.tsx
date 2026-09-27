// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Main window after the wizard: the rail and the selected screen. The selection is view state and
// never reaches Rust. The body is the page's <main>; screens render sections and divs inside it.
import { useState } from 'react';
import type { ScreenProps } from '../actions';
import { Rail } from '../components';
import type { RailItem } from '../components';
import { AppsScreen } from './AppsScreen';
import { DisplayScreen } from './DisplayScreen';
import { InputScreen } from './InputScreen';
import { SettingsScreen } from './SettingsScreen';
import { StageScreen } from './StageScreen';

function CurrentScreen({ item, snapshot, actions }: ScreenProps & { readonly item: RailItem }) {
  switch (item) {
    case 'stage': return <StageScreen snapshot={snapshot} actions={actions} />;
    case 'apps': return <AppsScreen snapshot={snapshot} actions={actions} />;
    case 'input': return <InputScreen snapshot={snapshot} actions={actions} />;
    case 'display': return <DisplayScreen snapshot={snapshot} actions={actions} />;
    case 'settings': return <SettingsScreen snapshot={snapshot} actions={actions} />;
  }
}

export function Shell({ snapshot, actions }: ScreenProps) {
  const [current, setCurrent] = useState<RailItem>('stage');
  return (
    <div className="ome-shell">
      <Rail current={current} onSelect={setCurrent} guestState={snapshot.guest.state} version={snapshot.productVersion} />
      <main className="ome-shell-body">
        <CurrentScreen item={current} snapshot={snapshot} actions={actions} />
      </main>
    </div>
  );
}
