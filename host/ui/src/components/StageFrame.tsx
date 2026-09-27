// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useEffect, useEffectEvent, useRef } from 'react';
import type { ReactNode } from 'react';
import type { StageRect } from '../contracts';

export interface StageFrameProps {
  /** Receives the frame's CSS-pixel rectangle whenever it moves or resizes. Rust places the window. */
  readonly onRect: (rect: StageRect) => void | Promise<void>;
  /** What sits in the frame while no window covers it. */
  readonly children?: ReactNode;
  readonly className?: string | undefined;
}

/**
 * The darkest area where the operating system's window is placed. It reports its rectangle to
 * Rust; zero-sized rectangles (hidden, not laid out) are not reported.
 */
export function StageFrame({ onRect, children, className }: StageFrameProps) {
  const frame = useRef<HTMLDivElement>(null);
  const report = useEffectEvent((rect: StageRect) => { void onRect(rect); });

  useEffect(() => {
    const element = frame.current;
    if (element === null) return undefined;
    let last = '';
    let pending = 0;
    const measure = () => {
      pending = 0;
      const box = element.getBoundingClientRect();
      if (box.width <= 0 || box.height <= 0) return;
      const rect: StageRect = {
        x: box.x, y: box.y, width: box.width, height: box.height, scaleFactor: window.devicePixelRatio || 1,
      };
      const key = `${String(rect.x)},${String(rect.y)},${String(rect.width)},${String(rect.height)},${String(rect.scaleFactor)}`;
      if (key === last) return;
      last = key;
      report(rect);
    };
    const schedule = () => {
      if (pending === 0) pending = window.requestAnimationFrame(measure);
    };
    measure();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(schedule);
    observer?.observe(element);
    window.addEventListener('resize', schedule);
    return () => {
      observer?.disconnect();
      window.removeEventListener('resize', schedule);
      if (pending !== 0) window.cancelAnimationFrame(pending);
    };
  }, []);

  return (
    <div ref={frame} className={className === undefined ? 'ome-stage-frame' : `ome-stage-frame ${className}`}>
      {children}
    </div>
  );
}
