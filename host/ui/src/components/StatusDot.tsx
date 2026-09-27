// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { StatusTone } from '../presentation';

export interface StatusDotProps {
  /** `muted` draws the same dot dimmed (stopped, nothing pending). */
  readonly tone: StatusTone;
  /** Soft halo pulse for work in progress. Stops under reduced motion; the text says the same. */
  readonly pulse?: boolean | undefined;
}

/** A filled dot with a light halo. Decorative: the words beside it carry the state. */
export function StatusDot({ tone, pulse = false }: StatusDotProps) {
  const className = pulse ? `ome-status-dot ome-status-dot-${tone} ome-status-dot-pulse` : `ome-status-dot ome-status-dot-${tone}`;
  return <span className={className} aria-hidden="true" />;
}
