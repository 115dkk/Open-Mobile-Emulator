// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { createElement } from 'react';
import { render, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TextInputView, TextKey } from '../contracts';
import { stageGallery, wizardGallery } from '../fixtures';
import { mockActions } from '../test-actions';
import { StageScreen } from './StageScreen';
import { WizardScreen } from './WizardScreen';
import { useGuestTextInput } from './use-guest-text-input';

const ACTIVE: TextInputView = { state: 'active', inputType: 1, package: 'example.app' };
const IDLE: TextInputView = { state: 'idle', inputType: null, package: null };
const flush = () => new Promise<void>((resolve) => { setTimeout(resolve, 0); });
const nextFrame = () => new Promise<void>((resolve) => { requestAnimationFrame(() => { resolve(); }); });
function actions() {
  return {
    textCompose: vi.fn<(text: string) => Promise<void>>(() => Promise.resolve()),
    textCommit: vi.fn<(text: string) => Promise<void>>(() => Promise.resolve()),
    textKey: vi.fn<(key: TextKey) => Promise<void>>(() => Promise.resolve()),
    inputHostKey: vi.fn<(code: string, pressed: boolean) => Promise<void>>(() => Promise.resolve()),
    suspendHotkey: 'F10',
  };
}
function field(): HTMLTextAreaElement {
  const element = document.querySelector('textarea[aria-hidden="true"]');
  if (!(element instanceof HTMLTextAreaElement)) throw new Error('Missing text input');
  return element;
}
function composition(type: string, data: string) {
  field().dispatchEvent(new CompositionEvent(type, { data, bubbles: true }));
}
function insert(type: string, data: string | null, init: InputEventInit = {}) {
  const event = new InputEvent(type, { inputType: 'insertText', data, bubbles: true, cancelable: true, ...init });
  field().dispatchEvent(event);
  return event;
}
function key(key: string, init: KeyboardEventInit = {}, type = 'keydown', target: EventTarget = field()) {
  const event = new KeyboardEvent(type, { key, code: key, bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  return event;
}
function paste(text: string) {
  const event = new Event('paste', { bubbles: true, cancelable: true });
  Object.defineProperty(event, 'clipboardData', { value: { getData: (kind: string) => kind === 'text/plain' ? text : '' } });
  field().dispatchEvent(event);
  return event;
}

beforeEach(() => { vi.spyOn(document, 'hasFocus').mockReturnValue(true); });

describe('guest text input', () => {
  it('creates and focuses one hidden textarea only while active and running', () => {
    const send = actions();
    const { rerender, unmount } = renderHook(({ textInput, running }) => useGuestTextInput(textInput, running, send), {
      initialProps: { textInput: IDLE, running: true },
    });
    expect(document.querySelector('textarea')).toBeNull();
    rerender({ textInput: ACTIVE, running: false });
    expect(document.querySelector('textarea')).toBeNull();
    rerender({ textInput: ACTIVE, running: true });
    const input = field();
    expect(input).toHaveFocus();
    expect(input).toHaveStyle({ position: 'fixed', left: '0px', bottom: '0px', width: '1px', height: '1px', opacity: '0', pointerEvents: 'none', resize: 'none' });
    for (const attribute of ['autocomplete', 'autocorrect', 'autocapitalize']) expect(input).toHaveAttribute(attribute, 'off');
    expect(input.spellcheck).toBe(false);
    rerender({ textInput: { ...ACTIVE, inputType: 2 }, running: true });
    expect(field()).toBe(input);
    rerender({ textInput: IDLE, running: true });
    expect(input).not.toBeInTheDocument();
    expect(input).not.toHaveFocus();
    rerender({ textInput: ACTIVE, running: true });
    unmount();
    expect(document.querySelector('textarea')).toBeNull();
  });

  it('serializes composition, commit, keys and the configured suspend shortcut', async () => {
    const send = actions();
    const sent: string[] = [];
    let finish = (): void => undefined;
    send.textCompose.mockImplementationOnce((text) => {
      sent.push(`compose:${text}`);
      return new Promise<void>((resolve) => { finish = resolve; });
    }).mockImplementation((text) => { sent.push(`compose:${text}`); return Promise.resolve(); });
    send.textCommit.mockImplementation((text) => { sent.push(`commit:${text}`); return Promise.resolve(); });
    send.textKey.mockImplementation((key) => { sent.push(`key:${key}`); return Promise.resolve(); });
    send.inputHostKey.mockImplementation((code, pressed) => { sent.push(`${code}:${String(pressed)}`); return Promise.resolve(); });
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    composition('compositionstart', 'ㅎ');
    composition('compositionupdate', '하');
    composition('compositionupdate', '한');
    field().value = '한';
    composition('compositionend', '한');
    key('Enter');
    key('F10');
    key('F10', {}, 'keyup');
    expect(field()).toHaveValue('');
    await flush();
    expect(sent).toEqual(['compose:ㅎ']);
    finish();
    await flush();
    expect(sent).toEqual(['compose:ㅎ', 'compose:하', 'compose:한', 'commit:한', 'key:enter', 'F10:true', 'F10:false']);
  });

  it('clears a canceled composition instead of committing an empty string', async () => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    composition('compositionstart', '');
    composition('compositionend', '');
    await flush();
    expect(send.textCompose.mock.calls).toEqual([[''], ['']]);
    expect(send.textCommit).not.toHaveBeenCalled();
  });

  it('leaves composing Backspace and Enter to the host IME', async () => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    expect(key('Backspace', { isComposing: true }).defaultPrevented).toBe(false);
    composition('compositionstart', '');
    expect(key('Backspace').defaultPrevented).toBe(false);
    expect(key('Enter', { isComposing: true }).defaultPrevented).toBe(false);
    insert('input', 'ㅎ', { isComposing: true, inputType: 'insertCompositionText' });
    await flush();
    expect(send.textKey).not.toHaveBeenCalled();
    expect(send.textCommit).not.toHaveBeenCalled();
  });

  it.each([
    ['Backspace', 'backspace'], ['Enter', 'enter'], ['Delete', 'delete'], ['Tab', 'tab'], ['Escape', 'escape'],
    ['ArrowLeft', 'left'], ['ArrowRight', 'right'], ['ArrowUp', 'up'], ['ArrowDown', 'down'], ['Home', 'home'], ['End', 'end'],
  ])('forwards non-composing %s as textKey %s', async (name, command) => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    expect(key(name).defaultPrevented).toBe(true);
    key(name, {}, 'keyup');
    await flush();
    expect(send.textKey.mock.calls).toEqual([[command]]);
    expect(send.inputHostKey).not.toHaveBeenCalled();
  });

  it.each([{ ctrlKey: true }, { altKey: true }, { metaKey: true }])('ignores modified keys %j', async (modifier) => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    expect(key('Backspace', modifier).defaultPrevented).toBe(false);
    expect(key('v', { ...modifier, code: 'KeyV' }).defaultPrevented).toBe(false);
    await flush();
    expect(send.textKey).not.toHaveBeenCalled();
    expect(send.inputHostKey).not.toHaveBeenCalled();
  });

  it('commits cancelable beforeinput once and handles input-only text', async () => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    expect(insert('beforeinput', 'a').defaultPrevented).toBe(true);
    insert('input', 'a');
    field().value = 'b';
    insert('input', null);
    insert('beforeinput', 'c', { cancelable: false });
    insert('input', 'c');
    await flush();
    expect(send.textCommit.mock.calls).toEqual([['a'], ['b'], ['c']]);
    expect(field()).toHaveValue('');
  });

  it.each(['insertText', 'insertCompositionText', 'insertFromComposition'])('does not duplicate the final %s after compositionend', async (inputType) => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    composition('compositionstart', '');
    composition('compositionupdate', '한');
    insert('input', '한', { isComposing: true, inputType: 'insertCompositionText' });
    composition('compositionend', '한');
    insert('beforeinput', '한', { inputType });
    field().value = '한';
    insert('input', '한', { inputType });
    await flush();
    expect(send.textCommit.mock.calls).toEqual([['한']]);
    insert('input', '한');
    await flush();
    expect(send.textCommit.mock.calls).toEqual([['한'], ['한']]);
    expect(field()).toHaveValue('');
  });

  it('commits a multiline paste in one command with normalized newlines', async () => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    expect(paste('한글\r\n둘째\r셋째\n끝').defaultPrevented).toBe(true);
    await flush();
    expect(send.textCommit.mock.calls).toEqual([['한글\n둘째\n셋째\n끝']]);
    expect(field()).toHaveValue('');
  });

  it('handles an insertFromPaste input fallback once', async () => {
    const send = actions();
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    insert('beforeinput', null, { inputType: 'insertFromPaste' });
    field().value = '붙여넣기';
    insert('input', null, { inputType: 'insertFromPaste' });
    await flush();
    expect(send.textCommit.mock.calls).toEqual([['붙여넣기']]);
  });

  it.each(['role', 'aria-modal'])('does not take focus while a %s dialog exists, even before it is focused', async (attribute) => {
    const dialog = document.createElement('div');
    dialog.setAttribute(attribute, attribute === 'role' ? 'dialog' : 'true');
    document.body.append(dialog);
    try {
      renderHook(() => useGuestTextInput(ACTIVE, true, actions()));
      expect(field()).not.toHaveFocus();
      dialog.remove();
      await flush();
      await nextFrame();
      expect(field()).toHaveFocus();
    } finally { dialog.remove(); }
  });

  it.each(['input', 'textarea', 'select', 'contenteditable'])('does not take focus from another %s', async (kind) => {
    const editable = document.createElement(kind === 'contenteditable' ? 'div' : kind);
    editable.tabIndex = 0;
    if (kind === 'contenteditable') editable.setAttribute('contenteditable', 'true');
    document.body.append(editable);
    editable.focus();
    try {
      renderHook(() => useGuestTextInput(ACTIVE, true, actions()));
      expect(editable).toHaveFocus();
      editable.blur();
      await nextFrame();
      expect(field()).toHaveFocus();
    } finally { editable.remove(); }
  });

  it('refocuses next frame after blur, but not while the host window is unfocused', async () => {
    const focused = vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    renderHook(() => useGuestTextInput(ACTIVE, true, actions()));
    field().blur();
    await nextFrame();
    expect(field()).toHaveFocus();
    focused.mockReturnValue(false);
    field().blur();
    await nextFrame();
    expect(field()).not.toHaveFocus();
    focused.mockReturnValue(true);
    window.dispatchEvent(new Event('focus'));
    await nextFrame();
    expect(field()).toHaveFocus();
  });

  it.each(['text_input_not_active', 'connection_failed'])('continues the queue after %s', async (code) => {
    const send = actions();
    send.textCompose.mockRejectedValueOnce({ code });
    renderHook(() => useGuestTextInput(ACTIVE, true, send));
    composition('compositionstart', 'ㅎ');
    composition('compositionupdate', '한');
    composition('compositionend', '한');
    key('Enter');
    await flush();
    expect(send.textCompose.mock.calls).toEqual([['ㅎ'], ['한']]);
    expect(send.textCommit).toHaveBeenCalledWith('한');
    expect(send.textKey).toHaveBeenCalledWith('enter');
  });

  it('uses the latest actions and hotkey without replacing the input or its composition', async () => {
    const first = actions();
    const second = { ...actions(), suspendHotkey: 'F11' };
    const { rerender, unmount } = renderHook(({ send }) => useGuestTextInput(ACTIVE, true, send), { initialProps: { send: first } });
    const input = field();
    composition('compositionstart', 'ㅎ');
    rerender({ send: second });
    expect(field()).toBe(input);
    composition('compositionend', '한');
    key('F10');
    key('F11');
    key('F11', { repeat: true });
    await flush();
    window.dispatchEvent(new Event('blur'));
    unmount();
    await flush();
    expect(first.textCompose).toHaveBeenCalledWith('ㅎ');
    expect(second.textCommit).toHaveBeenCalledWith('한');
    expect(second.inputHostKey.mock.calls).toEqual([['F11', true], ['F11', false]]);
  });

  it('preserves a queued hotkey release when deactivated before its press resolves', async () => {
    const send = actions();
    let finish = (): void => undefined;
    send.inputHostKey.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
    const { unmount } = renderHook(() => useGuestTextInput(ACTIVE, true, send));
    key('F10');
    key('F10', {}, 'keyup');
    unmount();
    expect(send.inputHostKey.mock.calls).toEqual([['F10', true]]);
    finish();
    await flush();
    expect(send.inputHostKey.mock.calls).toEqual([['F10', true], ['F10', false]]);
  });

  it('drops unsent text on deactivation and removes detached input listeners', async () => {
    const send = actions();
    let finish = (): void => undefined;
    send.textCompose.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
    const { rerender } = renderHook(({ state }) => useGuestTextInput(state, true, send), { initialProps: { state: ACTIVE } });
    const input = field();
    composition('compositionstart', 'ㅎ');
    composition('compositionend', '한');
    rerender({ state: IDLE });
    input.dispatchEvent(new CompositionEvent('compositionend', { data: '다' }));
    finish();
    await flush();
    await nextFrame();
    expect(send.textCommit).not.toHaveBeenCalled();
    expect(document.querySelector('textarea')).toBeNull();
  });
});

describe('screen keyboard and text input wiring', () => {
  it.each([
    { name: 'stage', component: StageScreen, fixture: stageGallery.find((item) => item.id === 'stage-running') },
    { name: 'wizard first boot', component: WizardScreen, fixture: wizardGallery.find((item) => item.id === 'first-boot') },
  ])('$name stops physical keyboard forwarding while text input is active', async ({ component, fixture }) => {
    if (fixture === undefined) throw new Error('Missing screen fixture');
    const snapshot = {
      ...fixture.snapshot,
      guest: { ...fixture.snapshot.guest, state: 'running' as const },
      input: { ...fixture.snapshot.input, editing: false, suspendHotkey: 'F9' },
      textInput: ACTIVE,
    };
    const send = mockActions();
    const { rerender } = render(createElement(component, { snapshot, actions: send }));
    key('a', { code: 'KeyA' }, 'keydown', window);
    key('a', { code: 'KeyA' }, 'keyup', window);
    key('Enter');
    key('F9');
    key('F9', {}, 'keyup');
    await flush();
    expect(send.textKey.mock.calls).toEqual([['enter']]);
    expect(send.inputHostKey.mock.calls).toEqual([['F9', true], ['F9', false]]);
    send.inputHostKey.mockClear();
    rerender(createElement(component, { snapshot: { ...snapshot, textInput: IDLE }, actions: send }));
    key('a', { code: 'KeyA' }, 'keydown', window);
    key('a', { code: 'KeyA' }, 'keyup', window);
    await flush();
    expect(send.inputHostKey.mock.calls).toEqual([['KeyA', true], ['KeyA', false]]);
  });
});
