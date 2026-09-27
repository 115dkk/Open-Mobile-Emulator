// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Lucide icons (ISC) drawn inline with currentColor. Path data for the icons used in the M2 mockups
// is copied from the mockup HTML. The rest (info, chevron-right, pause, minus, copy, mouse, pencil,
// trash, eye, eye-off, undo, save, app-window, hard-drive, smartphone) were written from memory of
// Lucide and are not yet checked against a Lucide source: 확인 필요.
import type { ReactElement } from 'react';

export type IconName =
  | 'stage' | 'apps' | 'input' | 'display' | 'settings'
  | 'refresh' | 'check' | 'alert-triangle' | 'alert-circle' | 'info' | 'arrow-right'
  | 'chevron-down' | 'chevron-right' | 'file-up' | 'download' | 'external-link' | 'folder-open'
  | 'camera' | 'volume' | 'power' | 'play' | 'pause' | 'search' | 'plus' | 'minus' | 'x'
  | 'copy' | 'mouse' | 'pencil' | 'trash' | 'eye' | 'eye-off' | 'undo' | 'save'
  | 'app-window' | 'hard-drive' | 'smartphone';

function shapes(name: IconName): ReactElement {
  switch (name) {
    case 'stage':
      return (<>
        <path d="M10 7.75a.75.75 0 0 1 1.142-.638l3.664 2.249a.75.75 0 0 1 0 1.278l-3.664 2.25a.75.75 0 0 1-1.142-.64z" />
        <path d="M12 17v4" /><path d="M8 21h8" /><rect x="2" y="3" width="20" height="14" rx="2" />
      </>);
    case 'apps':
      return (<>
        <rect width="7" height="7" x="3" y="3" rx="1" /><rect width="7" height="7" x="14" y="3" rx="1" />
        <rect width="7" height="7" x="14" y="14" rx="1" /><rect width="7" height="7" x="3" y="14" rx="1" />
      </>);
    case 'input':
      return (<>
        <path d="M10 8h.01" /><path d="M12 12h.01" /><path d="M14 8h.01" /><path d="M16 12h.01" />
        <path d="M18 8h.01" /><path d="M6 8h.01" /><path d="M7 16h10" /><path d="M8 12h.01" />
        <rect width="20" height="16" x="2" y="4" rx="2" />
      </>);
    case 'display':
      return (<>
        <path d="M8 3H5a2 2 0 0 0-2 2v3" /><path d="M21 8V5a2 2 0 0 0-2-2h-3" />
        <path d="M3 16v3a2 2 0 0 0 2 2h3" /><path d="M16 21h3a2 2 0 0 0 2-2v-3" />
      </>);
    case 'settings':
      return (<>
        <path d="M21 4h-7" /><path d="M10 4H3" /><path d="M21 12h-9" /><path d="M8 12H3" />
        <path d="M21 20h-5" /><path d="M12 20H3" /><path d="M14 2v4" /><path d="M8 10v4" /><path d="M16 18v4" />
      </>);
    case 'refresh':
      return (<><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /><path d="M3 3v5h5" /></>);
    case 'check':
      return <path d="M20 6 9 17l-5-5" />;
    case 'alert-triangle':
      return (<>
        <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" />
        <path d="M12 9v4" /><path d="M12 17h.01" />
      </>);
    case 'alert-circle':
      return (<>
        <circle cx="12" cy="12" r="10" /><line x1="12" x2="12" y1="8" y2="12" /><line x1="12" x2="12.01" y1="16" y2="16" />
      </>);
    case 'info':
      return (<><circle cx="12" cy="12" r="10" /><path d="M12 16v-4" /><path d="M12 8h.01" /></>);
    case 'arrow-right':
      return (<><path d="M5 12h14" /><path d="m12 5 7 7-7 7" /></>);
    case 'chevron-down':
      return <path d="m6 9 6 6 6-6" />;
    case 'chevron-right':
      return <path d="m9 18 6-6-6-6" />;
    case 'file-up':
      return (<>
        <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" /><path d="M14 2v4a2 2 0 0 0 2 2h4" />
        <path d="M12 12v6" /><path d="m15 15-3-3-3 3" />
      </>);
    case 'download':
      return (<><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><path d="m7 10 5 5 5-5" /><path d="M12 15V3" /></>);
    case 'external-link':
      return (<>
        <path d="M15 3h6v6" /><path d="M10 14 21 3" />
        <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
      </>);
    case 'folder-open':
      return <path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" />;
    case 'camera':
      return (<>
        <path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z" />
        <circle cx="12" cy="13" r="3" />
      </>);
    case 'volume':
      return (<>
        <path d="M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z" />
        <path d="M16 9a5 5 0 0 1 0 6" /><path d="M19.364 18.364a9 9 0 0 0 0-12.728" />
      </>);
    case 'power':
      return (<><path d="M12 2v10" /><path d="M18.4 6.6a9 9 0 1 1-12.77.04" /></>);
    case 'play':
      return <polygon points="6 3 20 12 6 21 6 3" />;
    case 'pause':
      return (<><rect x="14" y="4" width="4" height="16" rx="1" /><rect x="6" y="4" width="4" height="16" rx="1" /></>);
    case 'search':
      return (<><circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" /></>);
    case 'plus':
      return (<><path d="M5 12h14" /><path d="M12 5v14" /></>);
    case 'minus':
      return <path d="M5 12h14" />;
    case 'x':
      return (<><path d="M18 6 6 18" /><path d="m6 6 12 12" /></>);
    case 'copy':
      return (<>
        <rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
        <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
      </>);
    case 'mouse':
      return (<><rect x="5" y="2" width="14" height="20" rx="7" /><path d="M12 6v4" /></>);
    case 'pencil':
      return (<>
        <path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" />
        <path d="m15 5 4 4" />
      </>);
    case 'trash':
      return (<>
        <path d="M3 6h18" /><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
        <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
        <line x1="10" x2="10" y1="11" y2="17" /><line x1="14" x2="14" y1="11" y2="17" />
      </>);
    case 'eye':
      return (<>
        <path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0" />
        <circle cx="12" cy="12" r="3" />
      </>);
    case 'eye-off':
      return (<>
        <path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" />
        <path d="M14.084 14.158a3 3 0 0 1-4.242-4.242" />
        <path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" />
        <path d="m2 2 20 20" />
      </>);
    case 'undo':
      return (<><path d="M9 14 4 9l5-5" /><path d="M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11" /></>);
    case 'save':
      return (<>
        <path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
        <path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" /><path d="M7 3v4a1 1 0 0 0 1 1h7" />
      </>);
    case 'app-window':
      return (<>
        <rect x="2" y="4" width="20" height="16" rx="2" /><path d="M10 4v4" /><path d="M2 8h20" /><path d="M6 4v4" />
      </>);
    case 'hard-drive':
      return (<>
        <line x1="22" x2="2" y1="12" y2="12" />
        <path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" />
        <line x1="6" x2="6.01" y1="16" y2="16" /><line x1="10" x2="10.01" y1="16" y2="16" />
      </>);
    case 'smartphone':
      return (<><rect width="14" height="20" x="5" y="2" rx="2" ry="2" /><path d="M12 18h.01" /></>);
  }
}

export interface IconProps {
  readonly name: IconName;
  /** Pixel size of the square icon. 20 in navigation and toolbars, 18 in buttons, 16 inline. */
  readonly size?: number | undefined;
  /** 1.5 next to regular text, 2 next to semibold text (DESIGN.md 6). */
  readonly strokeWidth?: number | undefined;
  readonly className?: string | undefined;
}

export function Icon({ name, size = 20, strokeWidth = 1.5, className }: IconProps) {
  return (
    <svg
      className={className === undefined ? 'ome-icon' : `ome-icon ${className}`}
      aria-hidden="true"
      focusable="false"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {shapes(name)}
    </svg>
  );
}
