// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { ReactNode } from 'react';

export interface SettingRowProps {
  readonly label: string;
  /** Help sentence: when it applies and what it costs (DESIGN.md 9). */
  readonly help?: ReactNode;
  /** The control, right-aligned in a fixed 320px column. */
  readonly children?: ReactNode;
  /** Full-width content under the row (a path, a warning box). */
  readonly below?: ReactNode;
}

/** Label and help on the left, control on the right, a line underneath. No cards (S6). */
export function SettingRow({ label, help, children, below }: SettingRowProps) {
  return (
    <div className="ome-setting">
      <div className="ome-setting-row">
        <div className="ome-setting-text">
          <span className="ome-setting-label">{label}</span>
          {help !== undefined && <p className="ome-setting-help">{help}</p>}
        </div>
        {children !== undefined && <div className="ome-setting-control">{children}</div>}
      </div>
      {below !== undefined && <div className="ome-setting-below">{below}</div>}
    </div>
  );
}
