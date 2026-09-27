// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { ReactNode } from 'react';
import { Icon } from './Icon';
import type { IconName } from './Icon';

export interface EmptyStateProps {
  readonly title: string;
  readonly icon?: IconName | undefined;
  /** One sentence at most. */
  readonly children?: ReactNode;
  /** A single button, if there is something to do. */
  readonly action?: ReactNode;
}

export function EmptyState({ title, icon, children, action }: EmptyStateProps) {
  return (
    <div className="ome-empty">
      {icon !== undefined && <span className="ome-empty-icon"><Icon name={icon} size={32} /></span>}
      <p className="ome-empty-title">{title}</p>
      {children !== undefined && <p className="ome-empty-body">{children}</p>}
      {action !== undefined && <div className="ome-empty-action">{action}</div>}
    </div>
  );
}
