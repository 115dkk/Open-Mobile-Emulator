// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Words and tones for snapshot values. Pure mapping from snapshot fields to what a screen draws;
// nothing here decides whether an action is allowed.
import type {
  CapabilityId, GuestState, GuestView, HostCheckId, HostRow, HostStatus, ImageStatus,
} from './contracts';

export type StatusTone = 'muted' | 'accent' | 'success' | 'warning' | 'danger';
export type StepState = 'done' | 'active' | 'pending' | 'failed';

/** Noun phrases next to the guest status dot (M2-SCREENS.md 2, 9). */
export const GUEST_STATE_LABEL: Readonly<Record<GuestState, string>> = {
  stopped: '시작 가능',
  starting: '부팅 중',
  running: '실행 중',
  stopping: '끄는 중',
  restarting: '다시 시작 중',
  failed: '실패',
};

export const GUEST_STATE_TONE: Readonly<Record<GuestState, StatusTone>> = {
  stopped: 'muted',
  starting: 'accent',
  running: 'success',
  stopping: 'warning',
  restarting: 'warning',
  failed: 'danger',
};

export const HOST_STATUS_TONE: Readonly<Record<HostStatus, StatusTone>> = {
  ready: 'success',
  attention: 'warning',
  blocked: 'danger',
};

/** Host-check rows as S1.1 lists them. Rust reports eight probes; the screen shows six labels. */
const HOST_GROUPS: readonly { readonly label: string; readonly ids: readonly HostCheckId[] }[] = [
  { label: 'CPU 가상화', ids: ['cpuVirtualization'] },
  { label: 'Windows 하이퍼바이저 플랫폼', ids: ['hypervisorPlatform', 'whpxAvailable'] },
  { label: '다시 시작 대기', ids: ['rebootPending'] },
  { label: '가상 머신 구성 요소', ids: ['qemuPresent', 'firmwarePresent'] },
  { label: '앱 설치 도구', ids: ['adbPresent'] },
  { label: '디스크 여유 공간', ids: ['diskSpace'] },
];

const STATUS_RANK: Readonly<Record<HostStatus, number>> = { ready: 0, attention: 1, blocked: 2 };

export interface HostLine {
  readonly label: string;
  readonly status: HostStatus;
  readonly tone: StatusTone;
  /** Rust's sentence for the most severe row of the group, verbatim. */
  readonly detail: string;
}

/** Groups Rust's host rows under the S1.1 labels. The line shows its most severe row. */
export function hostLines(rows: readonly HostRow[]): HostLine[] {
  const lines: HostLine[] = [];
  for (const group of HOST_GROUPS) {
    let worst: HostRow | undefined;
    for (const row of rows) {
      if (!group.ids.includes(row.id)) continue;
      if (worst === undefined || STATUS_RANK[row.status] > STATUS_RANK[worst.status]) worst = row;
    }
    if (worst === undefined) continue;
    // Nothing waiting for a restart is not an achievement; the mockup draws it dimmed.
    const tone = worst.id === 'rebootPending' && worst.status === 'ready' ? 'muted' : HOST_STATUS_TONE[worst.status];
    lines.push({ label: group.label, status: worst.status, tone, detail: worst.detail });
  }
  return lines;
}

/** Capability probe items (ADR-0004) in the user's words. */
export const CAPABILITY_LABEL: Readonly<Record<CapabilityId, string>> = {
  bootMarker: '부팅 완료 신호',
  appList: '앱 목록',
  displaySize: '화면 크기와 밀도',
  mediaVolume: '미디어 볼륨',
  deviceId: '기기 ID',
  screenshot: '스크린샷',
  foregroundApp: '전경 앱',
  multitouch: '멀티터치',
  nativeBridge: 'ARM 앱 실행',
  root: '루트 권한',
};

export const IMAGE_STATUS_LABEL: Readonly<Record<ImageStatus, string>> = {
  verified: '검증됨',
  candidate: '검증 전',
  deprecated: '지원 종료',
};

export const IMAGE_STATUS_TONE: Readonly<Record<ImageStatus, StatusTone>> = {
  verified: 'success',
  candidate: 'warning',
  deprecated: 'muted',
};

export interface BootStep {
  readonly id: 'vm' | 'screen' | 'boot' | 'probe';
  readonly label: string;
  readonly state: StepState;
}

/**
 * Boot progress lines for S1.6 (four lines) and S2 `Starting` (first three). Each line is done when
 * the snapshot shows its fact; the first line not done is active while the guest is starting or
 * running and failed when the guest failed.
 */
export function bootSteps(guest: GuestView, includeProbe: boolean): BootStep[] {
  const screen = guest.hosting !== 'none';
  const booted = guest.bootCompleted;
  const vm = screen || booted || guest.state === 'running';
  const facts: { id: BootStep['id']; label: string; done: boolean }[] = [
    { id: 'vm', label: '가상 머신 시작', done: vm },
    { id: 'screen', label: '화면 연결', done: screen },
    { id: 'boot', label: '운영체제 부팅', done: booted },
  ];
  if (includeProbe) facts.push({ id: 'probe', label: '기능 확인', done: guest.capabilities.probedAt !== null });
  const moving = guest.state === 'starting' || guest.state === 'running' || guest.state === 'restarting';
  let current = false;
  return facts.map(({ id, label, done }) => {
    if (done) return { id, label, state: 'done' };
    if (current) return { id, label, state: 'pending' };
    current = true;
    if (guest.state === 'failed') return { id, label, state: 'failed' };
    return { id, label, state: moving ? 'active' : 'pending' };
  });
}
