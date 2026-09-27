// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// What the overlay window draws, read off the snapshot (ADR-0005, ARCHITECTURE.md 3.17), and the
// client-side mirror of the profile rules in ome-input (`Profile::validate`, `is_keyboard_code`).
// The mirror only lets the editor say `이미 쓰는 키입니다.` before saving; Rust stays the judge.
import type { AppSnapshot, Binding, InputProfile, MouseButton, WheelDirection } from '../contracts';
import { actionWords, triggerLabel } from '../input-profile';

export type OverlayMode = 'hidden' | 'showing' | 'editing';

/** The five marker kinds of the editor's tool picker (M2-SCREENS.md 4). */
export type Tool = 'tap' | 'joystick' | 'swipe' | 'mouse' | 'wheel';

/** Editor state the QA gallery starts from; the product window never passes one. */
export interface OverlaySeed {
  readonly tool?: Tool | undefined;
  readonly selectedId?: string | undefined;
  readonly cardOpen?: boolean | undefined;
}

export function activeProfile(snapshot: AppSnapshot): InputProfile | null {
  const { input } = snapshot;
  return input.profiles.find((profile) => profile.id === input.activeId) ?? null;
}

/**
 * Hidden when the operating system is not running, no profile is applied, or markers are switched
 * off outside the editor; editing when the editor is on; showing otherwise. Rust shows and hides
 * the window on the same conditions, so the page never draws stale markers.
 */
export function overlayMode(snapshot: AppSnapshot): OverlayMode {
  const { guest, input } = snapshot;
  if (guest.state !== 'running' || activeProfile(snapshot) === null) return 'hidden';
  if (!input.overlayVisible && !input.editing) return 'hidden';
  return input.editing ? 'editing' : 'showing';
}

const NAMED_CODES: ReadonlySet<string> = new Set([
  'Backquote', 'Backslash', 'Backspace', 'BracketLeft', 'BracketRight', 'Comma', 'ContextMenu', 'Delete',
  'End', 'Enter', 'Equal', 'Escape', 'Home', 'Insert', 'Minus', 'PageDown', 'PageUp', 'Period', 'Quote',
  'Semicolon', 'Slash', 'Space', 'Tab', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ShiftLeft',
  'ShiftRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'MetaLeft', 'MetaRight', 'CapsLock',
  'NumLock', 'ScrollLock',
]);

const NUMPAD_SUFFIXES: ReadonlySet<string> = new Set([
  '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
  'Add', 'Comma', 'Decimal', 'Divide', 'Enter', 'Equal', 'Multiply', 'Subtract',
]);

/** The W3C `KeyboardEvent.code` vocabulary ome-input accepts (`is_keyboard_code`), value for value. */
export function isKeyboardCode(code: string): boolean {
  if (NAMED_CODES.has(code)) return true;
  if (/^Key[A-Z]$/u.test(code) || /^Digit[0-9]$/u.test(code)) return true;
  const fn = /^F(\d{1,3})$/u.exec(code);
  if (fn?.[1] !== undefined) {
    const number = Number(fn[1]);
    return number >= 1 && number <= 24;
  }
  return code.startsWith('Numpad') && NUMPAD_SUFFIXES.has(code.slice('Numpad'.length));
}

function triggerCodes(binding: Binding): readonly string[] {
  const { trigger } = binding;
  switch (trigger.kind) {
    case 'key': return [trigger.code];
    case 'keySet': return [trigger.up, trigger.down, trigger.left, trigger.right];
    case 'mouseButton':
    case 'wheel':
      return [];
  }
}

/** Every keyboard code the bindings use, leaving out the binding `exceptId` (the one being re-keyed). */
export function usedKeyCodes(bindings: readonly Binding[], exceptId: string | null): ReadonlySet<string> {
  const codes = new Set<string>();
  for (const binding of bindings) {
    if (binding.id === exceptId) continue;
    for (const code of triggerCodes(binding)) codes.add(code);
  }
  return codes;
}

export function mouseButtonTaken(bindings: readonly Binding[], button: MouseButton, exceptId: string | null): boolean {
  return bindings.some((binding) => binding.id !== exceptId && binding.trigger.kind === 'mouseButton' && binding.trigger.button === button);
}

export function wheelTaken(bindings: readonly Binding[], direction: WheelDirection, exceptId: string | null): boolean {
  return bindings.some((binding) => binding.id !== exceptId && binding.trigger.kind === 'wheel' && binding.trigger.direction === direction);
}

export type KeyVerdict = 'ok' | 'invalid' | 'taken';

/**
 * Whether `code` may become a trigger: in the vocabulary, not used by another binding, not one of the
 * joystick keys already captured (`captured`), and not the suspend hotkey, which the input gate takes
 * before any binding sees it.
 */
export function judgeKey(
  code: string,
  bindings: readonly Binding[],
  exceptId: string | null,
  suspendHotkey: string,
  captured: readonly string[] = [],
): KeyVerdict {
  if (!isKeyboardCode(code)) return 'invalid';
  if (code === suspendHotkey || captured.includes(code) || usedKeyCodes(bindings, exceptId).has(code)) return 'taken';
  return 'ok';
}

/** Structural equality for contract values; Rust's key order need not match objects built here. */
export function sameValue(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  if (typeof left !== 'object' || typeof right !== 'object' || left === null || right === null) return false;
  if (Array.isArray(left) || Array.isArray(right)) {
    if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) return false;
    return left.every((item, index) => sameValue(item, right[index]));
  }
  const leftEntries = Object.entries(left);
  const rightRecord = right as Record<string, unknown>;
  if (leftEntries.length !== Object.keys(right).length) return false;
  return leftEntries.every(([key, value]) => Object.hasOwn(rightRecord, key) && sameValue(value, rightRecord[key]));
}

/** The order the joystick's four keys are captured in, with the word the prompt names. */
export const JOYSTICK_ORDER = ['up', 'down', 'left', 'right'] as const;
export type JoystickDirection = (typeof JOYSTICK_ORDER)[number];
export const DIRECTION_WORD: Readonly<Record<JoystickDirection, string>> = {
  up: '위', down: '아래', left: '왼쪽', right: '오른쪽',
};

/** Defaults for new markers (the brief of the overlay editor). */
export const DEFAULT_JOYSTICK_RADIUS = 0.15;
export const DEFAULT_SWIPE_MS = 300;
export const DEFAULT_WHEEL_DISTANCE = 0.2;

/** A marker's accessible name: its input and what it does, in the input screen's words. */
export function markerName(binding: Binding): string {
  const label = triggerLabel(binding.trigger);
  const words = actionWords(binding.action);
  // A mouse button or wheel trigger already names its kind; the description says where it acts.
  return binding.trigger.kind === 'mouseButton' || binding.trigger.kind === 'wheel'
    ? `${label} ${words.description}`
    : `${label} ${words.kind}`;
}
