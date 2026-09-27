// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Markers appear and disappear with a 120 ms opacity change and nothing else (DESIGN.md 8). Appearing
// is a CSS animation; disappearing needs the marker to stay drawn, faded, for that long. This hook keeps
// a removed binding in place, marked as leaving, until the fade is over.
import { useEffect, useState } from 'react';
import type { Binding } from '../contracts';

/** `--motion-fast`. */
export const MARKER_FADE_MS = 120;

export interface Present {
  readonly key: string;
  readonly binding: Binding;
  readonly leaving: boolean;
}

function entries(profileId: string, bindings: readonly Binding[]): Present[] {
  return bindings.map((binding) => ({ key: `${profileId}/${binding.id}`, binding, leaving: false }));
}

interface Source {
  readonly profileId: string;
  readonly bindings: readonly Binding[];
  readonly shown: readonly Present[];
}

/**
 * The profile's bindings plus the ones that just left, each keyed by profile and binding id so a
 * profile switch fades the old markers out and the new ones in. Order is kept, so a leaving marker
 * stays the same DOM node and its opacity transition runs.
 */
export function usePresence(profileId: string, bindings: readonly Binding[]): readonly Present[] {
  const [source, setSource] = useState<Source>(() => ({ profileId, bindings, shown: entries(profileId, bindings) }));
  if (source.profileId !== profileId || source.bindings !== bindings) {
    const incoming = new Map(entries(profileId, bindings).map((entry) => [entry.key, entry]));
    const shown: Present[] = [];
    for (const entry of source.shown) {
      const next = incoming.get(entry.key);
      if (next !== undefined) {
        shown.push(next);
        incoming.delete(entry.key);
      } else {
        shown.push({ ...entry, leaving: true });
      }
    }
    shown.push(...incoming.values());
    setSource({ profileId, bindings, shown });
  }

  const leaving = source.shown.some((entry) => entry.leaving);
  useEffect(() => {
    if (!leaving) return undefined;
    const timer = window.setTimeout(() => {
      setSource((current) => ({ ...current, shown: current.shown.filter((entry) => !entry.leaving) }));
    }, MARKER_FADE_MS);
    return () => { window.clearTimeout(timer); };
  }, [leaving, source]);

  return source.shown;
}
