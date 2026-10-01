// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AppSnapshot, SettingsInput } from '../contracts';
import { settingsGallery } from '../fixtures';
import { mockActions } from '../test-actions';
import { SettingsScreen } from './SettingsScreen';

function variant(id: string): AppSnapshot {
  const found = settingsGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found.snapshot;
}

function show(snapshot: AppSnapshot) {
  const actions = mockActions();
  render(<SettingsScreen snapshot={snapshot} actions={actions} />);
  return actions;
}

function section(name: string): HTMLElement {
  return screen.getByRole('region', { name });
}

/** The fixture's settings as `SettingsInput`, so a test states only the field it changes. */
const saved: SettingsInput = {
  memoryMib: 8192, vcpus: 4, gpuMode: 'virgl', closeAction: 'stopGuest', showFps: false,
  autoUpdateCheck: false, adbAccess: 'localhost', bindingOverlayDefault: true,
};

describe('settings screen (S6): layout', () => {
  it('keeps the section order and hides Google 계정 while the system is off', () => {
    show(variant('settings-stopped'));
    expect(screen.getByRole('heading', { level: 1, name: '설정' })).toBeInTheDocument();
    expect(screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)).toEqual([
      '운영체제', '설치된 운영체제', '입력', '고급', '저장 위치', '업데이트', '진단', '정보',
    ]);
  });

  it('adds Google 계정 after 설치된 운영체제 while running', () => {
    show(variant('settings-running'));
    expect(screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)).toEqual([
      '운영체제', '설치된 운영체제', 'Google 계정', '입력', '고급', '저장 위치', '업데이트', '진단', '정보',
    ]);
  });

  it('shows the issue above the sections', () => {
    show({ ...variant('settings-stopped'), issue: { code: 'x', message: '설정을 저장하지 못했습니다.', nextAction: '다시 시도하십시오.' } });
    expect(screen.getByRole('alert')).toHaveTextContent('설정을 저장하지 못했습니다.');
    expect(screen.getByRole('alert')).toHaveTextContent('다시 시도하십시오.');
  });

  it('has no way to set up again from the beginning', () => {
    show(variant('settings-running'));
    expect(screen.queryByRole('button', { name: '처음부터 다시 설정' })).toBeNull();
    expect(screen.queryByRole('link', { name: '처음부터 다시 설정' })).toBeNull();
  });
});

describe('settings screen (S6): 운영체제', () => {
  it('draws memory and cores between the host limits with the restart sentence', () => {
    show(variant('settings-stopped'));
    const system = section('운영체제');
    expect(within(system).getAllByText('다시 시작해야 적용됩니다.')).toHaveLength(2);
    const memory = within(system).getByRole('slider', { name: '메모리' });
    expect(memory).toHaveAttribute('min', '4096');
    expect(memory).toHaveAttribute('max', '61440');
    expect(memory).toHaveAttribute('aria-valuetext', '8 GiB');
    const cores = within(system).getByRole('slider', { name: '프로세서 코어' });
    expect(cores).toHaveAttribute('min', '2');
    expect(cores).toHaveAttribute('max', '16');
    expect(cores).toHaveAttribute('aria-valuetext', '4개');
    expect(within(system).getByText(
      '하드웨어 가속을 기본적으로 사용합니다. 화면이 검게 나오거나 깨질 때만 소프트웨어 렌더링을 사용하십시오. 소프트웨어 렌더링은 성능을 저하시킵니다.',
    )).toBeInTheDocument();
    expect(within(system).getByText('운영체제가 실행 중일 때 적용됩니다.')).toBeInTheDocument();
  });

  it('saves memory and cores when the slider is released', () => {
    const actions = show(variant('settings-stopped'));
    const memory = screen.getByRole('slider', { name: '메모리' });
    fireEvent.change(memory, { target: { value: '12288' } });
    expect(screen.getByText('12 GiB')).toBeInTheDocument();
    fireEvent.keyUp(memory);
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, memoryMib: 12288 });
    const cores = screen.getByRole('slider', { name: '프로세서 코어' });
    fireEvent.change(cores, { target: { value: '6' } });
    fireEvent.pointerUp(cores);
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, vcpus: 6 });
  });

  it('switches software rendering and the close action', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(screen.getByRole('switch', { name: '소프트웨어 렌더링' }));
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, gpuMode: 'software' });
    const close = screen.getByRole('combobox', { name: '창을 닫을 때' });
    expect(within(close).getAllByRole('option').map((option) => option.textContent)).toEqual(['운영체제 끄기', '트레이로 내리기']);
    await userEvent.selectOptions(close, 'minimizeToTray');
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, closeAction: 'minimizeToTray' });
  });
});

describe('settings screen (S6): 설치된 운영체제', () => {
  it('lists each system with version, disk, last start and confirmed features', () => {
    show(variant('settings-stopped'));
    const systems = section('설치된 운영체제');
    expect(Array.from(systems.querySelectorAll('.ome-setting-label'), (label) => label.textContent)).toEqual([
      'android-13', 'android-15', '시작할 운영체제', '운영체제 이미지',
    ]);
    expect(within(systems).getByText('안드로이드 13 · 64 GB · 마지막 실행 2026-09-26 19:58 · 확인된 기능 8개')).toBeInTheDocument();
    expect(within(systems).getByText('안드로이드 15 · 32 GB')).toBeInTheDocument();
    expect(within(systems).getByText('새 운영체제 이미지는 앱 업데이트로 추가됩니다.')).toBeInTheDocument();
  });

  it('reinstalls after the one-line confirmation', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(screen.getAllByRole('button', { name: '다시 설치' })[0] as HTMLElement);
    const dialog = screen.getByRole('dialog', { name: 'android-13 운영체제의 디스크를 지우고 같은 이미지로 처음부터 설치합니다.' });
    await userEvent.click(within(dialog).getByRole('button', { name: '다시 설치' }));
    expect(actions.guestReinstall).toHaveBeenCalledWith('android-13');
  });

  it('deletes after the one-line confirmation', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(screen.getAllByRole('button', { name: '삭제' })[1] as HTMLElement);
    const dialog = screen.getByRole('dialog', { name: 'android-15 운영체제를 디스크 파일까지 삭제합니다.' });
    await userEvent.click(within(dialog).getByRole('button', { name: '취소' }));
    expect(actions.guestDelete).not.toHaveBeenCalled();
    await userEvent.click(screen.getAllByRole('button', { name: '삭제' })[1] as HTMLElement);
    await userEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: '삭제' }));
    expect(actions.guestDelete).toHaveBeenCalledWith('android-15');
  });

  it('keeps the running system disk out of reach', () => {
    show(variant('settings-running'));
    expect(screen.getAllByRole('button', { name: '다시 설치' })).toHaveLength(1);
    expect(screen.getAllByRole('button', { name: '삭제' })).toHaveLength(1);
  });

  it('chooses the system to start', async () => {
    const actions = show(variant('settings-stopped'));
    const select = screen.getByRole('combobox', { name: '시작할 운영체제' });
    expect(select).toHaveValue('android-13');
    await userEvent.selectOptions(select, 'android-15');
    expect(actions.guestSelect).toHaveBeenCalledWith('android-15');
  });

  it('installs a new system from an image card and a disk size', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(screen.getByRole('button', { name: '새 운영체제 설치' }));
    const dialog = screen.getByRole('dialog', { name: '새 운영체제를 설치합니다.' });
    expect(within(dialog).getByRole('radio', { name: /안드로이드 13/ })).toHaveAttribute('aria-checked', 'true');
    expect(within(dialog).getByRole('radio', { name: '32 GB' })).toHaveAttribute('aria-checked', 'true');
    await userEvent.click(within(dialog).getByRole('radio', { name: /안드로이드 15/ }));
    await userEvent.click(within(dialog).getByRole('radio', { name: '128 GB' }));
    await userEvent.click(within(dialog).getByRole('button', { name: '설치' }));
    expect(actions.guestCreate).toHaveBeenCalledWith('sample-android-15', 128);
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('settings screen (S6): Google 계정', () => {
  const lead = '이 운영체제는 Google 인증 기기가 아닙니다. Google 계정으로 로그인하려면 기기 ID를 Google에 한 번 등록해야 합니다.';
  const r8 = '등록해도 Play 스토어와 인앱 결제는 보장되지 않습니다.';
  const fallback = '운영체제의 설정 → 계정에서 추가하십시오.';

  /** Accessible names of the section's buttons, in order. */
  function buttons(google: HTMLElement): (string | null)[] {
    return within(google).queryAllByRole('button').map((button) => button.getAttribute('aria-label') ?? button.textContent);
  }

  function lamps(google: HTMLElement): string[] {
    return Array.from(google.querySelectorAll('.ome-status-dot'), (dot) => dot.className);
  }

  /** The lead first and the R8 sentence last, whatever the state. */
  function expectFrame(google: HTMLElement) {
    const rows = google.querySelector('.ome-settings-rows');
    expect(rows?.firstElementChild).toHaveTextContent(lead);
    expect(rows?.lastElementChild?.textContent).toBe(r8);
  }

  it('is absent while the system is not running', () => {
    show(variant('settings-stopped'));
    expect(screen.queryByRole('region', { name: 'Google 계정' })).toBeNull();
    cleanup();
    const running = variant('settings-google-id');
    show({ ...running, guest: { ...running.guest, state: 'starting' } });
    expect(screen.queryByRole('region', { name: 'Google 계정' })).toBeNull();
  });

  it('reads the device ID first: grey lamp and no buttons', () => {
    show(variant('settings-google-reading'));
    const google = section('Google 계정');
    expectFrame(google);
    expect(within(google).getByText(
      '기기 ID를 읽는 중입니다. 운영체제가 Google 서버와 첫 교신을 마치면 나타납니다.',
    )).toBeInTheDocument();
    expect(lamps(google)).toEqual(['ome-status-dot ome-status-dot-muted']);
    expect(buttons(google)).toEqual([]);
  });

  it('shows the device ID with copy, the registration page and help before registration', async () => {
    const actions = show(variant('settings-google-id'));
    const google = section('Google 계정');
    expectFrame(google);
    expect(within(google).getByText('3f8a1c2e9b7d4051')).toHaveClass('ome-mono');
    expect(lamps(google)).toEqual([]);
    expect(buttons(google)).toEqual(['복사', '등록 페이지 열기', '도움말']);
    await userEvent.click(within(google).getByRole('button', { name: '복사' }));
    expect(actions.copyToClipboard).toHaveBeenCalledWith('deviceId');
    await userEvent.click(within(google).getByRole('button', { name: '등록 페이지 열기' }));
    expect(actions.openRegistrationPage).toHaveBeenCalledOnce();
    await userEvent.click(within(google).getByRole('button', { name: '도움말' }));
    expect(actions.openHelp).toHaveBeenCalledWith('googleAccount');
  });

  it('waits after the registration page was opened: amber lamp, the time, add account and reopen', async () => {
    const actions = show(variant('settings-google-registered'));
    const google = section('Google 계정');
    expectFrame(google);
    expect(within(google).getByText('등록한 뒤 약 10분이 지나면 계정을 추가할 수 있습니다.')).toBeInTheDocument();
    expect(within(google).getByText('20:14에 열었습니다')).toBeInTheDocument();
    expect(lamps(google)).toEqual(['ome-status-dot ome-status-dot-warning']);
    expect(buttons(google)).toEqual(['계정 추가 화면 열기', '등록 페이지 다시 열기']);
    await userEvent.click(within(google).getByRole('button', { name: '계정 추가 화면 열기' }));
    expect(actions.googleAccountAddOpen).toHaveBeenCalledOnce();
    await userEvent.click(within(google).getByRole('button', { name: '등록 페이지 다시 열기' }));
    expect(actions.openRegistrationPage).toHaveBeenCalledOnce();
  });

  it('counts the signed-in accounts: green lamp and only add account', async () => {
    const actions = show(variant('settings-google-account'));
    const google = section('Google 계정');
    expectFrame(google);
    expect(within(google).getByText('Google 계정 1개가 로그인되어 있습니다.')).toBeInTheDocument();
    expect(within(google).queryByText('20:14에 열었습니다')).toBeNull();
    expect(lamps(google)).toEqual(['ome-status-dot ome-status-dot-success']);
    expect(buttons(google)).toEqual(['계정 추가 화면 열기']);
    await userEvent.click(within(google).getByRole('button', { name: '계정 추가 화면 열기' }));
    expect(actions.googleAccountAddOpen).toHaveBeenCalledOnce();
  });

  it('puts the one-line fallback in place of add account where the system cannot open that screen', () => {
    for (const id of ['settings-google-registered', 'settings-google-account']) {
      const snapshot = variant(id);
      show({ ...snapshot, guest: { ...snapshot.guest, addAccountSupported: false } });
      const google = section('Google 계정');
      expectFrame(google);
      expect(within(google).getByText(fallback)).toBeInTheDocument();
      expect(within(google).queryByRole('button', { name: '계정 추가 화면 열기' })).toBeNull();
      cleanup();
    }
  });

  it('has no fallback line while the add-account screen can open', () => {
    show(variant('settings-google-account'));
    expect(within(section('Google 계정')).queryByText(fallback)).toBeNull();
  });
});

describe('settings screen (S6): 입력', () => {
  it('changes the pause hotkey with the next key press', async () => {
    const actions = show(variant('settings-stopped'));
    const hotkey = within(section('입력')).getByRole('button', { name: 'F12' });
    await userEvent.click(hotkey);
    expect(hotkey).toHaveTextContent('키를 누르십시오');
    fireEvent.keyDown(document, { key: 'F9', code: 'F9' });
    expect(actions.inputSuspendHotkeySet).toHaveBeenCalledWith('F9');
    expect(hotkey).toHaveTextContent('F12');
  });

  it('keeps the hotkey on Esc', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(screen.getByRole('button', { name: 'F12' }));
    fireEvent.keyDown(document, { key: 'Escape', code: 'Escape' });
    expect(actions.inputSuspendHotkeySet).not.toHaveBeenCalled();
  });

  it('switches the marker default and per-game application', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(screen.getByRole('switch', { name: '매핑 표지 기본 표시' }));
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, bindingOverlayDefault: false });
    await userEvent.click(screen.getByRole('switch', { name: '게임별 자동 적용' }));
    expect(actions.inputAutoApplySet).toHaveBeenCalledWith(false);
  });
});

describe('settings screen (S6): 고급', () => {
  it('shows the adb address with its warning and allows other PCs only on request', async () => {
    const actions = show(variant('settings-stopped'));
    const advanced = section('고급');
    expect(within(advanced).getByText('127.0.0.1:5555')).toBeInTheDocument();
    expect(within(advanced).getByText(
      'adb로 연결한 프로그램은 운영체제 안의 모든 앱과 데이터를 다룰 수 있습니다. 신뢰하는 프로그램만 연결하십시오.',
    )).toBeInTheDocument();
    const network = within(advanced).getByRole('switch', { name: '다른 PC에서 연결 허용' });
    expect(network).toHaveAttribute('aria-checked', 'false');
    expect(within(advanced).queryByText('같은 네트워크의 누구나 이 운영체제에 접근할 수 있습니다.')).toBeNull();
    await userEvent.click(network);
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, adbAccess: 'network' });
  });

  it('copies the adb address and opens the adb help page', async () => {
    const actions = show(variant('settings-stopped'));
    const advanced = section('고급');
    await userEvent.click(within(advanced).getByRole('button', { name: '복사' }));
    expect(actions.copyToClipboard).toHaveBeenCalledWith('adbAddress');
    await userEvent.click(within(advanced).getByRole('button', { name: '도움말' }));
    expect(actions.openHelp).toHaveBeenCalledWith('adbSecurity');
  });

  it('has no copy button without an adb address', () => {
    const snapshot = variant('settings-stopped');
    show({ ...snapshot, guest: { ...snapshot.guest, adbAddress: null } });
    expect(within(section('고급')).queryByRole('button', { name: '복사' })).toBeNull();
    expect(within(section('고급')).getByRole('button', { name: '도움말' })).toBeInTheDocument();
  });

  it('warns while other PCs may connect', () => {
    show(variant('settings-network'));
    expect(screen.getByText('같은 네트워크의 누구나 이 운영체제에 접근할 수 있습니다.')).toBeInTheDocument();
  });

  it('changes root access only while running', async () => {
    const help = '앱이 루트 권한을 요청할 수 있게 합니다. 일부 게임과 결제 기능은 루트가 켜진 기기에서 동작하지 않습니다.';
    const { unmount } = render(<SettingsScreen snapshot={variant('settings-stopped')} actions={mockActions()} />);
    expect(screen.getByText(help)).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: '루트 권한' })).toBeDisabled();
    unmount();
    const actions = show(variant('settings-running'));
    await userEvent.click(screen.getByRole('switch', { name: '루트 권한' }));
    expect(actions.guestRootSet).toHaveBeenCalledWith(true);
  });

  it('has no root row before the system reported it', () => {
    const snapshot = variant('settings-stopped');
    show({ ...snapshot, guest: { ...snapshot.guest, rootEnabled: null } });
    expect(screen.queryByRole('switch', { name: '루트 권한' })).toBeNull();
  });

  it('switches the fps readout', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(screen.getByRole('switch', { name: 'fps 표시' }));
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, showFps: true });
  });
});

describe('settings screen (S6): 저장 위치, 업데이트, 진단, 정보', () => {
  it('shows the home folder and its disk usage', () => {
    show(variant('settings-stopped'));
    const storage = section('저장 위치');
    expect(within(storage).getByText('C:\\Users\\사용자\\AppData\\Local\\OpenMobileEmulator')).toBeInTheDocument();
    expect(within(storage).getByText('38.2 GB')).toBeInTheDocument();
  });

  it('opens the home folder', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(within(section('저장 위치')).getByRole('button', { name: '폴더 열기' }));
    expect(actions.openHomeFolder).toHaveBeenCalledOnce();
  });

  it('opens the program folder from the about section', async () => {
    const actions = show(variant('settings-stopped'));
    await userEvent.click(within(section('정보')).getByRole('button', { name: '프로그램 폴더 열기' }));
    expect(actions.openInstallFolder).toHaveBeenCalledOnce();
  });

  it('checks for updates and switches the automatic check', async () => {
    const actions = show(variant('settings-stopped'));
    const update = section('업데이트');
    expect(within(update).getByText('마지막 확인 2026-09-26 09:10. 새 버전이 없습니다.')).toBeInTheDocument();
    expect(within(update).getByText('0.1.0')).toBeInTheDocument();
    await userEvent.click(within(update).getByRole('button', { name: '업데이트 확인' }));
    expect(actions.updateCheck).toHaveBeenCalledOnce();
    expect(within(update).getByText('앱을 열 때 새 버전이 있는지 확인합니다.')).toBeInTheDocument();
    await userEvent.click(within(update).getByRole('switch', { name: '자동 확인' }));
    expect(actions.settingsSave).toHaveBeenLastCalledWith({ ...saved, autoUpdateCheck: true });
  });

  it('holds the check button while checking', () => {
    const snapshot = variant('settings-stopped');
    show({ ...snapshot, update: { currentVersion: '0.1.0', state: { kind: 'checking' } } });
    expect(screen.getByRole('button', { name: '업데이트 확인' })).toBeDisabled();
    expect(screen.getByText('새 버전을 확인하는 중입니다.')).toBeInTheDocument();
  });

  it('offers a new version for download and install', async () => {
    const actions = show(variant('settings-network'));
    const update = section('업데이트');
    expect(within(update).getByText('새 버전')).toBeInTheDocument();
    expect(within(update).getByText('0.2.0')).toBeInTheDocument();
    await userEvent.click(within(update).getByRole('button', { name: '다운로드 후 설치' }));
    expect(actions.updateInstall).toHaveBeenCalledOnce();
  });

  it('opens the release notes of the new version, and has no link without them', async () => {
    const actions = show(variant('settings-network'));
    await userEvent.click(within(section('업데이트')).getByRole('button', { name: '릴리스 노트' }));
    expect(actions.openHelp).toHaveBeenCalledWith('releaseNotes');
    cleanup();
    const snapshot = variant('settings-network');
    show({ ...snapshot, update: { currentVersion: '0.1.0', state: { kind: 'available', version: '0.2.0', notesUrl: null, asset: { name: 'ome-0.2.0-setup.exe', sizeBytes: 1 } } } });
    expect(within(section('업데이트')).queryByRole('button', { name: '릴리스 노트' })).toBeNull();
    expect(within(section('업데이트')).getByRole('button', { name: '다운로드 후 설치' })).toBeInTheDocument();
  });

  it('shows download progress and the ready state', async () => {
    show(variant('settings-update-downloading'));
    expect(screen.getByText('다운로드 중')).toBeInTheDocument();
    expect(screen.getByText('37%')).toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: '업데이트 다운로드 진행률' })).toBeInTheDocument();
    const snapshot = variant('settings-stopped');
    const actions = mockActions();
    render(<SettingsScreen snapshot={{ ...snapshot, update: { currentVersion: '0.1.0', state: { kind: 'readyToInstall', version: '0.2.0' } } }} actions={actions} />);
    await userEvent.click(screen.getByRole('button', { name: '설치' }));
    expect(actions.updateInstall).toHaveBeenCalledOnce();
  });

  it('states a failed update in the issue words', () => {
    show(variant('settings-update-failed'));
    expect(screen.getByText('새 버전을 확인하지 못했습니다.')).toBeInTheDocument();
    expect(screen.getByText('인터넷 연결을 확인한 뒤 다시 시도하십시오.')).toBeInTheDocument();
  });

  it('exports diagnostics, opens the log folder and lists the last five events', async () => {
    const actions = show(variant('settings-stopped'));
    const diagnostics = section('진단');
    expect(within(diagnostics).getByText('로그와 환경 정보를 파일 하나로 묶습니다.')).toBeInTheDocument();
    await userEvent.click(within(diagnostics).getByRole('button', { name: '진단 묶음 내보내기' }));
    await userEvent.click(within(diagnostics).getByRole('button', { name: '로그 폴더 열기' }));
    expect(actions.diagnosticsExport).toHaveBeenCalledOnce();
    expect(actions.openLogsFolder).toHaveBeenCalledOnce();
    expect(within(diagnostics).getByRole('heading', { level: 3, name: '최근 이벤트' })).toBeInTheDocument();
    const events = within(diagnostics).getAllByRole('listitem');
    expect(events).toHaveLength(5);
    expect(events[0]).toHaveTextContent('2026-09-26 19:58');
    expect(events[0]).toHaveTextContent('운영체제를 시작했습니다.');
    expect(within(diagnostics).queryByText('호스트 점검 결과를 저장했습니다.')).toBeNull();
  });

  it('names the product, version and license', () => {
    show(variant('settings-stopped'));
    const about = section('정보');
    expect(within(about).getByText('Open Mobile Emulator')).toBeInTheDocument();
    expect(within(about).getByText('버전 0.1.0 · GPL-2.0-or-later')).toBeInTheDocument();
  });

  it('opens the third-party notices and the virtual machine source', async () => {
    const actions = show(variant('settings-stopped'));
    const about = section('정보');
    expect(within(about).getByText('가상 머신 소스 코드')).toBeInTheDocument();
    expect(within(about).getByText('이 앱에 포함된 가상 머신 프로그램의 소스 코드를 받을 수 있습니다.')).toBeInTheDocument();
    await userEvent.click(within(about).getByRole('button', { name: '제3자 고지' }));
    expect(actions.openHelp).toHaveBeenLastCalledWith('thirdPartyNotices');
    await userEvent.click(within(about).getByRole('button', { name: '소스 코드 받기' }));
    expect(actions.openHelp).toHaveBeenLastCalledWith('qemuSource');
    expect(actions.openHelp).toHaveBeenCalledTimes(2);
  });
});
