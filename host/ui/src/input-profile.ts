// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Words for input profiles (M2-SCREENS.md 4) and the ids a new profile or binding needs. Pure
// mapping from contract values; Rust validates every profile the webview sends.
import type { BindingAction, LogicalPoint, MouseButton, Trigger, WheelDirection } from './contracts';

const NAMED_KEYS: Readonly<Record<string, string>> = {
  Space: 'Space',
  Enter: 'Enter',
  NumpadEnter: 'Num Enter',
  Escape: 'Esc',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'Page Up',
  PageDown: 'Page Down',
  CapsLock: 'Caps Lock',
  ShiftLeft: 'Shift',
  ShiftRight: '오른쪽 Shift',
  ControlLeft: 'Ctrl',
  ControlRight: '오른쪽 Ctrl',
  AltLeft: 'Alt',
  AltRight: '오른쪽 Alt',
  MetaLeft: 'Windows',
  MetaRight: '오른쪽 Windows',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: '\'',
  Backquote: '`',
  Comma: ',',
  Period: '.',
  Slash: '/',
};

/** A W3C `KeyboardEvent.code` as the key cap reads: `KeyW` is `W`, `Digit1` is `1`, `Space` stays. */
export function keyLabel(code: string): string {
  const named = NAMED_KEYS[code];
  if (named !== undefined) return named;
  const letter = /^Key([A-Z])$/u.exec(code);
  if (letter?.[1] !== undefined) return letter[1];
  const digit = /^Digit(\d)$/u.exec(code);
  if (digit?.[1] !== undefined) return digit[1];
  const numpad = /^Numpad(\d)$/u.exec(code);
  if (numpad?.[1] !== undefined) return `Num ${numpad[1]}`;
  return code;
}

const MOUSE_BUTTON: Readonly<Record<MouseButton, string>> = {
  left: '왼쪽 버튼',
  right: '오른쪽 버튼',
  middle: '가운데 버튼',
};

const WHEEL: Readonly<Record<WheelDirection, string>> = {
  up: '휠 위',
  down: '휠 아래',
};

/** The input column: a key, the four joystick keys, a mouse button or a wheel direction. */
export function triggerLabel(trigger: Trigger): string {
  switch (trigger.kind) {
    case 'key': return keyLabel(trigger.code);
    case 'mouseButton': return MOUSE_BUTTON[trigger.button];
    case 'wheel': return WHEEL[trigger.direction];
    case 'keySet': {
      if (trigger.up === 'ArrowUp' && trigger.left === 'ArrowLeft' && trigger.down === 'ArrowDown' && trigger.right === 'ArrowRight') {
        return '방향키';
      }
      return [trigger.up, trigger.left, trigger.down, trigger.right].map(keyLabel).join(' ');
    }
  }
}

export interface ActionWords {
  /** The kind column: 탭, 홀드, 조이스틱, 스와이프, 마우스 버튼, 휠, 통과. */
  readonly kind: string;
  /** The description column, in the words of M2-SCREENS.md 4. */
  readonly description: string;
}

export function actionWords(action: BindingAction): ActionWords {
  switch (action.kind) {
    case 'tap':
      return action.hold ? { kind: '홀드', description: '누르는 동안 터치 유지' } : { kind: '탭', description: '누르면 터치' };
    case 'joystick': return { kind: '조이스틱', description: '방향키 넷을 가상 스틱으로' };
    case 'swipe': return { kind: '스와이프', description: '끌기' };
    case 'mouseTap': return { kind: '마우스 버튼', description: action.at === null ? '누른 자리' : '고정 자리' };
    case 'wheelSwipe': return { kind: '휠', description: '끌기' };
    case 'passThrough': return { kind: '통과', description: '운영체제로 그대로 보냄' };
  }
}

function percent(value: number): string {
  return `${String(Math.round(value * 100))}%`;
}

/** A logical point (0..1 on both axes) as screen percentages: `42%, 71%`. */
export function pointLabel(point: LogicalPoint): string {
  return `${percent(point.x)}, ${percent(point.y)}`;
}

/** The position column. Null when the binding has no fixed place (누른 자리, 통과). */
export function positionLabel(action: BindingAction): string | null {
  switch (action.kind) {
    case 'tap': return pointLabel(action.at);
    case 'swipe': return `${pointLabel(action.from)} → ${pointLabel(action.to)}`;
    case 'joystick': return `${pointLabel(action.center)}, 반지름 ${percent(action.radius)}`;
    case 'mouseTap': return action.at === null ? null : pointLabel(action.at);
    case 'wheelSwipe': return pointLabel(action.at);
    case 'passThrough': return null;
  }
}

/** A fresh id for a profile or binding the webview creates. Rust checks uniqueness. */
export function newId(prefix: string): string {
  const random = typeof crypto.randomUUID === 'function'
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
  return `${prefix}-${random}`;
}

/** `내 프로필 N` with the smallest N no profile uses yet. */
export function nextProfileName(names: readonly string[]): string {
  const taken = new Set(names);
  let index = 1;
  while (taken.has(`내 프로필 ${String(index)}`)) index += 1;
  return `내 프로필 ${String(index)}`;
}
