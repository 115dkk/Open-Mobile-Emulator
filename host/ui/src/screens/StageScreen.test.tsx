// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AppSnapshot } from '../contracts';
import { stageGallery } from '../fixtures';
import { mockActions } from '../test-actions';
import { StageScreen } from './StageScreen';

function variant(id: string): AppSnapshot {
  const found = stageGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found.snapshot;
}

function show(snapshot: AppSnapshot) {
  const actions = mockActions();
  render(<StageScreen snapshot={snapshot} actions={actions} />);
  return actions;
}

function toolbar() {
  return screen.getByRole('group', { name: '화면 도구' });
}

function statusBar() {
  const bar = document.querySelector('footer.ome-stage-status');
  if (!(bar instanceof HTMLElement)) throw new Error('Missing status bar');
  return bar;
}

/** Toolbar controls in document order, by accessible name. */
function toolbarOrder(): (string | null)[] {
  return [...toolbar().querySelectorAll('button, select')].map((control) => control.getAttribute('aria-label'));
}

/** Outside the running state the toolbar holds the display preset and the marker toggle (M2-SCREENS.md 9.1). */
function expectPresetAndMarkerToggleOnly() {
  expect(toolbarOrder()).toEqual(['표시 프리셋', '매핑 표시']);
}

describe('stage: stopped', () => {
  it('offers one start button and the last normal exit', async () => {
    const actions = show(variant('stage-stopped'));
    expect(screen.getByRole('heading', { level: 1, name: '화면' })).toBeInTheDocument();
    expect(screen.getByText('마지막 실행 2026-09-26 19:58, 정상 종료')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '로그 보기' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: '시작' }));
    expect(actions.guestStart).toHaveBeenCalledOnce();
  });

  it('keeps the display preset and the marker toggle in the toolbar', async () => {
    const actions = show(variant('stage-stopped'));
    expectPresetAndMarkerToggleOnly();
    expect(within(toolbar()).getByRole('combobox', { name: '표시 프리셋' })).toHaveValue('fhd-landscape');
    await userEvent.click(within(toolbar()).getByRole('button', { name: '매핑 표시' }));
    expect(actions.inputOverlayToggle).toHaveBeenCalledOnce();
  });

  it('names the state, version, size and input profile in the status bar', () => {
    show(variant('stage-stopped'));
    const bar = within(statusBar());
    expect(bar.getByText('시작 가능')).toBeInTheDocument();
    expect(bar.getByText('안드로이드 13')).toBeInTheDocument();
    expect(bar.getByText('1920x1080 · 창에 맞춤')).toBeInTheDocument();
    expect(bar.getByText('입력 프로필: 내 프로필 1')).toBeInTheDocument();
    expect(bar.queryByText('앱 관리 연결됨')).toBeNull();
  });

  it('says an abnormal exit and opens the logs', async () => {
    const actions = show(variant('stage-stopped-abnormal'));
    expect(screen.getByText('마지막 실행 2026-09-26 19:58, 비정상 종료')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '로그 보기' }));
    expect(actions.openLogsFolder).toHaveBeenCalledOnce();
  });

  it('applies a display preset from the toolbar', async () => {
    const actions = show(variant('stage-stopped'));
    const select = within(toolbar()).getByRole('combobox', { name: '표시 프리셋' });
    expect(within(select).getAllByRole('option').map((option) => option.textContent)).toEqual([
      '1280x720', '1920x1080', '세로 720x1280',
    ]);
    await userEvent.selectOptions(select, 'hd-portrait');
    expect(actions.displayPresetApply).toHaveBeenCalledWith('hd-portrait');
  });

  it('shows a custom size in the preset select without offering it as a choice', async () => {
    const stopped = variant('stage-stopped');
    const actions = show({
      ...stopped,
      guest: { ...stopped.guest, resolution: null },
      display: { ...stopped.display, activeId: null, custom: { size: { width: 2560, height: 1440 }, densityDpi: 400 } },
    });
    const select = within(toolbar()).getByRole('combobox', { name: '표시 프리셋' });
    expect(select).toHaveValue('');
    expect(select).toHaveDisplayValue('2560x1440');
    expect(within(statusBar()).getByText('2560x1440 · 창에 맞춤')).toBeInTheDocument();
    await userEvent.selectOptions(select, 'hd-landscape');
    expect(actions.displayPresetApply).toHaveBeenCalledWith('hd-landscape');
  });
});

describe('stage: starting', () => {
  it('shows the three boot steps without sentences and cancels by stopping', async () => {
    const actions = show(variant('stage-starting'));
    const steps = screen.getByRole('list', { name: '부팅 진행' });
    expect(within(steps).getAllByRole('listitem').map((item) => item.querySelector('.ome-step-label')?.textContent))
      .toEqual(['가상 머신 시작', '화면 연결', '운영체제 부팅']);
    expect(within(steps).getByText('운영체제 부팅').closest('li')).toHaveAttribute('aria-current', 'step');
    expect(within(statusBar()).getByText('부팅 중')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '시작' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: '취소' }));
    expect(actions.guestStop).toHaveBeenCalledOnce();
    expectPresetAndMarkerToggleOnly();
  });
});

describe('stage: running', () => {
  it('orders the toolbar and sends its commands', async () => {
    const actions = show(variant('stage-running'));
    expect(toolbarOrder()).toEqual(['스크린샷', '볼륨', '표시 프리셋', '매핑 표시', '매핑 편집', '다시 시작', '끄기']);
    const bar = within(toolbar());
    await userEvent.click(bar.getByRole('button', { name: '스크린샷' }));
    await userEvent.click(bar.getByRole('button', { name: '매핑 표시' }));
    await userEvent.click(bar.getByRole('button', { name: '매핑 편집' }));
    await userEvent.click(bar.getByRole('button', { name: '다시 시작' }));
    await userEvent.click(bar.getByRole('button', { name: '끄기' }));
    expect(actions.screenshotSave).toHaveBeenCalledOnce();
    expect(actions.inputOverlayToggle).toHaveBeenCalledOnce();
    expect(actions.inputEditorToggle).toHaveBeenCalledOnce();
    expect(actions.guestRestart).toHaveBeenCalledOnce();
    expect(actions.guestStop).toHaveBeenCalledOnce();
    expect(bar.getByRole('button', { name: '매핑 편집' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('marks the marker toggle from overlayVisible', () => {
    const running = variant('stage-running');
    const { unmount } = render(<StageScreen snapshot={running} actions={mockActions()} />);
    expect(within(toolbar()).getByRole('button', { name: '매핑 표시' })).toHaveAttribute('aria-pressed', 'true');
    unmount();
    show({ ...running, input: { ...running.input, overlayVisible: false } });
    expect(within(toolbar()).getByRole('button', { name: '매핑 표시' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('opens the media volume slider in the toolbar and sends each step', async () => {
    const actions = show(variant('stage-running'));
    const bar = within(toolbar());
    const volume = bar.getByRole('button', { name: '볼륨' });
    expect(volume).toHaveAttribute('aria-expanded', 'false');
    expect(bar.queryByRole('slider', { name: '미디어 볼륨' })).toBeNull();
    await userEvent.click(volume);
    expect(volume).toHaveAttribute('aria-expanded', 'true');
    const slider = bar.getByRole('slider', { name: '미디어 볼륨' });
    expect(slider).toHaveValue('9');
    expect(slider).toHaveAttribute('min', '0');
    expect(slider).toHaveAttribute('max', '15');
    fireEvent.change(slider, { target: { value: '12' } });
    expect(actions.guestVolumeSet).toHaveBeenCalledWith(12);
    expect(slider).toHaveValue('12');
    fireEvent.pointerUp(slider);
    expect(actions.guestVolumeSet).toHaveBeenCalledOnce();
  });

  it('has no volume button unless the probe found the media volume and Rust reported its value', () => {
    const running = variant('stage-running');
    const items = running.guest.capabilities.items.map((item) => item.id === 'mediaVolume'
      ? { ...item, state: 'unknown' as const } : item);
    const { unmount } = render(
      <StageScreen
        snapshot={{ ...running, guest: { ...running.guest, capabilities: { ...running.guest.capabilities, items } } }}
        actions={mockActions()}
      />,
    );
    expect(within(toolbar()).queryByRole('button', { name: '볼륨' })).toBeNull();
    unmount();
    show({ ...running, guest: { ...running.guest, mediaVolume: null } });
    expect(within(toolbar()).queryByRole('button', { name: '볼륨' })).toBeNull();
    expect(within(toolbar()).getByRole('button', { name: '스크린샷' })).toBeInTheDocument();
  });

  it('draws nothing over the embedded window and names the connection in the status bar', () => {
    show(variant('stage-running'));
    expect(screen.queryByRole('button', { name: '시작' })).toBeNull();
    expect(screen.queryByRole('list', { name: '부팅 진행' })).toBeNull();
    const bar = within(statusBar());
    expect(bar.getByText('실행 중')).toBeInTheDocument();
    expect(bar.getByText('안드로이드 13')).toBeInTheDocument();
    expect(bar.getByText('1920x1080 · 창에 맞춤')).toBeInTheDocument();
    expect(bar.getByText('앱 관리 연결됨')).toBeInTheDocument();
    expect(bar.getByText('입력 프로필: 내 프로필 1')).toBeInTheDocument();
    expect(bar.queryByText(/fps/u)).toBeNull();
    expect(bar.queryByText(/매핑 일시 중지/u)).toBeNull();
  });

  it('adds the foreground app, the pause and fps when the snapshot says so', () => {
    show(variant('stage-running-auto'));
    const bar = within(statusBar());
    expect(bar.getByText('입력 프로필: 내 프로필 1 (샘플 앱 A)')).toBeInTheDocument();
    expect(bar.getByText('매핑 일시 중지 (F12)')).toBeInTheDocument();
    expect(bar.getByText('58 fps')).toBeInTheDocument();
  });

  it('marks the mapping editor as pressed while editing', () => {
    show(variant('stage-running-editing'));
    expect(within(toolbar()).getByRole('button', { name: '매핑 편집' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('has no screenshot button when the probe found screenshots unavailable', () => {
    const running = variant('stage-running');
    const items = running.guest.capabilities.items.map((item) => item.id === 'screenshot'
      ? { ...item, state: 'unavailable' as const } : item);
    show({ ...running, guest: { ...running.guest, capabilities: { ...running.guest.capabilities, items } } });
    expect(within(toolbar()).queryByRole('button', { name: '스크린샷' })).toBeNull();
    expect(within(toolbar()).getByRole('button', { name: '끄기' })).toBeInTheDocument();
  });

  it('points to the separate window and brings it to the front', async () => {
    const actions = show(variant('stage-running-separate'));
    expect(screen.getByText('운영체제 화면은 별도 창에 있습니다.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '창 앞으로' }));
    expect(actions.guestWindowToFront).toHaveBeenCalledOnce();
  });

  it('shows the issue above the stage', () => {
    show(variant('stage-issue'));
    expect(screen.getByRole('alert')).toHaveTextContent('스크린샷을 저장하지 못했습니다.');
    expect(screen.getByRole('alert')).toHaveTextContent('저장 위치의 여유 공간을 확인하십시오.');
  });
});

describe('stage: restarting and stopping', () => {
  it('keeps the stage dark with the state in the status bar', () => {
    show(variant('stage-restarting'));
    expect(within(statusBar()).getByText('다시 시작 중')).toBeInTheDocument();
    expect(screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual(['매핑 표시']);
  });

  it('names stopping in the status bar', () => {
    show(variant('stage-stopping'));
    expect(within(statusBar()).getByText('끄는 중')).toBeInTheDocument();
    expect(screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual(['매핑 표시']);
  });
});

describe('stage: failed', () => {
  const body = '다시 시작을 시도할 수 있습니다. 반복될 경우 로그를 첨부해 문제를 보고하십시오.';

  it.each([
    ['stage-failed-start', '가상 머신이 시작하지 못했습니다.'],
    ['stage-failed-boot', '운영체제 부팅에 실패했습니다.'],
    ['stage-failed-crash', '크래시가 일어났습니다.'],
  ])('%s titles the cause', (id, title) => {
    show(variant(id));
    expect(screen.getByRole('heading', { level: 2, name: title })).toBeInTheDocument();
    expect(screen.getByText(body)).toBeInTheDocument();
    expect(within(statusBar()).getByText('실패')).toBeInTheDocument();
  });

  it('starts again, exports diagnostics and keeps the log path under 세부 정보', async () => {
    const actions = show(variant('stage-failed-boot'));
    await userEvent.click(screen.getByRole('button', { name: '다시 시작' }));
    await userEvent.click(screen.getByRole('button', { name: '진단 묶음 내보내기' }));
    expect(actions.guestStart).toHaveBeenCalledOnce();
    expect(actions.guestRestart).not.toHaveBeenCalled();
    expect(actions.diagnosticsExport).toHaveBeenCalledOnce();
    const details = screen.getByText('세부 정보').closest('details');
    expect(details).not.toBeNull();
    expect(details).not.toHaveAttribute('open');
    expect(details).toHaveTextContent('로그 파일');
    expect(details).toHaveTextContent('C:\\Users\\사용자\\AppData\\Local\\OpenMobileEmulator\\logs\\guest-20260926-1958.log');
  });

  it('leaves out 세부 정보 when there is no log', () => {
    const failed = variant('stage-failed-crash');
    const lastExit = failed.guest.lastExit === null ? null : { ...failed.guest.lastExit, logPath: null };
    show({ ...failed, guest: { ...failed.guest, lastExit } });
    expect(screen.queryByText('세부 정보')).toBeNull();
    expect(screen.getByRole('heading', { level: 2, name: '크래시가 일어났습니다.' })).toBeInTheDocument();
  });

  it('has only the display preset and the marker toggle in the toolbar', () => {
    show(variant('stage-failed-start'));
    expectPresetAndMarkerToggleOnly();
  });
});
