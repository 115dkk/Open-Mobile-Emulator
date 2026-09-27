// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { mainSnapshot } from '../fixtures';
import { mockActions } from '../test-actions';
import { Shell } from './Shell';

describe('shell: rail and body', () => {
  it('starts on the stage with the five rail items, status and version', () => {
    render(<Shell snapshot={mainSnapshot} actions={mockActions()} />);
    const rail = screen.getByRole('navigation', { name: '주 메뉴' });
    const items = within(rail).getAllByRole('button');
    expect(items.map((item) => item.textContent)).toEqual(['무대', '앱', '입력', '표시', '설정']);
    expect(within(rail).getByRole('button', { name: '무대' })).toHaveAttribute('aria-current', 'page');
    expect(within(rail).getByText('시작 가능')).toBeInTheDocument();
    expect(within(rail).getByText('0.1.0')).toBeInTheDocument();
    expect(within(screen.getByRole('main')).getByRole('heading', { level: 1, name: '무대' })).toBeInTheDocument();
  });

  it('switches the body when a rail item is chosen', async () => {
    render(<Shell snapshot={mainSnapshot} actions={mockActions()} />);
    const rail = screen.getByRole('navigation', { name: '주 메뉴' });
    for (const name of ['앱', '입력', '표시', '설정']) {
      await userEvent.click(within(rail).getByRole('button', { name }));
      expect(within(rail).getByRole('button', { name })).toHaveAttribute('aria-current', 'page');
      expect(within(screen.getByRole('main')).getByRole('heading', { level: 1, name })).toBeInTheDocument();
    }
    expect(within(rail).getByRole('button', { name: '무대' })).not.toHaveAttribute('aria-current');
  });

  it('names the running state next to the rail dot', () => {
    const running = { ...mainSnapshot, guest: { ...mainSnapshot.guest, state: 'running' as const } };
    render(<Shell snapshot={running} actions={mockActions()} />);
    expect(within(screen.getByRole('navigation', { name: '주 메뉴' })).getByText('실행 중')).toBeInTheDocument();
  });
});
