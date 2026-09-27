// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The overlay page treats its own box as the guest screen. In the product window that box is the
// viewport; in the QA gallery it is a 1280x800 frame. Measured on resize, never asked of Rust.
import { useEffect, useState } from 'react';
import type { Size } from '../contracts';

function windowSize(): Size {
  return { width: window.innerWidth, height: window.innerHeight };
}

function measure(element: HTMLElement | null): Size {
  const box = element?.getBoundingClientRect();
  // Not laid out (or a test DOM without layout): the viewport is the box.
  if (box === undefined || box.width <= 0 || box.height <= 0) return windowSize();
  return { width: box.width, height: box.height };
}

/** The element's CSS-pixel size, updated after layout changes. */
export function useElementSize(element: HTMLElement | null): Size {
  const [size, setSize] = useState<Size>(windowSize);
  useEffect(() => {
    let frame = 0;
    const update = () => {
      frame = 0;
      const next = measure(element);
      setSize((previous) => (previous.width === next.width && previous.height === next.height ? previous : next));
    };
    const schedule = () => {
      if (frame === 0) frame = window.requestAnimationFrame(update);
    };
    schedule();
    const observer = element === null || typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(schedule);
    if (element !== null) observer?.observe(element);
    window.addEventListener('resize', schedule);
    return () => {
      observer?.disconnect();
      window.removeEventListener('resize', schedule);
      if (frame !== 0) window.cancelAnimationFrame(frame);
    };
  }, [element]);
  return size;
}

/** The element's top-left corner in client coordinates, for turning pointer events into offsets. */
export function clientOrigin(element: HTMLElement | null): { readonly left: number; readonly top: number } {
  const box = element?.getBoundingClientRect();
  return box === undefined ? { left: 0, top: 0 } : { left: box.left, top: box.top };
}
