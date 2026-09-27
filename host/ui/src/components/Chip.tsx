// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { ReactNode } from 'react';
import type { StatusTone } from '../presentation';
import { Icon } from './Icon';
import type { IconName } from './Icon';

export interface ChipProps {
  readonly children: ReactNode;
  /** Status colors only for real states (DESIGN.md 2); `muted` for everything else. */
  readonly tone?: StatusTone | undefined;
  readonly icon?: IconName | undefined;
}

export function Chip({ children, tone = 'muted', icon }: ChipProps) {
  return (
    <span className={`ome-chip ome-chip-${tone}`}>
      {icon !== undefined && <Icon name={icon} size={14} strokeWidth={2} />}
      <span>{children}</span>
    </span>
  );
}
