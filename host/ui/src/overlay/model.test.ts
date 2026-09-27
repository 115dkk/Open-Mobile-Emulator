// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { describe, expect, it } from 'vitest';
import type { AppSnapshot } from '../contracts';
import { overlayProfile, overlaySnapshot } from '../fixtures';
import { isKeyboardCode, judgeKey, markerName, overlayMode, sameValue } from './model';

function withInput(patch: Partial<AppSnapshot['input']>, guest: Partial<AppSnapshot['guest']> = {}): AppSnapshot {
  return {
    ...overlaySnapshot,
    guest: { ...overlaySnapshot.guest, ...guest },
    input: { ...overlaySnapshot.input, ...patch },
  };
}

describe('overlay mode', () => {
  it('is hidden unless the system runs, a profile is applied and markers are on or being edited', () => {
    expect(overlayMode(withInput({}, { state: 'stopped' }))).toBe('hidden');
    expect(overlayMode(withInput({}, { state: 'starting' }))).toBe('hidden');
    expect(overlayMode(withInput({}, { bootCompleted: false }))).toBe('hidden');
    expect(overlayMode(withInput({ activeId: null }))).toBe('hidden');
    expect(overlayMode(withInput({ activeId: 'no-such-profile' }))).toBe('hidden');
    expect(overlayMode(withInput({ overlayVisible: false }))).toBe('hidden');
  });

  it('is editing while the editor is on, even with markers switched off, and showing otherwise', () => {
    expect(overlayMode(withInput({ editing: true }))).toBe('editing');
    expect(overlayMode(withInput({ editing: true, overlayVisible: false }))).toBe('editing');
    expect(overlayMode(withInput({}))).toBe('showing');
    expect(overlayMode(withInput({ suspended: true }))).toBe('showing');
    expect(overlayMode(withInput({ editing: true }, { state: 'failed' }))).toBe('hidden');
  });
});

describe('the client mirror of the profile rules', () => {
  it('accepts exactly the keyboard codes ome-input accepts', () => {
    const accepted = ['KeyW', 'Digit1', 'F1', 'F12', 'F24', 'Numpad5', 'NumpadAdd', 'NumpadEnter', 'Space', 'ShiftLeft', 'ArrowUp', 'Escape', 'ContextMenu'];
    const refused = ['F25', 'F0', 'IntlRo', 'KeyWW', 'Keyw', 'Digit10', 'NumpadParenLeft', 'Fn', 'MediaPlayPause', ''];
    expect(accepted.filter((code) => !isKeyboardCode(code))).toEqual([]);
    expect(refused.filter(isKeyboardCode)).toEqual([]);
  });

  it('refuses a key another binding, the joystick being captured or the suspend hotkey uses', () => {
    const bindings = overlayProfile.bindings;
    expect(judgeKey('KeyF', bindings, null, 'F12')).toBe('ok');
    expect(judgeKey('KeyQ', bindings, null, 'F12')).toBe('taken');
    expect(judgeKey('KeyW', bindings, null, 'F12')).toBe('taken');
    expect(judgeKey('Escape', bindings, null, 'F12')).toBe('taken');
    expect(judgeKey('KeyQ', bindings, 'skill', 'F12')).toBe('ok');
    expect(judgeKey('KeyW', bindings, 'move', 'F12')).toBe('ok');
    expect(judgeKey('F12', bindings, null, 'F12')).toBe('taken');
    expect(judgeKey('KeyI', bindings, null, 'F12', ['KeyI'])).toBe('taken');
    expect(judgeKey('IntlRo', bindings, null, 'F12')).toBe('invalid');
  });

  it('compares contract values by structure, not key order', () => {
    expect(sameValue({ kind: 'tap', at: { x: 0.1, y: 0.2 }, hold: false }, { hold: false, at: { y: 0.2, x: 0.1 }, kind: 'tap' })).toBe(true);
    expect(sameValue([{ a: 1 }], [{ a: 2 }])).toBe(false);
    expect(sameValue({ at: null }, { at: { x: 0, y: 0 } })).toBe(false);
    expect(sameValue([1, 2], [1, 2, 3])).toBe(false);
  });

  it('names a marker by its input and what it does', () => {
    const names = overlayProfile.bindings.map(markerName);
    expect(names).toEqual(['W A S D 조이스틱', 'Q 탭', 'E 홀드', 'Space 스와이프', '오른쪽 버튼 고정 자리', '왼쪽 버튼 누른 자리', '휠 위 끌기', 'Esc 통과']);
  });
});
