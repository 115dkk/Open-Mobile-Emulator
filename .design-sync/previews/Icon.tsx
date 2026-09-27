// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Icon } from 'open-mobile-emulator';

const cell = { display: 'flex', flexDirection: 'column' as const, alignItems: 'center', gap: 6, width: 72 };
const caption = { font: 'var(--type-caption)', color: 'var(--muted)' };

function Row({ names, size, strokeWidth }: { readonly names: readonly string[]; readonly size?: number; readonly strokeWidth?: number }) {
  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
      {names.map((name) => (
        <div key={name} style={cell}>
          <Icon name={name as never} size={size} strokeWidth={strokeWidth} />
          <span style={caption}>{name}</span>
        </div>
      ))}
    </div>
  );
}

/** The five rail icons at 20px, stroke 1.5 (2 when selected). */
export function Navigation() {
  return <Row names={['stage', 'apps', 'input', 'display', 'settings']} />;
}

/** Status icons: check, warning, error, info. */
export function Status() {
  return <Row names={['check', 'alert-triangle', 'alert-circle', 'info']} strokeWidth={2} />;
}

/** Toolbar and row actions at 18px. */
export function Actions() {
  return (
    <Row
      names={['camera', 'volume', 'power', 'play', 'pause', 'refresh', 'download', 'file-up', 'folder-open',
        'external-link', 'copy', 'pencil', 'trash', 'eye', 'eye-off', 'search', 'plus', 'minus', 'x', 'undo', 'save']}
      size={18}
    />
  );
}

/** Objects and chevrons. */
export function Objects() {
  return <Row names={['app-window', 'hard-drive', 'smartphone', 'mouse', 'arrow-right', 'chevron-down', 'chevron-right']} />;
}

/** The three sizes in use: 16 inline, 20 navigation, 32 empty states. */
export function Sizes() {
  return (
    <div style={{ display: 'flex', alignItems: 'flex-end', gap: 16 }}>
      <Icon name="download" size={16} />
      <Icon name="download" size={20} />
      <Icon name="download" size={32} />
    </div>
  );
}
