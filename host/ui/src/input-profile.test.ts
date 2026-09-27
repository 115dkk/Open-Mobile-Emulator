// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { describe, expect, it } from 'vitest';
import { actionWords, keyLabel, newId, nextProfileName, positionLabel, triggerLabel } from './input-profile';

describe('input profile words', () => {
  it('names keys as their caps read', () => {
    expect(['KeyW', 'Digit1', 'Numpad4', 'Space', 'Escape', 'ShiftLeft', 'ArrowUp', 'F12', 'Backquote', 'IntlRo'].map(keyLabel))
      .toEqual(['W', '1', 'Num 4', 'Space', 'Esc', 'Shift', '↑', 'F12', '`', 'IntlRo']);
  });

  it('names triggers in the user words', () => {
    expect(triggerLabel({ kind: 'keySet', up: 'KeyW', down: 'KeyS', left: 'KeyA', right: 'KeyD' })).toBe('W A S D');
    expect(triggerLabel({ kind: 'keySet', up: 'ArrowUp', down: 'ArrowDown', left: 'ArrowLeft', right: 'ArrowRight' })).toBe('방향키');
    expect(triggerLabel({ kind: 'mouseButton', button: 'middle' })).toBe('가운데 버튼');
    expect(triggerLabel({ kind: 'wheel', direction: 'down' })).toBe('휠 아래');
  });

  it('describes each action kind as M2-SCREENS.md 4 does', () => {
    const at = { x: 0.25, y: 0.5 };
    expect(actionWords({ kind: 'tap', at, hold: false })).toEqual({ kind: '탭', description: '누르면 터치' });
    expect(actionWords({ kind: 'tap', at, hold: true })).toEqual({ kind: '홀드', description: '누르는 동안 터치 유지' });
    expect(actionWords({ kind: 'joystick', center: at, radius: 0.2 })).toEqual({ kind: '조이스틱', description: '방향키 넷을 가상 스틱으로' });
    expect(actionWords({ kind: 'swipe', from: at, to: at, durationMs: 200 })).toEqual({ kind: '스와이프', description: '끌기' });
    expect(actionWords({ kind: 'mouseTap', at: null })).toEqual({ kind: '마우스 버튼', description: '누른 자리' });
    expect(actionWords({ kind: 'mouseTap', at })).toEqual({ kind: '마우스 버튼', description: '고정 자리' });
    expect(actionWords({ kind: 'wheelSwipe', at, distance: 0.1 })).toEqual({ kind: '휠', description: '끌기' });
    expect(actionWords({ kind: 'passThrough' })).toEqual({ kind: '통과', description: '운영체제로 그대로 보냄' });
  });

  it('writes places as screen percentages', () => {
    expect(positionLabel({ kind: 'tap', at: { x: 0.424, y: 0.705 }, hold: false })).toBe('42%, 71%');
    expect(positionLabel({ kind: 'swipe', from: { x: 0.65, y: 0.5 }, to: { x: 0.35, y: 0.5 }, durationMs: 240 })).toBe('65%, 50% → 35%, 50%');
    expect(positionLabel({ kind: 'mouseTap', at: null })).toBeNull();
    expect(positionLabel({ kind: 'passThrough' })).toBeNull();
  });

  it('makes fresh ids and the next free profile name', () => {
    const first = newId('binding');
    expect(first).toMatch(/^binding-./);
    expect(newId('binding')).not.toBe(first);
    expect(nextProfileName([])).toBe('내 프로필 1');
    expect(nextProfileName(['내 프로필 1', '내 프로필 3'])).toBe('내 프로필 2');
  });
});
