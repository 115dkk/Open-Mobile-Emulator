// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { AppSnapshot } from '../contracts';
import { wizardGallery } from '../fixtures';
import { mockActions } from '../test-actions';
import { WizardScreen } from './WizardScreen';

function variant(id: string): AppSnapshot {
  const found = wizardGallery.find((item) => item.id === id);
  if (found === undefined) throw new Error(`Missing fixture ${id}`);
  return found.snapshot;
}

function show(snapshot: AppSnapshot) {
  const actions = mockActions();
  render(<WizardScreen snapshot={snapshot} actions={actions} />);
  return actions;
}

describe('first-run wizard: 나중에 하기', () => {
  it.each([
    'host-ready', 'whpx-consent', 'reboot-pending', 'download-idle', 'download-transferring',
    'install-disk', 'install-guide', 'first-boot', 'first-boot-failed', 'app-install',
  ])('%s closes the wizard and keeps the step', async (id) => {
    const actions = show(variant(id));
    await userEvent.click(screen.getByRole('button', { name: '나중에 하기' }));
    expect(actions.wizardDefer).toHaveBeenCalledOnce();
    expect(actions.wizardSkip).not.toHaveBeenCalled();
  });
});

describe('first-run wizard: S1.1 host check', () => {
  it('states the verdict, the six rows with Rust sentences and the check time', () => {
    show(variant('host-ready'));
    expect(screen.getByRole('heading', { level: 1, name: '호스트 점검' })).toBeInTheDocument();
    expect(screen.getByText('1 / 7')).toBeInTheDocument();
    expect(screen.getByText('이 PC에서는 Open Mobile Emulator를 사용할 수 있습니다.')).toBeInTheDocument();
    const rows = within(screen.getByRole('list')).getAllByRole('listitem');
    expect(rows.map((row) => row.querySelector('.ome-host-label')?.textContent)).toEqual([
      'CPU 가상화', 'Windows 하이퍼바이저 플랫폼', '다시 시작 대기', '가상 머신 구성 요소', '앱 설치 도구', '디스크 여유 공간',
    ]);
    expect(screen.getByText('Windows 하이퍼바이저 플랫폼이 꺼져 있습니다. 다음 단계에서 동의하면 켭니다.')).toBeInTheDocument();
    expect(screen.getByText('2026-09-26 19:52 확인')).toBeInTheDocument();
  });

  it('sends 다시 확인 and 계속', async () => {
    const actions = show(variant('host-ready'));
    await userEvent.click(screen.getByRole('button', { name: '다시 확인' }));
    await userEvent.click(screen.getByRole('button', { name: '계속' }));
    expect(actions.hostCheckRefresh).toHaveBeenCalledOnce();
    expect(actions.wizardContinue).toHaveBeenCalledOnce();
  });

  it('says the PC cannot be used and holds 계속 when a row blocks', () => {
    show(variant('host-blocked'));
    expect(screen.getByText('이 PC에서는 Open Mobile Emulator를 사용할 수 없습니다.')).toBeInTheDocument();
    expect(screen.getByText('저장 공간이 40GB보다 적습니다. 파일을 정리한 뒤 다시 확인하십시오.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '계속' })).toBeDisabled();
  });

  it('shows the issue above the step', () => {
    const snapshot = variant('host-ready');
    show({ ...snapshot, issue: { code: 'x', message: '호스트를 확인하지 못했습니다.', nextAction: '다시 확인하십시오.' } });
    expect(screen.getByRole('alert')).toHaveTextContent('호스트를 확인하지 못했습니다.');
    expect(screen.getByRole('alert')).toHaveTextContent('다시 확인하십시오.');
  });
});

describe('first-run wizard: S1.2 hypervisor consent', () => {
  it('lists the requirement, the three points and the caution', async () => {
    const actions = show(variant('whpx-consent'));
    expect(screen.getByRole('heading', { level: 1, name: '하이퍼바이저 활성화' })).toBeInTheDocument();
    expect(screen.getByText('Open Mobile Emulator를 사용하려면 하이퍼바이저를 활성화해야 합니다.')).toBeInTheDocument();
    expect(screen.getByText('하이퍼바이저를 활성화합니다.')).toBeInTheDocument();
    expect(screen.getByText('활성화 시 관리자 권한이 필요합니다.')).toBeInTheDocument();
    expect(screen.getByText('활성화 후 다시 시작해야 할 수 있습니다. 작업을 미리 저장해두십시오.')).toBeInTheDocument();
    expect(screen.getByText('활성화하면 같은 PC에서 커널 안티치트를 쓰는 게임이 실행되지 않을 수 있습니다.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '활성화' }));
    expect(actions.whpxEnable).toHaveBeenCalledOnce();
  });

  it.each([false, true])('leaves by button or Esc and says so, whatever canSkip is (%s)', async (canSkip) => {
    const snapshot = variant('whpx-consent');
    const actions = show({ ...snapshot, wizard: { ...snapshot.wizard, canSkip } });
    expect(screen.getByText('Esc 키로도 나갈 수 있습니다.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '지금은 건너뛰기' }));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(actions.wizardDefer).toHaveBeenCalledTimes(2);
    expect(actions.wizardSkip).not.toHaveBeenCalled();
  });
});

describe('first-run wizard: S1.3 restart', () => {
  it('states that the hypervisor is on and what happens after the restart', () => {
    show(variant('reboot-pending'));
    expect(screen.getByText('하이퍼바이저를 활성화했습니다.')).toBeInTheDocument();
    expect(screen.getByText('PC를 다시 시작한 뒤 앱을 다시 열면 이어서 진행합니다.')).toBeInTheDocument();
  });

  it('closes the app with 닫기 and offers no restart button', async () => {
    const actions = show(variant('reboot-pending'));
    expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual(['나중에 하기', '닫기']);
    await userEvent.click(screen.getByRole('button', { name: '닫기' }));
    expect(actions.appQuit).toHaveBeenCalledOnce();
  });
});

describe('first-run wizard: S1.4 image download', () => {
  it('shows the image cards with the Rust selection and sends a new choice', async () => {
    const actions = show(variant('download-idle'));
    expect(screen.getByRole('heading', { level: 1, name: '운영체제 이미지 다운로드' })).toBeInTheDocument();
    expect(screen.getByText('운영체제 이미지를 공식 배포처에서 다운로드합니다. 연결이 끊겨도 다시 이어받을 수 있습니다.')).toBeInTheDocument();
    const cards = screen.getAllByRole('radio');
    expect(cards.map((card) => card.getAttribute('aria-checked'))).toEqual(['false', 'true']);
    expect(within(cards[1] as HTMLElement).getByText('검증됨')).toBeInTheDocument();
    expect(within(cards[0] as HTMLElement).getByText('검증 전')).toBeInTheDocument();
    expect(screen.getByText('SourceForge 공식 프로젝트')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('radio', { name: /안드로이드 15/u }));
    expect(actions.guestImageSelect).toHaveBeenCalledWith('sample-android-15');
    await userEvent.click(screen.getByRole('button', { name: '다운로드' }));
    expect(actions.artifactDownloadStart).toHaveBeenCalledOnce();
  });

  it('shows progress, rate, remaining amount and the stage strip while downloading', async () => {
    const actions = show(variant('download-transferring'));
    expect(screen.getByText('guest-image-x86_64.iso')).toBeInTheDocument();
    expect(screen.getByText('1.42 GB / 2.26 GB')).toBeInTheDocument();
    expect(screen.getByText('12.4 MB/s')).toBeInTheDocument();
    expect(screen.getByText('0.84 GB')).toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: '다운로드 진행률' })).toHaveAttribute('aria-valuenow');
    const strip = screen.getByRole('list', { name: '진행' });
    expect(within(strip).getByText('다운로드 중')).toBeInTheDocument();
    expect(within(strip).getByText('무결성 확인')).toBeInTheDocument();
    expect(within(strip).getByText('완료')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '다음' })).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: '취소' }));
    expect(actions.artifactDownloadCancel).toHaveBeenCalledOnce();
  });

  it('continues after the integrity check', async () => {
    const actions = show(variant('download-verified'));
    expect(screen.queryByRole('button', { name: '다운로드' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: '다음' }));
    expect(actions.wizardContinue).toHaveBeenCalledOnce();
  });
});

describe('first-run wizard: S1.5 install', () => {
  it('creates the disk with the chosen size and shows free space', async () => {
    const actions = show(variant('install-disk'));
    expect(screen.getByRole('heading', { level: 1, name: '운영체제 설치' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: '64 GB' })).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByText('여유 공간 412 GB')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('radio', { name: '128 GB' }));
    await userEvent.click(screen.getByRole('button', { name: '디스크 만들기' }));
    expect(actions.guestCreate).toHaveBeenCalledWith('sample-android-13', 128);
  });

  it('shows the image profile install guide next to the stage', () => {
    show(variant('install-guide'));
    const guide = screen.getAllByRole('listitem').map((item) => item.textContent);
    expect(guide).toContain('1ISO 메뉴에서 Installation을 선택합니다.');
    expect(guide).toContain('6설치가 끝나면 가상 머신을 끄고 ISO 없이 다시 시작합니다.');
    expect(screen.getByRole('button', { name: '설치 완료' })).toBeDisabled();
  });
});

describe('first-run wizard: S1.6 first boot', () => {
  it('lists the boot lines and cancels the boot', async () => {
    const actions = show(variant('first-boot'));
    const steps = screen.getByRole('list', { name: '첫 부팅 진행' });
    for (const label of ['가상 머신 시작', '화면 연결', '운영체제 부팅', '기능 확인']) {
      expect(within(steps).getByText(label)).toBeInTheDocument();
    }
    expect(within(steps).getByText('운영체제 부팅').closest('li')).toHaveAttribute('aria-current', 'step');
    await userEvent.click(screen.getByRole('button', { name: '취소' }));
    expect(actions.guestStop).toHaveBeenCalledOnce();
  });

  it('says what to do after a failed boot and restarts', async () => {
    const actions = show(variant('first-boot-failed'));
    const steps = screen.getByRole('list', { name: '첫 부팅 진행' });
    const boot = within(steps).getByText('운영체제 부팅').closest('li');
    expect(boot).toHaveClass('ome-step-failed');
    expect(within(boot as HTMLElement).getByText('다시 시작을 시도할 수 있습니다. 반복될 경우 로그를 첨부해 문제를 보고하십시오.')).toBeInTheDocument();
    expect(within(steps).queryByText(/180|신호/)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '취소' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '다시 시작' }));
    expect(actions.guestStart).toHaveBeenCalledOnce();
  });

  it('marks probed capabilities and reports the applied defaults', async () => {
    const actions = show(variant('first-boot-done'));
    expect(screen.getByText('미디어 볼륨')).toBeInTheDocument();
    expect(screen.getByText('ARM 앱 실행')).toBeInTheDocument();
    expect(screen.getByText('미디어 볼륨과 기본 표시 프리셋을 적용했습니다.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '다음' }));
    expect(actions.wizardContinue).toHaveBeenCalledOnce();
  });
});

describe('first-run wizard: S1.7 app install', () => {
  it('shows the drop area, progress and results and sends each button', async () => {
    const actions = show(variant('app-install'));
    expect(screen.getByRole('heading', { level: 1, name: '앱 설치' })).toBeInTheDocument();
    expect(screen.getByText('앱 파일을 지금 설치할 수 있습니다.')).toBeInTheDocument();
    expect(screen.getByText('APK, XAPK, APKS 파일을 여기에 놓습니다.')).toBeInTheDocument();
    expect(screen.getByText('sample-app-d.xapk')).toBeInTheDocument();
    expect(screen.getByText('샘플 앱 A')).toBeInTheDocument();
    expect(screen.getByText('설치 완료')).toBeInTheDocument();
    expect(screen.getByText('앱 화면에서도 추가로 설치할 수 있습니다.')).toBeInTheDocument();
    expect(screen.getByText('7 / 7')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '파일 고르기' }));
    await userEvent.click(screen.getByRole('button', { name: '건너뛰기' }));
    await userEvent.click(screen.getByRole('button', { name: '완료' }));
    expect(actions.appInstallPick).toHaveBeenCalledOnce();
    expect(actions.wizardSkip).toHaveBeenCalledOnce();
    expect(actions.wizardContinue).toHaveBeenCalledOnce();
  });
});
