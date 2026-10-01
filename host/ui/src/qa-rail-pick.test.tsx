// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { Gallery } from './qa-gallery';

function open(state: string) {
  window.history.replaceState(null, '', `/?state=${state}`);
  render(<Gallery />);
}

describe('gallery: rail screens', () => {
  afterEach(() => { window.history.replaceState(null, '', '/'); });

  it('opens the apps screen for an apps variant', async () => {
    open('apps-running');
    const main = await screen.findByRole('main');
    expect(await within(main).findByRole('heading', { level: 1, name: '앱' })).toBeInTheDocument();
    expect(within(screen.getByRole('navigation', { name: '주 메뉴' })).getByRole('button', { name: '앱' }))
      .toHaveAttribute('aria-current', 'page');
  });

  it('stays on the stage for a stage variant', async () => {
    open('stage-running');
    const main = await screen.findByRole('main');
    expect(await within(main).findByRole('heading', { level: 1, name: '화면' })).toBeInTheDocument();
  });
});
