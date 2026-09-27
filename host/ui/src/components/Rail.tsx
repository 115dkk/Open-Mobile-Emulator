// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { GuestState } from '../contracts';
import { GUEST_STATE_LABEL, GUEST_STATE_TONE } from '../presentation';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { StatusDot } from './StatusDot';

export type RailItem = 'stage' | 'apps' | 'input' | 'display' | 'settings';

const ITEMS: readonly { readonly id: RailItem; readonly label: string; readonly icon: IconName }[] = [
  { id: 'stage', label: '무대', icon: 'stage' },
  { id: 'apps', label: '앱', icon: 'apps' },
  { id: 'input', label: '입력', icon: 'input' },
  { id: 'display', label: '표시', icon: 'display' },
  { id: 'settings', label: '설정', icon: 'settings' },
];

export interface RailProps {
  readonly current: RailItem;
  readonly onSelect: (item: RailItem) => void;
  readonly guestState: GuestState;
  readonly version: string;
}

/** 64px icon-and-label navigation with the guest status dot and product version at the bottom. */
export function Rail({ current, onSelect, guestState, version }: RailProps) {
  return (
    <nav className="ome-rail" aria-label="주 메뉴">
      <div className="ome-rail-items">
        {ITEMS.map((item) => {
          const selected = item.id === current;
          return (
            <button
              key={item.id}
              type="button"
              className="ome-rail-item"
              aria-current={selected ? 'page' : undefined}
              onClick={() => { onSelect(item.id); }}
            >
              <Icon name={item.icon} size={20} strokeWidth={selected ? 2 : 1.5} />
              <span className="ome-rail-label">{item.label}</span>
            </button>
          );
        })}
      </div>
      <div className="ome-rail-foot" title={GUEST_STATE_LABEL[guestState]}>
        <StatusDot tone={GUEST_STATE_TONE[guestState]} pulse={guestState === 'starting'} />
        <span className="ome-visually-hidden">{GUEST_STATE_LABEL[guestState]}</span>
        <span className="ome-rail-version">{version}</span>
      </div>
    </nav>
  );
}
