// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { ReactNode } from 'react';

export interface ScreenHeaderProps {
  readonly title: string;
  /** Actions on the right (search, buttons). */
  readonly children?: ReactNode;
}

/** Title row of a rail screen (S3, S5, S6 layout: 24px top, 32px sides). */
export function ScreenHeader({ title, children }: ScreenHeaderProps) {
  return (
    <header className="ome-screen-header">
      <h1 className="ome-screen-title">{title}</h1>
      {children !== undefined && <div className="ome-screen-header-actions">{children}</div>}
    </header>
  );
}
