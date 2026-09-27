// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Gallery only: the rail selection is view state inside Shell and never in a snapshot, so a variant
// of a rail screen other than the stage opens its screen by pressing the rail button once the shell
// has been drawn. Loaded only by qa-gallery.tsx; the product build rejects this module.
import { useEffect } from 'react';

/** Presses the rail button labelled `targets.get(variantId)`, if any, after the shell appears. */
export function useRailPick(variantId: string, targets: ReadonlyMap<string, string>): void {
  const label = targets.get(variantId);
  useEffect(() => {
    if (label === undefined) return undefined;
    const press = (): boolean => {
      const rail = document.querySelector('nav.ome-rail');
      if (rail === null) return false;
      const button = Array.from(rail.querySelectorAll('button')).find((item) => item.textContent === label);
      if (button === undefined) return false;
      button.click();
      return true;
    };
    if (press()) return undefined;
    const observer = new MutationObserver(() => {
      if (press()) observer.disconnect();
    });
    observer.observe(document.body, { childList: true, subtree: true });
    return () => { observer.disconnect(); };
  }, [variantId, label]);
}
