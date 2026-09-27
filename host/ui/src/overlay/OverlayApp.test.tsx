// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The overlay page in a test DOM without layout: its box falls back to the 1024x768 window, so a
// logical point (x, y) is at client (1024x, 768y).
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AppIssue, AppSnapshot, Binding, InputProfile } from '../contracts';
import { fixtureBridge, overlayGallery, overlayProfile, overlaySnapshot } from '../fixtures';
import type { OverlayGalleryVariant } from '../fixtures';
import { mockActions } from '../test-actions';
import type { MockActions } from '../test-actions';
import { OverlayApp } from './OverlayApp';
import type { OverlaySeed } from './model';
import { OverlayWindow } from './OverlayWindow';

const WIDTH = 1024;
const HEIGHT = 768;

function variant(id: string): OverlayGalleryVariant {
  const found = overlayGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found;
}

const editing = variant('overlay-editing').snapshot;

function show(snapshot: AppSnapshot, seed?: OverlaySeed) {
  const actions = mockActions();
  const view = render(<OverlayApp snapshot={snapshot} actions={actions} seed={seed} />);
  return { actions, ...view };
}

function withProfile(snapshot: AppSnapshot, bindings: readonly Binding[]): AppSnapshot {
  const profiles = snapshot.input.profiles.map((profile) => (profile.id === overlayProfile.id ? { ...profile, bindings } : profile));
  return { ...snapshot, input: { ...snapshot.input, profiles } };
}

function pointer(element: Element, type: 'down' | 'move' | 'up', x: number, y: number) {
  const init = { pointerId: 1, button: 0, clientX: x * WIDTH, clientY: y * HEIGHT };
  if (type === 'down') fireEvent.pointerDown(element, init);
  else if (type === 'move') fireEvent.pointerMove(element, init);
  else fireEvent.pointerUp(element, init);
}

function clickAt(element: Element, x: number, y: number) {
  pointer(element, 'down', x, y);
  pointer(element, 'up', x, y);
}

function drag(element: Element, from: readonly [number, number], to: readonly [number, number]) {
  pointer(element, 'down', ...from);
  pointer(element, 'move', (from[0] + to[0]) / 2, (from[1] + to[1]) / 2);
  pointer(element, 'move', ...to);
  pointer(element, 'up', ...to);
}

function press(code: string) {
  fireEvent.keyDown(document.body, { code, key: code });
}

function surface(): HTMLElement {
  return screen.getByRole('button', { name: '새 표지 자리' });
}

function savedProfile(actions: MockActions): InputProfile {
  expect(actions.inputProfileSave).toHaveBeenCalledTimes(1);
  const profile = actions.inputProfileSave.mock.calls[0]?.[0];
  if (profile === undefined) throw new Error('No profile was saved.');
  return profile;
}

function binding(profile: InputProfile, id: string): Binding | undefined {
  return profile.bindings.find((item) => item.id === id);
}

describe('overlay page: when it draws', () => {
  it('draws nothing while hidden', () => {
    const stopped = { ...overlaySnapshot, guest: { ...overlaySnapshot.guest, state: 'stopped' as const } };
    const unmapped = { ...overlaySnapshot, input: { ...overlaySnapshot.input, activeId: null } };
    const switchedOff = { ...overlaySnapshot, input: { ...overlaySnapshot.input, overlayVisible: false } };
    for (const snapshot of [stopped, unmapped, switchedOff]) {
      const { container, unmount } = show(snapshot);
      expect(container).toBeEmptyDOMElement();
      unmount();
    }
  });

  it('shows every placed binding as a marker and nothing interactive outside the editor', () => {
    const { container } = show(variant('overlay-showing').snapshot);
    const root = container.querySelector('.ome-overlay');
    expect(root).toHaveClass('ome-overlay-showing');
    expect(screen.queryAllByRole('button')).toEqual([]);
    expect(screen.queryAllByRole('switch')).toEqual([]);
    expect(screen.queryAllByRole('textbox')).toEqual([]);
    expect(container.querySelectorAll('button, input, select, a, [tabindex]')).toHaveLength(0);
    // Tap, hold, joystick keys, swipe with its duration, fixed mouse button, wheel.
    for (const text of ['Q', 'E', 'W', 'A', 'S', 'D', 'Space', '240 ms', '오른쪽 버튼', '휠 위']) {
      expect(within(root as HTMLElement).getByText(text)).toBeInTheDocument();
    }
    expect(container.querySelector('.ome-overlay-marker-hold')).toHaveTextContent('E');
    // A mouse button acting where pressed and a pass-through binding have no place to draw.
    expect(screen.queryByText('왼쪽 버튼')).not.toBeInTheDocument();
    expect(screen.queryByText('Esc')).not.toBeInTheDocument();
    expect(screen.getByText('Q').closest('.ome-overlay-pin')).toHaveStyle({ left: '80%', top: '45%' });
  });

  it('draws nothing until the first snapshot arrives, then the markers', async () => {
    const { container } = render(<OverlayWindow bridge={fixtureBridge(overlaySnapshot)} />);
    expect(container).toBeEmptyDOMElement();
    await waitFor(() => { expect(container.querySelector('.ome-overlay-showing')).not.toBeNull(); });
    expect(screen.getByText('Q')).toBeInTheDocument();
  });

  it('dims the markers and names the suspend hotkey while mapping is suspended', () => {
    const suspended = variant('overlay-suspended').snapshot;
    const { container } = show({ ...suspended, input: { ...suspended.input, suspendHotkey: 'KeyP' } });
    expect(screen.getByText('매핑 일시 중지 (P)')).toBeInTheDocument();
    expect(container.querySelector('.ome-overlay-markers')).toHaveClass('ome-overlay-markers-suspended');
    expect(screen.queryAllByRole('button')).toEqual([]);
  });
});

describe('overlay editor: creating markers', () => {
  it('makes a tap from a click and a key, refusing a used key, and saves the whole profile once', async () => {
    const { actions } = show(editing);
    expect(screen.getByRole('button', { name: '저장' })).toBeDisabled();
    clickAt(surface(), 0.3, 0.25);
    expect(screen.getByRole('status')).toHaveTextContent('키를 누르십시오');
    press('KeyQ');
    expect(screen.getByRole('status')).toHaveTextContent('이미 쓰는 키입니다.');
    press('F12');
    expect(screen.getByRole('status')).toHaveTextContent('이미 쓰는 키입니다.');
    press('KeyF');
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'F 탭' })).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: '저장' }));
    const saved = savedProfile(actions);
    const created = saved.bindings[saved.bindings.length - 1];
    expect(created?.id).toMatch(/^binding-/);
    expect(saved).toEqual({
      ...overlayProfile,
      bindings: [
        ...overlayProfile.bindings,
        { id: created?.id, trigger: { kind: 'key', code: 'KeyF' }, action: { kind: 'tap', at: { x: 0.3, y: 0.25 }, hold: false } },
      ],
    });
    expect(actions.inputEditorToggle).not.toHaveBeenCalled();
  });

  it('cancels a key capture with Esc', () => {
    show(editing);
    clickAt(surface(), 0.3, 0.25);
    press('Escape');
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    press('KeyF');
    expect(screen.queryByRole('button', { name: 'F 탭' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '저장' })).toBeDisabled();
  });

  it('captures a joystick\'s four keys in the order up, down, left, right', async () => {
    const { actions } = show(editing);
    await userEvent.click(screen.getByRole('radio', { name: '조이스틱' }));
    clickAt(surface(), 0.35, 0.2);
    expect(screen.getByRole('status')).toHaveTextContent('위 키를 누르십시오');
    press('KeyI');
    expect(screen.getByRole('status')).toHaveTextContent('아래 키를 누르십시오');
    press('KeyI');
    expect(screen.getByRole('status')).toHaveTextContent('이미 쓰는 키입니다.');
    press('KeyK');
    expect(screen.getByRole('status')).toHaveTextContent('왼쪽 키를 누르십시오');
    press('KeyJ');
    press('KeyL');
    await userEvent.click(screen.getByRole('button', { name: '저장' }));
    const created = savedProfile(actions).bindings.at(-1);
    expect(created?.trigger).toEqual({ kind: 'keySet', up: 'KeyI', down: 'KeyK', left: 'KeyJ', right: 'KeyL' });
    expect(created?.action).toEqual({ kind: 'joystick', center: { x: 0.35, y: 0.2 }, radius: 0.15 });
  });

  it('draws a swipe by press, drag and release, then takes its key', async () => {
    const { actions } = show(editing);
    await userEvent.click(screen.getByRole('radio', { name: '스와이프' }));
    drag(surface(), [0.3, 0.25], [0.45, 0.25]);
    expect(screen.getByRole('status')).toHaveTextContent('키를 누르십시오');
    press('KeyR');
    await userEvent.click(screen.getByRole('button', { name: '저장' }));
    const created = savedProfile(actions).bindings.at(-1);
    expect(created?.trigger).toEqual({ kind: 'key', code: 'KeyR' });
    expect(created?.action).toEqual({ kind: 'swipe', from: { x: 0.3, y: 0.25 }, to: { x: 0.45, y: 0.25 }, durationMs: 300 });
  });

  it('picks a mouse button in the card, offering only the free ones', async () => {
    const { actions } = show(editing);
    await userEvent.click(screen.getByRole('radio', { name: '마우스 버튼' }));
    clickAt(surface(), 0.3, 0.25);
    const card = screen.getByRole('dialog', { name: '마우스 버튼' });
    expect(within(card).getByRole('radio', { name: '왼쪽' })).toBeDisabled();
    expect(within(card).getByRole('radio', { name: '오른쪽' })).toBeDisabled();
    await userEvent.click(within(card).getByRole('radio', { name: '가운데' }));
    expect(screen.getByRole('dialog', { name: '가운데 버튼 고정 자리' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '저장' }));
    const created = savedProfile(actions).bindings.at(-1);
    expect(created?.trigger).toEqual({ kind: 'mouseButton', button: 'middle' });
    expect(created?.action).toEqual({ kind: 'mouseTap', at: { x: 0.3, y: 0.25 } });
  });
});

describe('overlay editor: moving and changing markers', () => {
  it('moves a dragged marker and saves where it was dropped', async () => {
    const { actions } = show(editing);
    const skill = screen.getByRole('button', { name: 'Q 탭' });
    drag(skill, [0.8, 0.45], [0.6, 0.6]);
    expect(screen.getByRole('button', { name: 'Q 탭' })).toHaveStyle({ left: '60%', top: '60%' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '저장' }));
    expect(binding(savedProfile(actions), 'skill')?.action).toEqual({ kind: 'tap', at: { x: 0.6, y: 0.6 }, hold: false });
  });

  it('snaps a marker back when it is dropped onto another one', () => {
    show(editing);
    // The held tap E sits at (0.9, 0.62); this drop is about 26px from it.
    drag(screen.getByRole('button', { name: 'Q 탭' }), [0.8, 0.45], [0.88, 0.6]);
    expect(screen.getByRole('button', { name: 'Q 탭' })).toHaveStyle({ left: '80%', top: '45%' });
    expect(screen.getByRole('button', { name: '저장' })).toBeDisabled();
  });

  it('opens the card on a click without a drag; the card toggles 홀드 and Esc closes it', async () => {
    const { actions } = show(editing);
    clickAt(screen.getByRole('button', { name: 'E 홀드' }), 0.9, 0.62);
    const card = screen.getByRole('dialog', { name: 'E 홀드' });
    expect(screen.getByRole('button', { name: 'E 홀드' })).toHaveAttribute('data-selected', 'true');
    const hold = within(card).getByRole('switch', { name: '홀드' });
    expect(hold).toHaveAttribute('aria-checked', 'true');
    await userEvent.click(hold);
    expect(screen.getByRole('button', { name: 'E 탭' })).toBeInTheDocument();
    press('Escape');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '저장' }));
    expect(binding(savedProfile(actions), 'guard')?.action).toEqual({ kind: 'tap', at: { x: 0.9, y: 0.62 }, hold: false });
  });

  it('re-captures a key from the card and deletes a marker', async () => {
    const { actions } = show(editing, { selectedId: 'dash', cardOpen: true });
    const card = screen.getByRole('dialog', { name: 'Space 스와이프' });
    expect(within(card).getByRole('spinbutton', { name: '지속 시간' })).toHaveValue(240);
    await userEvent.click(within(card).getByRole('button', { name: '키 바꾸기' }));
    press('KeyE');
    expect(screen.getByRole('status')).toHaveTextContent('이미 쓰는 키입니다.');
    press('KeyG');
    expect(screen.getByRole('button', { name: 'G 스와이프' })).toBeInTheDocument();
    clickAt(screen.getByRole('button', { name: 'Q 탭' }), 0.8, 0.45);
    await userEvent.click(within(screen.getByRole('dialog', { name: 'Q 탭' })).getByRole('button', { name: '삭제' }));
    expect(screen.queryByRole('button', { name: 'Q 탭' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '저장' }));
    const saved = savedProfile(actions);
    expect(binding(saved, 'dash')?.trigger).toEqual({ kind: 'key', code: 'KeyG' });
    expect(binding(saved, 'skill')).toBeUndefined();
    // Bindings with nothing to draw survive the draft untouched.
    expect(binding(saved, 'touch')).toEqual(binding(overlayProfile, 'touch'));
    expect(binding(saved, 'back')).toEqual(binding(overlayProfile, 'back'));
  });

  it('marks the seeded selection with the selected border', () => {
    show(editing, { selectedId: 'skill' });
    expect(screen.getByRole('button', { name: 'Q 탭' })).toHaveAttribute('data-selected', 'true');
    expect(screen.getByRole('button', { name: 'E 홀드' })).not.toHaveAttribute('data-selected');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

describe('overlay editor: the tool bar', () => {
  it('puts the draft back with 되돌리기', async () => {
    const { actions } = show(editing);
    drag(screen.getByRole('button', { name: 'Q 탭' }), [0.8, 0.45], [0.6, 0.6]);
    expect(screen.getByRole('button', { name: '저장' })).toBeEnabled();
    await userEvent.click(screen.getByRole('button', { name: '되돌리기' }));
    expect(screen.getByRole('button', { name: 'Q 탭' })).toHaveStyle({ left: '80%', top: '45%' });
    expect(screen.getByRole('button', { name: '저장' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '되돌리기' })).toBeDisabled();
    expect(actions.inputProfileSave).not.toHaveBeenCalled();
  });

  it('ends at once when nothing changed', async () => {
    const { actions } = show(editing);
    await userEvent.click(screen.getByRole('button', { name: '편집 끝' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(actions.inputEditorToggle).toHaveBeenCalledTimes(1);
  });

  it('asks before ending with unsaved changes: 계속 편집 keeps the draft', async () => {
    const { actions } = show(editing);
    drag(screen.getByRole('button', { name: 'Q 탭' }), [0.8, 0.45], [0.6, 0.6]);
    await userEvent.click(screen.getByRole('button', { name: '편집 끝' }));
    const dialog = screen.getByRole('dialog', { name: '저장하지 않은 변경이 있습니다.' });
    expect(within(dialog).getByRole('button', { name: '계속 편집' })).toHaveFocus();
    await userEvent.click(within(dialog).getByRole('button', { name: '계속 편집' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Q 탭' })).toHaveStyle({ left: '60%', top: '60%' });
    expect(actions.inputEditorToggle).not.toHaveBeenCalled();
    expect(actions.inputProfileSave).not.toHaveBeenCalled();
  });

  it('버리고 끝 ends without saving', async () => {
    const { actions } = show(editing);
    drag(screen.getByRole('button', { name: 'Q 탭' }), [0.8, 0.45], [0.6, 0.6]);
    await userEvent.click(screen.getByRole('button', { name: '편집 끝' }));
    await userEvent.click(screen.getByRole('button', { name: '버리고 끝' }));
    expect(actions.inputProfileSave).not.toHaveBeenCalled();
    expect(actions.inputEditorToggle).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('저장하고 끝 saves, then ends once the snapshot carries the saved profile', async () => {
    const { actions, rerender } = show(editing);
    drag(screen.getByRole('button', { name: 'Q 탭' }), [0.8, 0.45], [0.6, 0.6]);
    await userEvent.click(screen.getByRole('button', { name: '편집 끝' }));
    await userEvent.click(screen.getByRole('button', { name: '저장하고 끝' }));
    const saved = savedProfile(actions);
    expect(binding(saved, 'skill')?.action).toEqual({ kind: 'tap', at: { x: 0.6, y: 0.6 }, hold: false });
    expect(actions.inputEditorToggle).not.toHaveBeenCalled();
    rerender(<OverlayApp snapshot={withProfile(editing, saved.bindings)} actions={actions} />);
    expect(actions.inputEditorToggle).toHaveBeenCalledTimes(1);
  });

  it('저장하고 끝 stays in the editor with the draft when the save comes back with an issue', async () => {
    const { actions, rerender } = show(editing);
    drag(screen.getByRole('button', { name: 'Q 탭' }), [0.8, 0.45], [0.6, 0.6]);
    await userEvent.click(screen.getByRole('button', { name: '편집 끝' }));
    await userEvent.click(screen.getByRole('button', { name: '저장하고 끝' }));
    const issue: AppIssue = { code: 'input_profile_invalid', message: '입력 프로필을 저장하지 못했습니다.', nextAction: '겹치는 키를 바꾼 뒤 다시 저장하십시오.' };
    rerender(<OverlayApp snapshot={{ ...editing, issue }} actions={actions} />);
    expect(screen.getByRole('alert')).toHaveTextContent('입력 프로필을 저장하지 못했습니다.');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Q 탭' })).toHaveStyle({ left: '60%', top: '60%' });
    expect(screen.getByRole('button', { name: '저장' })).toBeEnabled();
    expect(actions.inputEditorToggle).not.toHaveBeenCalled();
  });

  it('follows the saved profile while the draft is untouched', () => {
    const { rerender, actions } = show(editing);
    const moved = overlayProfile.bindings.map((item) => (item.id === 'skill' ? { ...item, action: { kind: 'tap' as const, at: { x: 0.7, y: 0.3 }, hold: false } } : item));
    rerender(<OverlayApp snapshot={withProfile(editing, moved)} actions={actions} />);
    expect(screen.getByRole('button', { name: 'Q 탭' })).toHaveStyle({ left: '70%', top: '30%' });
    expect(screen.getByRole('button', { name: '저장' })).toBeDisabled();
  });

  it('keeps a bundled preset read-only', async () => {
    const { actions } = show(variant('overlay-editing-bundled').snapshot);
    expect(screen.getByText('동봉 프리셋은 복제한 뒤 편집할 수 있습니다.')).toBeInTheDocument();
    for (const radio of screen.getAllByRole('radio')) expect(radio).toBeDisabled();
    expect(screen.queryByRole('button', { name: '새 표지 자리' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Space 탭' })).not.toBeInTheDocument();
    expect(screen.getByText('Space')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '저장' })).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: '편집 끝' }));
    expect(actions.inputEditorToggle).toHaveBeenCalledTimes(1);
    expect(actions.inputProfileSave).not.toHaveBeenCalled();
  });
});
