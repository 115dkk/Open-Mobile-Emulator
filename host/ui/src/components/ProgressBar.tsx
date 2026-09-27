// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

export interface ProgressBarProps {
  /** Ratio 0..1, or null when the total is unknown. An unknown total draws an empty track, never a guess. */
  readonly value: number | null;
  readonly label: string;
  readonly size?: 'default' | 'thin' | undefined;
}

export function ProgressBar({ value, label, size = 'default' }: ProgressBarProps) {
  const ratio = value === null ? null : Math.min(1, Math.max(0, value));
  return (
    <div
      className={size === 'thin' ? 'ome-progress ome-progress-thin' : 'ome-progress'}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={ratio === null ? undefined : Math.round(ratio * 1000) / 10}
    >
      {ratio !== null && <div className="ome-progress-fill" style={{ width: `${String(ratio * 100)}%` }} />}
    </div>
  );
}
