// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useEffect, useState } from 'react';
import { formatDuration } from '../format';

export interface BootActivityProps {
  /** When the virtual machine started (ISO 8601), or null before it has. */
  readonly startedAt: string | null;
}

function elapsedSeconds(startedAt: string | null, now: number): number | null {
  if (startedAt === null) return null;
  const started = Date.parse(startedAt);
  return Number.isNaN(started) ? null : Math.max(0, (now - started) / 1000);
}

/**
 * What the dark stage shows while the operating system boots behind it. The guest window stays
 * hidden until Android reports boot completion, so firmware and console text never appear; the
 * turning ring and the counter say the machine is working. The counter keeps ticking when the
 * ring stops for reduced motion.
 */
export function BootActivity({ startedAt }: BootActivityProps) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => { setNow(Date.now()); }, 1000);
    return () => { window.clearInterval(timer); };
  }, []);
  const elapsed = elapsedSeconds(startedAt, now);
  return (
    <div className="ome-boot-activity" role="status" aria-live="polite">
      <span className="ome-boot-activity-ring" aria-hidden="true" />
      <p className="ome-boot-activity-title">운영체제를 시작하고 있습니다</p>
      {elapsed !== null && (
        <p className="ome-boot-activity-elapsed" aria-live="off">{`시작한 지 ${formatDuration(elapsed)}`}</p>
      )}
    </div>
  );
}
