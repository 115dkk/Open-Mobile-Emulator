// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { AppSnapshot } from '../contracts';
import { inputGallery, railRunningSnapshot } from '../fixtures';
import { mockActions } from '../test-actions';
import { InputScreen } from './InputScreen';
import { Shell } from './Shell';

function variant(id: string): AppSnapshot {
  const found = inputGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found.snapshot;
}

function show(snapshot: AppSnapshot, navigate?: (screen: 'stage') => void) {
  const actions = mockActions();
  render(<InputScreen snapshot={snapshot} actions={actions} navigate={navigate} />);
  return actions;
}

function cells(row: HTMLElement): string[] {
  return within(row).getAllByRole('cell').map((cell) => cell.textContent);
}

describe('input screen (S4): profile list', () => {
  it('groups bundled presets and own profiles with target app, binding count and the auto-apply mark', () => {
    show(variant('input-running'));
    expect(screen.getByRole('heading', { level: 1, name: '입력' })).toBeInTheDocument();
    const bundled = within(screen.getByRole('region', { name: '동봉 프리셋' })).getAllByRole('button');
    expect(bundled).toHaveLength(1);
    expect(bundled[0]).toHaveTextContent('샘플 앱 A 기본 입력');
    expect(bundled[0]).toHaveTextContent('샘플 앱 A · 표지 3개');
    const own = within(screen.getByRole('region', { name: '내 프로필' })).getAllByRole('button');
    expect(own.map((row) => row.querySelector('.ome-input-profile-name')?.textContent)).toEqual(['내 프로필 1', '내 프로필 2']);
    expect(own[0]).toHaveTextContent('샘플 앱 B · 표지 8개');
    expect(own[0]).toHaveTextContent('자동 적용');
    expect(own[0]).toHaveAttribute('aria-current', 'true');
    expect(own[1]).toHaveTextContent('표지 1개');
    expect(own[1]).not.toHaveTextContent('자동 적용');
  });

  it('applies another profile when its row is chosen, and leaves the applied one alone', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: /내 프로필 1/ }));
    expect(actions.inputProfileSelect).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: /내 프로필 2/ }));
    expect(actions.inputProfileSelect).toHaveBeenCalledWith('user-2');
  });

  it('switches automatic application', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('switch', { name: '자동 적용' }));
    expect(actions.inputAutoApplySet).toHaveBeenCalledWith(false);
  });

  it('creates 내 프로필 3 with no bindings in the current screen aspect', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: '새 프로필' }));
    const profile = actions.inputProfileSave.mock.calls[0]?.[0];
    expect(profile?.id).toMatch(/^profile-/);
    expect(profile).toEqual({
      id: profile?.id,
      name: '내 프로필 3',
      bundled: false,
      targetPackage: null,
      referenceAspect: { width: 1920, height: 1080 },
      anchor: 'center',
      bindings: [],
    });
  });

  it('says there is no profile yet and still offers 새 프로필', async () => {
    const actions = show(variant('input-empty'));
    expect(screen.getByText('입력 프로필이 없습니다.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '새 프로필' }));
    expect(actions.inputProfileSave).toHaveBeenCalledWith(expect.objectContaining({ name: '내 프로필 1', bindings: [] }));
  });

  it('asks for a profile when none is applied', () => {
    show({ ...railRunningSnapshot, input: { ...railRunningSnapshot.input, activeId: null } });
    expect(screen.getByText('왼쪽 목록에서 프로필을 고르십시오.')).toBeInTheDocument();
  });

  it('shows the issue above the screen', () => {
    show({ ...variant('input-running'), issue: { code: 'x', message: '프로필을 저장하지 못했습니다.', nextAction: '같은 키가 두 번 쓰였는지 확인하십시오.' } });
    expect(screen.getByRole('alert')).toHaveTextContent('프로필을 저장하지 못했습니다.');
    expect(screen.getByRole('alert')).toHaveTextContent('같은 키가 두 번 쓰였는지 확인하십시오.');
  });
});

describe('input screen (S4): the applied profile', () => {
  it('lists the bindings in the words of the screen', () => {
    show(variant('input-running'));
    expect(screen.getByRole('heading', { level: 2, name: '내 프로필 1' })).toBeInTheDocument();
    expect(screen.getByText('대상 앱 샘플 앱 B')).toBeInTheDocument();
    expect(screen.getByText('화면 위의 자리를 클릭하고 설정할 키를 누르십시오.')).toBeInTheDocument();
    expect(screen.getByText('현재 화면')).toBeInTheDocument();
    const table = screen.getByRole('table');
    expect(within(table).getAllByRole('columnheader').map((cell) => cell.textContent)).toEqual(['입력', '동작 종류', '자리', '설명', '삭제']);
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows.map((row) => cells(row).slice(0, 4))).toEqual([
      ['W A S D', '조이스틱', '16%, 72%, 반지름 16%', '방향키 넷을 가상 스틱으로'],
      ['Q', '탭', '84%, 42%', '누르면 터치'],
      ['E', '홀드', '90%, 70%', '누르는 동안 터치 유지'],
      ['Space', '스와이프', '62%, 80% → 76%, 80%', '끌기'],
      ['오른쪽 버튼', '마우스 버튼', '95%, 8%', '고정 자리'],
      ['왼쪽 버튼', '마우스 버튼', '', '누른 자리'],
      ['휠 위', '휠', '50%, 40%', '끌기'],
      ['Esc', '통과', '', '운영체제로 그대로 보냄'],
    ]);
  });

  it('labels the preview 화면 스냅샷 while the system is off and has no stage editing', () => {
    show(variant('input-stopped'), vi.fn());
    expect(screen.getByText('화면 스냅샷')).toBeInTheDocument();
    expect(screen.queryByText('현재 화면')).toBeNull();
    expect(screen.queryByRole('button', { name: '화면에서 편집' })).toBeNull();
    expect(screen.getByRole('table')).toBeInTheDocument();
  });

  it('places a tap where the preview was clicked, on the next key', async () => {
    const actions = show(variant('input-stopped'));
    await userEvent.click(screen.getByRole('button', { name: '새 표지 자리' }));
    fireEvent.keyDown(document, { key: 'f', code: 'KeyF' });
    const [profileId, binding] = actions.inputBindingUpsert.mock.calls[0] ?? [];
    expect(profileId).toBe('user-1');
    expect(binding?.id).toMatch(/^binding-/);
    expect(binding).toEqual({
      id: binding?.id,
      trigger: { kind: 'key', code: 'KeyF' },
      action: { kind: 'tap', at: { x: 0.5, y: 0.5 }, hold: false },
    });
  });

  it('drops a picked place on Esc', async () => {
    const actions = show(variant('input-stopped'));
    await userEvent.click(screen.getByRole('button', { name: '새 표지 자리' }));
    fireEvent.keyDown(document, { key: 'Escape', code: 'Escape' });
    fireEvent.keyDown(document, { key: 'f', code: 'KeyF' });
    expect(actions.inputBindingUpsert).not.toHaveBeenCalled();
  });

  it('removes one binding', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: 'Q 표지 삭제' }));
    expect(actions.inputBindingRemove).toHaveBeenCalledWith('user-1', 'skill');
  });

  it('duplicates as an own profile', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: '복제' }));
    const saved = actions.inputProfileSave.mock.calls[0]?.[0];
    expect(saved).toMatchObject({ name: '내 프로필 1 사본', bundled: false, targetPackage: 'com.example.sample.b' });
    expect(saved?.id).not.toBe('user-1');
    expect(saved?.bindings).toHaveLength(8);
  });

  it('renames through a dialog', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: '이름 바꾸기' }));
    const dialog = screen.getByRole('dialog', { name: '프로필 이름을 바꿉니다.' });
    const field = within(dialog).getByRole('textbox', { name: '이름' });
    await userEvent.clear(field);
    await userEvent.type(field, '전투');
    await userEvent.click(within(dialog).getByRole('button', { name: '저장' }));
    expect(actions.inputProfileSave).toHaveBeenCalledWith(expect.objectContaining({ id: 'user-1', name: '전투' }));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('picks the target app from the installed apps', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: '대상 앱 지정' }));
    const dialog = screen.getByRole('dialog', { name: '대상 앱을 지정합니다.' });
    const select = within(dialog).getByRole('combobox', { name: '대상 앱' });
    expect(within(select).getAllByRole('option').map((option) => option.textContent)).toEqual([
      '지정 안 함', '샘플 앱 A (com.example.sample.a)', '샘플 앱 B (com.example.sample.b)', '샘플 앱 C (com.example.sample.c)',
    ]);
    await userEvent.selectOptions(select, 'com.example.sample.c');
    await userEvent.click(within(dialog).getByRole('button', { name: '지정' }));
    expect(actions.inputProfileSave).toHaveBeenCalledWith(expect.objectContaining({ id: 'user-1', targetPackage: 'com.example.sample.c' }));
  });

  it('clears the target app with 지정 안 함', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: '대상 앱 지정' }));
    const dialog = screen.getByRole('dialog', { name: '대상 앱을 지정합니다.' });
    await userEvent.selectOptions(within(dialog).getByRole('combobox', { name: '대상 앱' }), '');
    await userEvent.click(within(dialog).getByRole('button', { name: '지정' }));
    expect(actions.inputProfileSave).toHaveBeenCalledWith(expect.objectContaining({ id: 'user-1', targetPackage: null }));
  });

  it('deletes after the one-line confirmation', async () => {
    const actions = show(variant('input-running'));
    await userEvent.click(screen.getByRole('button', { name: '삭제' }));
    const dialog = screen.getByRole('dialog', { name: '내 프로필 1 프로필을 삭제합니다.' });
    await userEvent.click(within(dialog).getByRole('button', { name: '삭제' }));
    expect(actions.inputProfileDelete).toHaveBeenCalledWith('user-1');
  });

  it('opens the stage editor: editing on, then the stage', async () => {
    const navigate = vi.fn();
    const actions = show(variant('input-running'), navigate);
    await userEvent.click(screen.getByRole('button', { name: '화면에서 편집' }));
    expect(actions.inputEditorToggle).toHaveBeenCalledOnce();
    expect(navigate).toHaveBeenCalledWith('stage');
  });

  it('does not switch editing off when it is already on', async () => {
    const navigate = vi.fn();
    const snapshot = variant('input-running');
    const actions = show({ ...snapshot, input: { ...snapshot.input, editing: true } }, navigate);
    await userEvent.click(screen.getByRole('button', { name: '화면에서 편집' }));
    expect(actions.inputEditorToggle).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith('stage');
  });

  it('offers only 복제 on a bundled preset', async () => {
    const actions = show(variant('input-bundled'), vi.fn());
    expect(screen.getByRole('heading', { level: 2, name: '샘플 앱 A 기본 입력' })).toBeInTheDocument();
    for (const name of ['이름 바꾸기', '대상 앱 지정', '삭제', '화면에서 편집', '새 표지 자리']) {
      expect(screen.queryByRole('button', { name })).toBeNull();
    }
    expect(screen.queryByText('화면 위의 자리를 클릭하고 설정할 키를 누르십시오.')).toBeNull();
    expect(within(screen.getByRole('table')).getAllByRole('columnheader')).toHaveLength(4);
    await userEvent.click(screen.getByRole('button', { name: '복제' }));
    expect(actions.inputProfileSave).toHaveBeenCalledWith(expect.objectContaining({ name: '샘플 앱 A 기본 입력 사본', bundled: false }));
  });
});

describe('input screen (S4) inside the shell', () => {
  it('moves the rail to the stage from 화면에서 편집', async () => {
    const actions = mockActions();
    render(<Shell snapshot={railRunningSnapshot} actions={actions} />);
    const rail = screen.getByRole('navigation', { name: '주 메뉴' });
    await userEvent.click(within(rail).getByRole('button', { name: '입력' }));
    await userEvent.click(screen.getByRole('button', { name: '화면에서 편집' }));
    expect(actions.inputEditorToggle).toHaveBeenCalledOnce();
    expect(within(rail).getByRole('button', { name: '화면' })).toHaveAttribute('aria-current', 'page');
  });
});
