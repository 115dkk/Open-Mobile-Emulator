// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AppSnapshot } from '../contracts';
import { blockedGallery } from '../fixtures';
import { mockActions } from '../test-actions';
import { BlockedScreen } from './BlockedScreen';

function variant(id: string): AppSnapshot {
  const found = blockedGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found.snapshot;
}

function show(snapshot: AppSnapshot) {
  const actions = mockActions();
  render(<BlockedScreen snapshot={snapshot} actions={actions} />);
  return actions;
}

describe('blocked screen (S8)', () => {
  it('explains BIOS virtualization and checks again', async () => {
    const actions = show(variant('blocked-virtualization'));
    expect(screen.getByRole('heading', { level: 1, name: '이 PC의 BIOS 설정에서 가상화가 꺼져 있습니다.' })).toBeInTheDocument();
    expect(screen.getByText('PC를 다시 시작해 BIOS 설정에서 가상화(Intel VT-x 또는 AMD-V)를 켠 뒤 이 앱을 다시 여십시오.')).toBeInTheDocument();
    expect(screen.getByText('BIOS 메뉴 이름은 PC 제조사마다 다릅니다.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '다시 확인' }));
    expect(actions.hostCheckRefresh).toHaveBeenCalledOnce();
  });

  it('asks for a reinstall when the virtual machine components are missing', async () => {
    const actions = show(variant('blocked-qemu'));
    expect(screen.getByRole('heading', { level: 1, name: '가상 머신 구성 요소를 찾을 수 없습니다.' })).toBeInTheDocument();
    expect(screen.getByText('앱을 다시 설치하십시오.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '다시 확인' }));
    expect(actions.hostCheckRefresh).toHaveBeenCalledOnce();
  });

  it('goes to the consent screen for the hypervisor and back with Esc', async () => {
    const actions = show(variant('blocked-hypervisor'));
    expect(screen.getByRole('heading', { level: 1, name: '하이퍼바이저가 꺼져 있습니다.' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '활성화' }));
    expect(actions.whpxEnable).not.toHaveBeenCalled();
    expect(screen.getByRole('heading', { level: 1, name: '하이퍼바이저 활성화' })).toBeInTheDocument();
    expect(screen.getByText('Esc 키로도 나갈 수 있습니다.')).toBeInTheDocument();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.getByRole('heading', { level: 1, name: '하이퍼바이저가 꺼져 있습니다.' })).toBeInTheDocument();
  });

  it('enables the hypervisor only from the consent screen', async () => {
    const actions = show(variant('blocked-hypervisor'));
    await userEvent.click(screen.getByRole('button', { name: '활성화' }));
    await userEvent.click(screen.getByRole('button', { name: '활성화' }));
    expect(actions.whpxEnable).toHaveBeenCalledOnce();
    await userEvent.click(screen.getByRole('button', { name: '지금은 건너뛰기' }));
    expect(screen.getByRole('heading', { level: 1, name: '하이퍼바이저가 꺼져 있습니다.' })).toBeInTheDocument();
  });

  it('shows the issue above the cause', () => {
    const snapshot = variant('blocked-virtualization');
    show({ ...snapshot, issue: { code: 'x', message: '호스트를 확인하지 못했습니다.', nextAction: null } });
    expect(screen.getByRole('alert')).toHaveTextContent('호스트를 확인하지 못했습니다.');
  });
});
