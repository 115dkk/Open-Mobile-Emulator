// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useEffect, useEffectEvent } from 'react';
import type { ScreenActions } from '../actions';
import type { TextInputView, TextKey } from '../contracts';

type TextActions = Pick<ScreenActions, 'textCompose' | 'textCommit' | 'textKey' | 'inputHostKey'> & {
  readonly suspendHotkey: string;
};
type Command = { kind: 'compose' | 'commit'; text: string } | { kind: 'key'; key: TextKey }
  | { kind: 'hostKey'; code: string; pressed: boolean };
const KEYS: Readonly<Record<string, TextKey | undefined>> = {
  Enter: 'enter', Backspace: 'backspace', Delete: 'delete', Tab: 'tab', Escape: 'escape',
  ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down', Home: 'home', End: 'end',
};
const DIALOG = '[role="dialog"], [aria-modal="true"]';
const EDITABLE = 'input, textarea, select, [contenteditable]';

/** The host IME owns composition; every guest text operation shares one ordered queue. */
export function useGuestTextInput(textInput: TextInputView, running: boolean, actions: TextActions): void {
  const active = textInput.state === 'active' && running;
  const hotkey = useEffectEvent(() => actions.suspendHotkey);
  const forward = useEffectEvent((command: Command) => {
    switch (command.kind) {
      case 'compose': return actions.textCompose(command.text);
      case 'commit': return actions.textCommit(command.text);
      case 'key': return actions.textKey(command.key);
      case 'hostKey': return actions.inputHostKey(command.code, command.pressed);
    }
  });
  useEffect(() => {
    if (!active) return undefined;
    const field = document.createElement('textarea');
    Object.assign(field.style, {
      position: 'fixed', left: '0', bottom: '0', width: '1px', height: '1px',
      opacity: '0', pointerEvents: 'none', resize: 'none',
    });
    field.setAttribute('aria-hidden', 'true');
    field.setAttribute('autocomplete', 'off');
    field.setAttribute('autocorrect', 'off');
    field.setAttribute('autocapitalize', 'off');
    field.spellcheck = false;
    document.body.append(field);

    const queue: Command[] = [];
    let busy = false;
    const pump = (): void => {
      const next = queue.shift();
      if (next === undefined) { busy = false; return; }
      busy = true;
      // not_active is expected while a new snapshot is in flight. Other failures also must not
      // leave the queue blocked or turn typed text into a presentation error.
      void forward(next).catch(() => undefined).then(pump);
    };
    const enqueue = (command: Command) => { queue.push(command); if (!busy) pump(); };
    let disposed = false;
    let frame = 0;
    const focus = () => {
      if (disposed || !document.hasFocus() || document.querySelector(DIALOG) !== null) return;
      const target = document.activeElement;
      if (target !== field && target instanceof Element && target.closest(EDITABLE)) return;
      field.focus({ preventScroll: true });
    };
    const scheduleFocus = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(focus);
    };
    // A dialog can close without a focusout (its focused child is removed from the DOM).
    const observer = new MutationObserver(scheduleFocus);
    observer.observe(document.body, { childList: true, subtree: true });
    document.addEventListener('focusout', scheduleFocus);
    window.addEventListener('focus', scheduleFocus);

    let composing = false;
    let echoedText: string | null = null;
    let echoTimer: ReturnType<typeof setTimeout> | undefined;
    const clearEcho = () => { clearTimeout(echoTimer); echoedText = null; };
    const expectEcho = (text: string) => {
      clearEcho();
      echoedText = text;
      // Chromium may follow compositionend with the final input in the same native event turn.
      // Do not discard the user's next independent insertion just because its text is identical.
      echoTimer = setTimeout(clearEcho, 0);
    };
    const compose = (event: CompositionEvent) => {
      clearEcho();
      composing = true;
      enqueue({ kind: 'compose', text: event.data });
    };
    const compositionEnd = (event: CompositionEvent) => {
      composing = false;
      enqueue({ kind: event.data === '' ? 'compose' : 'commit', text: event.data });
      field.value = '';
      expectEcho(event.data);
    };
    const insert = (event: InputEvent) => {
      if (composing || event.isComposing) return;
      if (event.inputType === 'insertCompositionText' || event.inputType === 'insertFromComposition') {
        field.value = '';
        return;
      }
      if (!event.inputType.startsWith('insert')) return;
      const text = event.data ?? (event.type === 'input' ? field.value : null);
      if (text === null) return;
      if (text === echoedText) {
        if (event.cancelable) event.preventDefault();
        if (event.type === 'input') clearEcho();
        field.value = '';
        return;
      }
      // Non-cancelable beforeinput is only a notification. Read the resulting input once instead.
      if (event.type === 'beforeinput' && !event.cancelable) return;
      if (event.cancelable) event.preventDefault();
      clearEcho();
      if (text !== '') enqueue({ kind: 'commit', text });
      field.value = '';
      if (event.type === 'beforeinput') expectEcho(text);
    };
    const paste = (event: ClipboardEvent) => {
      event.preventDefault();
      clearEcho();
      const text = event.clipboardData?.getData('text/plain').replace(/\r\n?/gu, '\n') ?? '';
      if (text !== '') enqueue({ kind: 'commit', text });
      field.value = '';
    };
    let heldHotkey: string | null = null;
    const release = () => {
      if (heldHotkey !== null) enqueue({ kind: 'hostKey', code: heldHotkey, pressed: false });
      heldHotkey = null;
    };
    const keyboard = (event: KeyboardEvent) => {
      if (event.type === 'keyup') {
        if (event.code === heldHotkey) { event.preventDefault(); release(); }
        return;
      }
      if (event.target !== field || event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey) return;
      if (event.code === hotkey()) {
        event.preventDefault();
        if (heldHotkey === null && !event.repeat) {
          heldHotkey = event.code;
          enqueue({ kind: 'hostKey', code: event.code, pressed: true });
        }
        return;
      }
      if (composing || event.isComposing) return;
      clearEcho();
      const key = KEYS[event.key];
      if (key !== undefined) { event.preventDefault(); enqueue({ kind: 'key', key }); }
    };
    const visibilityChanged = () => {
      if (document.visibilityState === 'hidden') release();
    };
    field.addEventListener('compositionstart', compose);
    field.addEventListener('compositionupdate', compose);
    field.addEventListener('compositionend', compositionEnd);
    field.addEventListener('beforeinput', insert);
    field.addEventListener('input', insert);
    field.addEventListener('paste', paste);
    window.addEventListener('keydown', keyboard);
    window.addEventListener('keyup', keyboard);
    window.addEventListener('blur', release);
    document.addEventListener('visibilitychange', visibilityChanged);
    focus();
    return () => {
      disposed = true;
      observer.disconnect();
      cancelAnimationFrame(frame);
      clearEcho();
      document.removeEventListener('focusout', scheduleFocus);
      window.removeEventListener('focus', scheduleFocus);
      field.removeEventListener('compositionstart', compose);
      field.removeEventListener('compositionupdate', compose);
      field.removeEventListener('compositionend', compositionEnd);
      field.removeEventListener('beforeinput', insert);
      field.removeEventListener('input', insert);
      field.removeEventListener('paste', paste);
      window.removeEventListener('keydown', keyboard);
      window.removeEventListener('keyup', keyboard);
      window.removeEventListener('blur', release);
      document.removeEventListener('visibilitychange', visibilityChanged);
      // Keep an already queued hotkey release: the press may still be in flight.
      queue.splice(0, queue.length, ...queue.filter((command) => command.kind === 'hostKey'));
      release();
      field.remove();
    };
  }, [active]);
}
