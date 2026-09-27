// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { StatusDot } from 'open-mobile-emulator';

const row = { display: 'flex', alignItems: 'center', gap: 8 };
const caption = { font: 'var(--type-caption)', color: 'var(--muted)' };

/** The five tones; the words beside the dot carry the state. */
export function Tones() {
  return (
    <div style={{ display: 'flex', gap: 24, flexWrap: 'wrap' }}>
      <span style={row}><StatusDot tone="muted" /><span style={caption}>꺼짐</span></span>
      <span style={row}><StatusDot tone="accent" /><span style={caption}>진행 중</span></span>
      <span style={row}><StatusDot tone="success" /><span style={caption}>실행 중</span></span>
      <span style={row}><StatusDot tone="warning" /><span style={caption}>주의</span></span>
      <span style={row}><StatusDot tone="danger" /><span style={caption}>실패</span></span>
    </div>
  );
}

/** Pulsing halo for work in progress (still under reduced motion). */
export function Pulse() {
  return (
    <span style={row}><StatusDot tone="accent" pulse /><span style={caption}>시작 중</span></span>
  );
}
