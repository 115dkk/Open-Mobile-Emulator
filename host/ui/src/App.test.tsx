// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { App } from './App';
import { blockedGallery, fixtureBridge, mainSnapshot, sampleSnapshot } from './fixtures';

describe('application root', () => {
  it('opens the first-run wizard while the phase is wizard', async () => {
    render(<App bridge={fixtureBridge()} />);
    expect(await screen.findByRole('heading', { level: 1, name: '호스트 점검' })).toBeInTheDocument();
    expect(screen.queryByRole('navigation', { name: '주 메뉴' })).toBeNull();
  });

  it('shows the issue returned by a failed command', async () => {
    const issue = {
      code: 'host_check_failed',
      message: '호스트를 확인하지 못했습니다.',
      nextAction: '앱을 다시 시작한 뒤 다시 확인하십시오.',
    };
    render(<App bridge={fixtureBridge(sampleSnapshot, issue)} />);
    await screen.findByRole('heading', { level: 1, name: '호스트 점검' });
    await userEvent.click(screen.getByRole('button', { name: '다시 확인' }));
    expect(await screen.findByText(issue.message)).toBeInTheDocument();
    expect(screen.getByText(issue.nextAction)).toBeInTheDocument();
  });

  it('draws the blocked screen whenever Rust reports a blocker', async () => {
    const blocked = blockedGallery[0]?.snapshot ?? sampleSnapshot;
    render(<App bridge={fixtureBridge(blocked)} />);
    expect(await screen.findByRole('heading', { level: 1, name: '이 PC의 BIOS 설정에서 가상화가 꺼져 있습니다.' })).toBeInTheDocument();
  });

  it('opens the rail shell after the wizard', async () => {
    render(<App bridge={fixtureBridge(mainSnapshot)} />);
    expect(await screen.findByRole('navigation', { name: '주 메뉴' })).toBeInTheDocument();
  });
});
