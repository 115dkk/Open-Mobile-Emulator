// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AppSnapshot } from '../contracts';
import { displayGallery } from '../fixtures';
import { mockActions } from '../test-actions';
import { DisplayScreen } from './DisplayScreen';

function variant(id: string): AppSnapshot {
  const found = displayGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found.snapshot;
}

function show(snapshot: AppSnapshot) {
  const actions = mockActions();
  render(<DisplayScreen snapshot={snapshot} actions={actions} />);
  return actions;
}

function options(group: HTMLElement): string[] {
  return within(group).getAllByRole('radio').map((option) => option.textContent);
}

describe('display screen (S5): resolution', () => {
  it('shows the current resolution and one card per preset with the sentence Rust chose', () => {
    show(variant('display-basic'));
    expect(screen.getByRole('heading', { level: 1, name: '표시' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 2, name: '해상도' })).toBeInTheDocument();
    expect(screen.getByText('1920x1080 · 240 DPI · 가로')).toBeInTheDocument();

    const hd = screen.getByRole('group', { name: '1280x720' });
    expect(hd).toHaveTextContent('160 DPI · 가로');
    expect(within(hd).getByText('즉시 적용할 수 있습니다.')).toBeInTheDocument();
    const full = screen.getByRole('group', { name: '1920x1080' });
    expect(within(full).getByText('적용됨')).toBeInTheDocument();
    expect(within(full).queryByRole('button', { name: '적용' })).toBeNull();
    const portrait = screen.getByRole('group', { name: '720x1280' });
    expect(portrait).toHaveTextContent('160 DPI · 세로');
    expect(within(portrait).getByText('방향을 바꾸려면 운영체제를 다시 시작해야 합니다.')).toBeInTheDocument();
  });

  it('applies a preset', async () => {
    const actions = show(variant('display-basic'));
    await userEvent.click(within(screen.getByRole('group', { name: '720x1280' })).getByRole('button', { name: '적용' }));
    expect(actions.displayPresetApply).toHaveBeenCalledWith('portrait-720');
  });

  it('applies a custom size and density once all three are filled', async () => {
    const actions = show(variant('display-basic'));
    const custom = screen.getByRole('group', { name: '사용자 지정' });
    const apply = within(custom).getByRole('button', { name: '적용' });
    expect(apply).toBeDisabled();
    expect(within(custom).getAllByText('640~7680')).toHaveLength(2);
    expect(within(custom).getByText('120~640')).toBeInTheDocument();
    await userEvent.type(within(custom).getByRole('spinbutton', { name: '폭' }), '2560');
    await userEvent.type(within(custom).getByRole('spinbutton', { name: '높이' }), '1440');
    await userEvent.type(within(custom).getByRole('spinbutton', { name: 'DPI' }), '320');
    await userEvent.click(apply);
    expect(actions.displayCustomApply).toHaveBeenCalledWith({ width: 2560, height: 1440 }, 320);
  });

  it('marks an applied custom size and starts its fields from it', () => {
    show(variant('display-custom'));
    expect(screen.getByText('2560x1440 · 320 DPI · 가로')).toBeInTheDocument();
    const custom = screen.getByRole('group', { name: '사용자 지정' });
    expect(within(custom).getByText('적용됨')).toBeInTheDocument();
    expect(within(custom).getByRole('spinbutton', { name: '폭' })).toHaveValue(2560);
    expect(screen.getByRole('group', { name: '1920x1080' })).not.toHaveTextContent('적용됨');
  });
});

describe('display screen (S5): refresh rate, vertical sync, scaling', () => {
  it('has no refresh-rate or vertical-sync rows when the virtual machine lacks them', () => {
    show(variant('display-basic'));
    expect(screen.queryByRole('heading', { name: '주사율' })).toBeNull();
    expect(screen.queryByRole('heading', { name: '수직 동기화' })).toBeNull();
    expect(screen.queryByRole('radiogroup', { name: '주사율' })).toBeNull();
  });

  it('lists the refresh rates with the default marked and sends the chosen one', async () => {
    const actions = show(variant('display-refresh'));
    const refresh = screen.getByRole('region', { name: '주사율' });
    expect(within(refresh).getByText('다시 시작해야 적용됩니다.')).toBeInTheDocument();
    const group = within(refresh).getByRole('radiogroup', { name: '주사율' });
    expect(options(group)).toEqual(['60 Hz (기본)', '75 Hz', '90 Hz', '120 Hz', '144 Hz', '사용자 지정']);
    expect(within(group).getByRole('radio', { name: '120 Hz' })).toHaveAttribute('aria-checked', 'true');
    await userEvent.click(within(group).getByRole('radio', { name: '144 Hz' }));
    expect(actions.displayRefreshSet).toHaveBeenLastCalledWith(144);
    await userEvent.click(within(group).getByRole('radio', { name: '60 Hz (기본)' }));
    expect(actions.displayRefreshSet).toHaveBeenLastCalledWith(null);
  });

  it('sends a custom refresh rate from 30 to 240', async () => {
    const actions = show(variant('display-refresh'));
    const refresh = screen.getByRole('region', { name: '주사율' });
    expect(within(refresh).queryByRole('spinbutton')).toBeNull();
    await userEvent.click(within(refresh).getByRole('radio', { name: '사용자 지정' }));
    expect(actions.displayRefreshSet).not.toHaveBeenCalled();
    expect(within(refresh).getByText('30~240')).toBeInTheDocument();
    await userEvent.type(within(refresh).getByRole('spinbutton', { name: '사용자 지정 주사율' }), '100');
    await userEvent.click(within(refresh).getByRole('button', { name: '적용' }));
    expect(actions.displayRefreshSet).toHaveBeenCalledWith(100);
  });

  it('selects a custom rate Rust already lists', () => {
    show(variant('display-custom'));
    expect(screen.getByRole('radio', { name: '100 Hz' })).toHaveAttribute('aria-checked', 'true');
  });

  it('sets vertical sync when the virtual machine supports it', async () => {
    const actions = show(variant('display-refresh'));
    const vsync = screen.getByRole('region', { name: '수직 동기화' });
    expect(within(vsync).getByText('다시 시작해야 적용됩니다.')).toBeInTheDocument();
    const group = within(vsync).getByRole('radiogroup', { name: '수직 동기화' });
    expect(options(group)).toEqual(['끔', '켬', '적응형']);
    expect(within(group).getByRole('radio', { name: '켬' })).toHaveAttribute('aria-checked', 'true');
    await userEvent.click(within(group).getByRole('radio', { name: '적응형' }));
    expect(actions.displayVsyncSet).toHaveBeenCalledWith('adaptive');
  });

  it('sets scaling', async () => {
    const actions = show(variant('display-basic'));
    const fit = screen.getByRole('region', { name: '배율' });
    expect(within(fit).getByText('창에 맞춤은 창 크기에 따라 화면을 늘리거나 줄입니다. 1:1은 픽셀을 그대로 보입니다.')).toBeInTheDocument();
    const group = within(fit).getByRole('radiogroup', { name: '배율' });
    expect(options(group)).toEqual(['창에 맞춤', '1:1']);
    await userEvent.click(within(group).getByRole('radio', { name: '1:1' }));
    expect(actions.stageFitSet).toHaveBeenCalledWith('oneToOne');
  });

  it('keeps the section order 해상도, 주사율, 수직 동기화, 배율', () => {
    show(variant('display-refresh'));
    expect(screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)).toEqual([
      '해상도', '주사율', '수직 동기화', '배율',
    ]);
  });

  it('shows the issue above the sections', () => {
    show({ ...variant('display-basic'), issue: { code: 'x', message: '화면 크기를 바꾸지 못했습니다.', nextAction: '폭과 높이를 8의 배수로 입력하십시오.' } });
    expect(screen.getByRole('alert')).toHaveTextContent('화면 크기를 바꾸지 못했습니다.');
    expect(screen.getByRole('alert')).toHaveTextContent('폭과 높이를 8의 배수로 입력하십시오.');
  });
});
