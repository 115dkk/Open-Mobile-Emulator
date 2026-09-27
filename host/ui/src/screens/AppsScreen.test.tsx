// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AppItem, AppSnapshot } from '../contracts';
import { appsGallery } from '../fixtures';
import { mockActions } from '../test-actions';
import { AppsScreen } from './AppsScreen';

function variant(id: string): AppSnapshot {
  const found = appsGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found.snapshot;
}

function show(snapshot: AppSnapshot) {
  const actions = mockActions();
  render(<AppsScreen snapshot={snapshot} actions={actions} />);
  return actions;
}

function row(name: string): HTMLElement {
  const cell = screen.getByRole('cell', { name });
  const found = cell.closest('tr');
  if (found === null) throw new Error(`Missing row ${name}`);
  return found;
}

function withApp(label: string): AppSnapshot {
  const base = variant('apps-running');
  const app: AppItem = { package: 'com.example.sample.x', label, versionName: '1.0.0', installedAt: null };
  return { ...base, apps: { ...base.apps, items: [app] } };
}

describe('apps: running operating system', () => {
  it('lists name, package, version and install day, then the count', () => {
    show(variant('apps-running'));
    expect(screen.getByRole('heading', { level: 1, name: '앱' })).toBeInTheDocument();
    const table = screen.getByRole('table', { name: '설치된 앱' });
    expect(within(table).getAllByRole('columnheader').map((cell) => cell.textContent))
      .toEqual(['이름', '패키지', '버전', '설치일', '동작']);
    const first = within(row('샘플 앱 A'));
    expect(first.getByText('com.example.sample.a')).toBeInTheDocument();
    expect(first.getByText('1.4.2')).toBeInTheDocument();
    expect(first.getByText('2026-09-24')).toBeInTheDocument();
    expect(within(table).getAllByRole('row')).toHaveLength(4);
    expect(screen.getByText('앱 3개.')).toBeInTheDocument();
  });

  it('installs, opens the screenshot folder and launches a row', async () => {
    const actions = show(variant('apps-running'));
    await userEvent.click(screen.getByRole('button', { name: '설치' }));
    await userEvent.click(screen.getByRole('button', { name: '스크린샷 폴더 열기' }));
    await userEvent.click(within(row('샘플 앱 B')).getByRole('button', { name: '실행' }));
    expect(actions.appInstallPick).toHaveBeenCalledOnce();
    expect(actions.openScreenshotsFolder).toHaveBeenCalledOnce();
    expect(actions.appLaunch).toHaveBeenCalledWith('com.example.sample.b');
  });

  it('asks before removing and removes on 제거', async () => {
    const actions = show(variant('apps-running'));
    await userEvent.click(within(row('샘플 앱 A')).getByRole('button', { name: '제거' }));
    const dialog = screen.getByRole('dialog', { name: '샘플 앱 A를 제거합니다.' });
    expect(actions.appUninstall).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole('button', { name: '제거' }));
    expect(actions.appUninstall).toHaveBeenCalledOnce();
    expect(actions.appUninstall).toHaveBeenCalledWith('com.example.sample.a');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('cancels the removal', async () => {
    const actions = show(variant('apps-running'));
    await userEvent.click(within(row('샘플 앱 C')).getByRole('button', { name: '제거' }));
    await userEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: '취소' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(actions.appUninstall).not.toHaveBeenCalled();
  });

  it('filters by name or package without changing the count', async () => {
    show(variant('apps-running'));
    const search = screen.getByRole('searchbox', { name: '앱 검색' });
    expect(search).toHaveAttribute('placeholder', '이름이나 패키지로 찾기');
    await userEvent.type(search, 'sample.b');
    expect(screen.getByRole('cell', { name: '샘플 앱 B' })).toBeInTheDocument();
    expect(screen.queryByRole('cell', { name: '샘플 앱 A' })).toBeNull();
    expect(screen.getByText('앱 3개.')).toBeInTheDocument();
    await userEvent.clear(search);
    await userEvent.type(search, '없는 이름');
    expect(screen.queryByRole('table')).toBeNull();
    expect(screen.getByText('검색 결과가 없습니다.')).toBeInTheDocument();
  });

  it('shows the install line above the list', () => {
    show(variant('apps-installing'));
    expect(screen.getByText('sample-app-d.xapk')).toBeInTheDocument();
    expect(screen.getByText('sample-app-d.xapk').parentElement).toHaveTextContent('sample-app-d.xapk 설치 중');
    expect(screen.getByRole('progressbar', { name: '설치 진행률' })).toHaveAttribute('aria-valuenow', '41');
    expect(screen.getByText('41%')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '취소' })).toBeNull();
  });

  it('says there are no apps', () => {
    show(variant('apps-empty'));
    expect(screen.getByText('설치된 앱이 없습니다.')).toBeInTheDocument();
    expect(screen.getByText('앱 0개.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '설치' })).toBeInTheDocument();
  });

  it('shows the issue at the top', () => {
    const base = variant('apps-running');
    show({ ...base, issue: { code: 'x', message: '앱을 제거하지 못했습니다.', nextAction: '운영체제를 다시 시작하십시오.' } });
    expect(screen.getByRole('alert')).toHaveTextContent('앱을 제거하지 못했습니다.');
    expect(screen.getByRole('alert')).toHaveTextContent('운영체제를 다시 시작하십시오.');
  });
});

describe('apps: operating system off', () => {
  it('keeps the last list dimmed without action buttons', () => {
    show(variant('apps-stopped'));
    const table = screen.getByRole('table', { name: '설치된 앱' });
    expect(table.closest('.ome-apps-list')).toHaveClass('ome-apps-list-stale');
    expect(within(table).getAllByRole('columnheader').map((cell) => cell.textContent))
      .toEqual(['이름', '패키지', '버전', '설치일']);
    expect(screen.getByRole('cell', { name: '샘플 앱 C' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '실행' })).toBeNull();
    expect(screen.queryByRole('button', { name: '제거' })).toBeNull();
    expect(screen.queryByRole('button', { name: '설치' })).toBeNull();
    expect(screen.getByRole('button', { name: '스크린샷 폴더 열기' })).toBeInTheDocument();
    expect(screen.getByText('앱 3개.')).toBeInTheDocument();
  });
});

describe('apps: removal title particle', () => {
  it.each([
    ['메모장', '메모장을 제거합니다.'],
    ['메모', '메모를 제거합니다.'],
    ['샘플 앱 L', '샘플 앱 L을 제거합니다.'],
    ['샘플 앱 2', '샘플 앱 2를 제거합니다.'],
    ['샘플 앱 10', '샘플 앱 10을 제거합니다.'],
    ['Sample Notes', 'Sample Notes 앱을 제거합니다.'],
  ])('%s', async (label, title) => {
    show(withApp(label));
    await userEvent.click(screen.getByRole('button', { name: '제거' }));
    expect(screen.getByRole('dialog', { name: title })).toBeInTheDocument();
  });
});
