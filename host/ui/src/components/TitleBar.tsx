// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The main window has no native frame (`decorations: false`), so the product draws its own title bar:
// icon and name on the left, an empty drag area, and the three caption buttons on the right.
// Dragging and double-click maximize come from Tauri's own `data-tauri-drag-region` script, which
// starts the native move loop; the buttons call the window API. Outside Tauri (tests, the QA gallery)
// nothing is called. Closing only asks the window to close: Rust's `CloseRequested` handler decides
// between hiding to the tray and quitting.
import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Icon } from './Icon';

type AppWindow = ReturnType<typeof getCurrentWindow>;

function insideTauri(): boolean {
  return '__TAURI_INTERNALS__' in window;
}

function withWindow(action: (appWindow: AppWindow) => Promise<unknown>) {
  if (!insideTauri()) return;
  // A refused window call has nothing the user could do about it; the button simply does nothing.
  action(getCurrentWindow()).catch(() => undefined);
}

export interface TitleBarProps {
  /** Fixed maximized state for the QA gallery. When unset, the bar follows the real window. */
  readonly maximized?: boolean | undefined;
}

function useWindowMaximized(fixed: boolean | undefined): boolean {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    if (fixed !== undefined || !insideTauri()) return undefined;
    const appWindow = getCurrentWindow();
    let disposed = false;
    let stop: (() => void) | undefined;
    const read = () => {
      appWindow.isMaximized().then((value) => { if (!disposed) setMaximized(value); }, () => undefined);
    };
    read();
    appWindow.onResized(read).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    }, () => undefined);
    return () => {
      disposed = true;
      stop?.();
    };
  }, [fixed]);
  return fixed ?? maximized;
}

export function TitleBar({ maximized: fixed }: TitleBarProps = {}) {
  const maximized = useWindowMaximized(fixed);
  const maximizeLabel = maximized ? '이전 크기로' : '최대화';

  return (
    <header className="ome-titlebar" data-tauri-drag-region>
      <div className="ome-titlebar-title" data-tauri-drag-region>
        <img
          className="ome-titlebar-icon"
          src="/32x32.png"
          alt=""
          width={16}
          height={16}
          draggable={false}
          data-tauri-drag-region
        />
        <span className="ome-titlebar-name" data-tauri-drag-region>Open Mobile Emulator</span>
      </div>
      <div className="ome-titlebar-controls">
        <button
          type="button"
          className="ome-titlebar-button"
          aria-label="최소화"
          title="최소화"
          onClick={() => { withWindow((appWindow) => appWindow.minimize()); }}
        >
          <Icon name="minus" size={16} />
        </button>
        <button
          type="button"
          className="ome-titlebar-button"
          aria-label={maximizeLabel}
          title={maximizeLabel}
          onClick={() => { withWindow((appWindow) => appWindow.toggleMaximize()); }}
        >
          <Icon name={maximized ? 'window-restore' : 'window-maximize'} size={16} />
        </button>
        <button
          type="button"
          className="ome-titlebar-button ome-titlebar-close"
          aria-label="닫기"
          title="닫기"
          onClick={() => { withWindow((appWindow) => appWindow.close()); }}
        >
          <Icon name="x" size={16} />
        </button>
      </div>
    </header>
  );
}
