// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useEffect, useEffectEvent } from 'react';

/** Forwards stage keys; Rust owns admission, hotkeys and profile interpretation. */
export function useGuestKeyboard(active: boolean, send: (code: string, pressed: boolean) => Promise<void>): void {
  const forward = useEffectEvent((code: string, pressed: boolean) => send(code, pressed));
  useEffect(() => {
    if (!active) return undefined;
    const pressed = new Set<string>();
    // One event at a time, in order. Each forwarded event runs on its own native task, so a release
    // sent a millisecond after its press could otherwise overtake it: the guest would then hold the
    // key, and the next press of it would be dropped as a repeat.
    const queue: [string, boolean][] = [];
    let busy = false;
    const pump = (): void => {
      const next = queue.shift();
      if (next === undefined) { busy = false; return; }
      busy = true;
      void forward(next[0], next[1]).catch(() => undefined).then(pump);
    };
    const enqueue = (code: string, down: boolean) => { queue.push([code, down]); if (!busy) pump(); };
    const listener = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing || event.code === '') return;
      if (event.target instanceof Element && event.target.closest(
        '[role="dialog"], [aria-modal="true"], input, textarea, select, [contenteditable]',
      )) return;
      const down = event.type === 'keydown';
      if (down && event.repeat) return;
      event.preventDefault();
      if (down) pressed.add(event.code);
      else pressed.delete(event.code);
      enqueue(event.code, down);
    };
    const release = () => {
      for (const code of pressed) enqueue(code, false);
      pressed.clear();
    };
    const visibilityChanged = () => {
      if (document.visibilityState === 'hidden') release();
    };
    window.addEventListener('keydown', listener);
    window.addEventListener('keyup', listener);
    window.addEventListener('blur', release);
    document.addEventListener('visibilitychange', visibilityChanged);
    return () => {
      window.removeEventListener('keydown', listener);
      window.removeEventListener('keyup', listener);
      window.removeEventListener('blur', release);
      document.removeEventListener('visibilitychange', visibilityChanged);
      release();
    };
  }, [active]);
}
