// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TitleBar } from './TitleBar';

vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: vi.fn() }));

type ResizeHandler = () => void;

function fakeWindow(maximized = false) {
  let onResize: ResizeHandler | undefined;
  const unlisten = vi.fn();
  const appWindow = {
    minimize: vi.fn(() => Promise.resolve()),
    toggleMaximize: vi.fn(() => Promise.resolve()),
    close: vi.fn(() => Promise.resolve()),
    isMaximized: vi.fn(() => Promise.resolve(maximized)),
    onResized: vi.fn((handler: ResizeHandler) => { onResize = handler; return Promise.resolve(unlisten); }),
  };
  vi.mocked(getCurrentWindow).mockReturnValue(appWindow as unknown as ReturnType<typeof getCurrentWindow>);
  return {
    appWindow,
    unlisten,
    resize: (next: boolean) => {
      appWindow.isMaximized.mockImplementation(() => Promise.resolve(next));
      onResize?.();
    },
  };
}

function enterTauri() {
  Object.defineProperty(window, '__TAURI_INTERNALS__', { value: {}, configurable: true });
}

afterEach(() => {
  Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
});

describe('TitleBar', () => {
  it('draws the product name, a drag region and the three caption buttons', () => {
    render(<TitleBar />);
    const bar = screen.getByRole('banner');
    expect(bar).toHaveAttribute('data-tauri-drag-region');
    expect(screen.getByText('Open Mobile Emulator')).toHaveAttribute('data-tauri-drag-region');
    expect(screen.queryByRole('heading')).not.toBeInTheDocument();
    expect(screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual(['최소화', '최대화', '닫기']);
  });

  it('does nothing and throws nothing outside Tauri', async () => {
    const user = userEvent.setup();
    render(<TitleBar />);
    for (const name of ['최소화', '최대화', '닫기']) await user.click(screen.getByRole('button', { name }));
    expect(getCurrentWindow).not.toHaveBeenCalled();
  });

  it('minimizes, toggles maximize and closes the current window inside Tauri', async () => {
    enterTauri();
    const { appWindow } = fakeWindow();
    const user = userEvent.setup();
    render(<TitleBar />);
    await user.click(screen.getByRole('button', { name: '최소화' }));
    expect(appWindow.minimize).toHaveBeenCalledOnce();
    await user.click(screen.getByRole('button', { name: '최대화' }));
    expect(appWindow.toggleMaximize).toHaveBeenCalledOnce();
    await user.click(screen.getByRole('button', { name: '닫기' }));
    expect(appWindow.close).toHaveBeenCalledOnce();
  });

  it('follows the window maximized state and stops listening on unmount', async () => {
    enterTauri();
    const { resize, unlisten } = fakeWindow(true);
    const { unmount } = render(<TitleBar />);
    expect(await screen.findByRole('button', { name: '이전 크기로' })).toBeInTheDocument();
    await act(async () => { resize(false); await Promise.resolve(); });
    expect(await screen.findByRole('button', { name: '최대화' })).toBeInTheDocument();
    unmount();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('shows the fixed state given by the gallery without touching the window', () => {
    enterTauri();
    render(<TitleBar maximized />);
    expect(screen.getByRole('button', { name: '이전 크기로' })).toBeInTheDocument();
    expect(getCurrentWindow).not.toHaveBeenCalled();
  });
});
