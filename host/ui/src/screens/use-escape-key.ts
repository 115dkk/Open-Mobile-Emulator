// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useEffect, useEffectEvent } from 'react';

/** Calls `onEscape` when Esc is pressed anywhere in the window. `null` listens to nothing. */
export function useEscapeKey(onEscape: (() => void) | null): void {
  const active = onEscape !== null;
  const escape = useEffectEvent(() => { onEscape?.(); });
  useEffect(() => {
    if (!active) return undefined;
    const listener = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return;
      event.preventDefault();
      escape();
    };
    document.addEventListener('keydown', listener);
    return () => { document.removeEventListener('keydown', listener); };
  }, [active]);
}
