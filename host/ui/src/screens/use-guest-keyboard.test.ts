// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useGuestKeyboard } from './use-guest-keyboard';

function key(type: 'keydown' | 'keyup', target: EventTarget = window, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent(type, { code: 'KeyA', bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  return event;
}

function sender() {
  return vi.fn<(code: string, pressed: boolean) => Promise<void>>(() => Promise.resolve());
}

describe('guest keyboard forwarding', () => {
  it('forwards down and up and prevents browser defaults', () => {
    const send = sender();
    const { unmount } = renderHook(() => useGuestKeyboard(true, send));
    expect(key('keydown').defaultPrevented).toBe(true);
    expect(key('keyup').defaultPrevented).toBe(true);
    expect(send.mock.calls).toEqual([['KeyA', true], ['KeyA', false]]);
    unmount();
    expect(send).toHaveBeenCalledTimes(2);
  });

  it('does nothing when inactive', () => {
    const send = sender();
    renderHook(() => useGuestKeyboard(false, send));
    expect(key('keydown').defaultPrevented).toBe(false);
    expect(key('keyup').defaultPrevented).toBe(false);
    expect(send).not.toHaveBeenCalled();
  });

  it('ignores repeated keydown but still forwards keyup', () => {
    const send = sender();
    renderHook(() => useGuestKeyboard(true, send));
    key('keydown');
    expect(key('keydown', window, { repeat: true }).defaultPrevented).toBe(false);
    key('keyup', window, { repeat: true });
    expect(send.mock.calls).toEqual([['KeyA', true], ['KeyA', false]]);
  });

  it.each(['role', 'aria-modal'])('ignores targets inside a %s dialog', (attribute) => {
    const send = sender();
    renderHook(() => useGuestKeyboard(true, send));
    const dialog = document.createElement('div');
    dialog.setAttribute(attribute, attribute === 'role' ? 'dialog' : 'true');
    const button = document.createElement('button');
    dialog.append(button);
    document.body.append(dialog);
    try {
      expect(key('keydown', button).defaultPrevented).toBe(false);
      expect(key('keyup', button).defaultPrevented).toBe(false);
      expect(send).not.toHaveBeenCalled();
    } finally { dialog.remove(); }
  });

  it.each(['input', 'textarea', 'select', 'contenteditable'])('ignores %s targets', (kind) => {
    const send = sender();
    renderHook(() => useGuestKeyboard(true, send));
    const editable = document.createElement(kind === 'contenteditable' ? 'div' : kind);
    const child = document.createElement('span');
    if (kind === 'contenteditable') {
      editable.setAttribute('contenteditable', 'true');
      editable.append(child);
    }
    document.body.append(editable);
    const target = kind === 'contenteditable' ? child : editable;
    try {
      expect(key('keydown', target).defaultPrevented).toBe(false);
      expect(key('keyup', target).defaultPrevented).toBe(false);
      expect(send).not.toHaveBeenCalled();
    } finally { editable.remove(); }
  });

  it('lets document listeners prevent forwarding before the window bubble listener', () => {
    const send = sender();
    renderHook(() => useGuestKeyboard(true, send));
    const prevent = (event: KeyboardEvent) => { event.preventDefault(); };
    document.addEventListener('keydown', prevent);
    document.addEventListener('keyup', prevent);
    try {
      expect(key('keydown', document.body).defaultPrevented).toBe(true);
      expect(key('keyup', document.body).defaultPrevented).toBe(true);
      expect(send).not.toHaveBeenCalled();
    } finally {
      document.removeEventListener('keydown', prevent);
      document.removeEventListener('keyup', prevent);
    }
  });

  it.each([{ isComposing: true }, { code: '' }])('ignores IME and empty-code events %j', (init) => {
    const send = sender();
    renderHook(() => useGuestKeyboard(true, send));
    expect(key('keydown', window, init).defaultPrevented).toBe(false);
    expect(key('keyup', window, init).defaultPrevented).toBe(false);
    expect(send).not.toHaveBeenCalled();
  });

  it.each(['Enter', 'Space'])('forwards %s on focused buttons and prevents default activation', (code) => {
    const send = sender();
    renderHook(() => useGuestKeyboard(true, send));
    const button = document.createElement('button');
    document.body.append(button);
    button.focus();
    try {
      expect(key('keydown', button, { code }).defaultPrevented).toBe(true);
      expect(key('keyup', button, { code }).defaultPrevented).toBe(true);
      expect(send.mock.calls).toEqual([[code, true], [code, false]]);
    } finally { button.remove(); }
  });

  it('releases all held keys once on blur', () => {
    const send = sender();
    const { unmount } = renderHook(() => useGuestKeyboard(true, send));
    key('keydown');
    key('keydown', window, { code: 'KeyB' });
    window.dispatchEvent(new Event('blur'));
    window.dispatchEvent(new Event('blur'));
    unmount();
    expect(send.mock.calls).toEqual([['KeyA', true], ['KeyB', true], ['KeyA', false], ['KeyB', false]]);
  });

  it('releases held keys only when the document becomes hidden', () => {
    const send = sender();
    renderHook(() => useGuestKeyboard(true, send));
    key('keydown');
    const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    document.dispatchEvent(new Event('visibilitychange'));
    expect(send).toHaveBeenCalledOnce();
    visibility.mockReturnValue('hidden');
    document.dispatchEvent(new Event('visibilitychange'));
    document.dispatchEvent(new Event('visibilitychange'));
    expect(send.mock.calls).toEqual([['KeyA', true], ['KeyA', false]]);
  });

  it('releases held keys when inactive and can listen again', () => {
    const send = sender();
    const { rerender } = renderHook(({ active }) => useGuestKeyboard(active, send), { initialProps: { active: true } });
    key('keydown');
    rerender({ active: false });
    expect(key('keydown').defaultPrevented).toBe(false);
    expect(send.mock.calls).toEqual([['KeyA', true], ['KeyA', false]]);
    rerender({ active: true });
    key('keydown');
    key('keyup');
    expect(send).toHaveBeenCalledTimes(4);
  });

  it('releases on unmount and sends nothing afterwards', () => {
    const send = sender();
    const { unmount } = renderHook(() => useGuestKeyboard(true, send));
    key('keydown');
    unmount();
    expect(send.mock.calls).toEqual([['KeyA', true], ['KeyA', false]]);
    expect(key('keydown').defaultPrevented).toBe(false);
    expect(key('keyup').defaultPrevented).toBe(false);
    window.dispatchEvent(new Event('blur'));
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
    document.dispatchEvent(new Event('visibilitychange'));
    expect(send).toHaveBeenCalledTimes(2);
  });

  it('uses the latest sender without releasing held keys on a rerender', () => {
    const first = sender();
    const second = sender();
    const { rerender, unmount } = renderHook(({ send }) => useGuestKeyboard(true, send), { initialProps: { send: first } });
    key('keydown');
    rerender({ send: second });
    expect(first.mock.calls).toEqual([['KeyA', true]]);
    expect(second).not.toHaveBeenCalled();
    unmount();
    expect(second.mock.calls).toEqual([['KeyA', false]]);
  });
});
