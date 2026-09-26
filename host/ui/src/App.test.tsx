// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { createElement } from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { App } from './App';
import { fixtureBridge, sampleSnapshot } from './fixtures';

describe('minimal application shell', () => {
  it('renders the guest state supplied by the bridge', async () => {
    render(createElement(App, { bridge: fixtureBridge() }));
    expect(await screen.findByText('stopped')).toBeInTheDocument();
  });

  it('shows the issue returned by a failed command', async () => {
    const issue = {
      code: 'guest_start_failed',
      message: '게스트를 시작하지 못했습니다.',
      nextAction: '다시 시도하십시오.',
    };
    render(createElement(App, { bridge: fixtureBridge(sampleSnapshot, issue) }));
    await screen.findByText('stopped');
    await userEvent.click(screen.getByRole('button', { name: '시작' }));
    expect(await screen.findByText(issue.message)).toBeInTheDocument();
    expect(screen.getByText(issue.nextAction)).toBeInTheDocument();
  });
});
