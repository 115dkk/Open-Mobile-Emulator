// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type {
  AppIssue, AppItem, AppSnapshot, BlockerKind, ControllerBridge, GuestImageSummary, GuestSummary, HostRow,
  TransferProgress,
} from './contracts';

export const sampleSnapshot: AppSnapshot = {
  contractVersion: 2, productVersion: '0.1.0', phase: 'wizard', blocker: null,
  host: { rows: [], ready: false, inspectedAt: null },
  wizard: {
    step: 'hostCheck', canContinue: false, canSkip: false, download: null,
    imageId: null, installGuide: [], diskSizeGib: 32, diskFreeBytes: null,
  },
  images: { profiles: [], guests: [], activeGuest: null },
  guest: {
    state: 'stopped', bootCompleted: false, adbConnected: false, hosting: 'none',
    resolution: null, lastExit: null, fps: null, startedAt: null, imageId: null,
    androidVersion: null, apiLevel: null, capabilities: { probedAt: null, items: [] },
    deviceId: null, adbAddress: '127.0.0.1:5555', rootEnabled: null,
  },
  apps: { available: false, items: [], install: null },
  input: {
    profiles: [], activeId: null, suspended: false, editing: false, autoApply: true,
    foregroundPackage: null, multitouch: 'unknown', suspendHotkey: 'F12',
  },
  display: {
    presets: [], activeId: null, custom: null, fit: 'fitWindow', refreshRateHz: null,
    refreshRates: [60, 75, 90, 120, 144], refreshSupported: false, vsync: 'off',
    vsyncSupported: false,
  },
  settings: {
    memoryMib: 8192, memoryMibMin: 4096, memoryMibMax: 16384,
    vcpus: 4, vcpusMax: 8, gpuMode: 'virgl', closeAction: 'stopGuest',
    showFps: false, autoUpdateCheck: false, homeDir: '', diskUsageBytes: null,
    adbAccess: 'localhost', bindingOverlayDefault: true,
  },
  update: { currentVersion: '0.1.0', state: { kind: 'idle' } },
  notices: [], issue: null,
};

export function fixtureBridge(
  snapshot: AppSnapshot = sampleSnapshot,
  failure: AppIssue | null = null,
): ControllerBridge {
  const read = () => Promise.resolve(snapshot);
  const apply = () => failure
    ? Promise.reject(Object.assign(new Error(failure.message), failure))
    : read();
  return {
    snapshot: read,
    hostCheckRefresh: apply,
    wizardContinue: apply,
    wizardSkip: apply,
    whpxEnable: apply,
    artifactDownloadStart: apply,
    artifactDownloadCancel: apply,
    guestImageSelect: apply,
    guestCreate: apply,
    guestSelect: apply,
    guestDelete: apply,
    guestReinstall: apply,
    guestStart: apply,
    guestStop: apply,
    guestRestart: apply,
    guestRootSet: apply,
    stageRectChanged: apply,
    screenshotSave: apply,
    appInstallPick: apply,
    appUninstall: apply,
    appLaunch: apply,
    inputProfileSelect: apply,
    inputSuspendToggle: apply,
    inputProfileDelete: apply,
    inputProfileSave: apply,
    inputBindingUpsert: apply,
    inputBindingRemove: apply,
    inputEditorToggle: apply,
    inputAutoApplySet: apply,
    inputSuspendHotkeySet: apply,
    displayPresetApply: apply,
    displayCustomApply: apply,
    displayRefreshSet: apply,
    displayVsyncSet: apply,
    stageFitSet: apply,
    settingsSave: apply,
    updateCheck: apply,
    updateInstall: apply,
    diagnosticsExport: apply,
    openLogsFolder: apply,
    openScreenshotsFolder: apply,
    openRegistrationPage: apply,
    guestWindowToFront: apply,
    watchSnapshot: () => Promise.resolve(() => Promise.resolve()),
    watchProgress: () => Promise.resolve(() => Promise.resolve()),
  };
}

// Snapshot variants for screen tests and the QA gallery. Values only; the types above are the
// contract. App names are neutral (R6). Host-row sentences are the ones ome-host-check emits.

const GIB = 1024 * 1024 * 1024;
const MIB = 1024 * 1024;

export const hostRowsReady: readonly HostRow[] = [
  { id: 'cpuVirtualization', status: 'ready', detail: '프로세서 가상화를 사용할 수 있습니다.' },
  { id: 'hypervisorPlatform', status: 'attention', detail: 'Windows 하이퍼바이저 플랫폼이 꺼져 있습니다. 다음 단계에서 동의하면 켭니다.' },
  { id: 'rebootPending', status: 'ready', detail: '적용을 기다리는 다시 시작 작업이 없습니다.' },
  { id: 'whpxAvailable', status: 'attention', detail: '가상화 실행 기능이 아직 준비되지 않았습니다. Windows 하이퍼바이저 플랫폼을 켜십시오.' },
  { id: 'qemuPresent', status: 'ready', detail: '가상 머신 실행 파일을 찾았습니다.' },
  { id: 'firmwarePresent', status: 'ready', detail: '가상 머신 시작에 필요한 펌웨어를 찾았습니다.' },
  { id: 'adbPresent', status: 'ready', detail: '운영체제 앱 관리 도구를 찾았습니다.' },
  { id: 'diskSpace', status: 'ready', detail: '운영체제 설치에 필요한 저장 공간이 있습니다.' },
];

export const hostRowsBlocked: readonly HostRow[] = hostRowsReady.map((row): HostRow => row.id === 'diskSpace'
  ? { id: 'diskSpace', status: 'blocked', detail: '저장 공간이 40GB보다 적습니다. 파일을 정리한 뒤 다시 확인하십시오.' }
  : row);

export const sampleProfiles: readonly GuestImageSummary[] = [
  {
    id: 'sample-android-15', displayName: '안드로이드 15', androidVersion: '15', apiLevel: 35,
    distribution: 'selfBuilt', translator: 'ndkTranslation', sizeBytes: Math.round(2.71 * GIB),
    status: 'candidate', releasedAt: '2026-09-10', verifiedGames: 0, recommended: false,
  },
  {
    id: 'sample-android-13', displayName: '안드로이드 13', androidVersion: '13', apiLevel: 33,
    distribution: 'bliss', translator: 'ndkTranslation', sizeBytes: Math.round(2.26 * GIB),
    status: 'verified', releasedAt: '2024-10-11', verifiedGames: 1, recommended: true,
  },
];

export const sampleInstallGuide: readonly string[] = [
  'ISO 메뉴에서 Installation을 선택합니다.',
  '빈 디스크에 GPT 파티션 표와 512MB EFI 시스템 파티션을 만듭니다.',
  '남은 공간에 Linux filesystem 파티션을 만들고 변경 사항을 기록합니다.',
  'EFI 파티션은 FAT32로, 운영체제 파티션은 ext4로 포맷합니다.',
  'OTA 업데이트는 No를 고르고 Grub2 EFI Bootloader를 설치합니다.',
  '설치가 끝나면 가상 머신을 끄고 ISO 없이 다시 시작합니다.',
];

export const sampleApps: readonly AppItem[] = [
  { package: 'com.example.sample.a', label: '샘플 앱 A', versionName: '1.4.2', installedAt: '2026-09-24T10:00:00' },
  { package: 'com.example.sample.b', label: '샘플 앱 B', versionName: '3.0.1', installedAt: '2026-09-25T10:00:00' },
  { package: 'com.example.sample.c', label: '샘플 앱 C', versionName: '0.9.0', installedAt: '2026-09-26T10:00:00' },
];

const sampleGuest: GuestSummary = {
  name: 'android-13', imageId: 'sample-android-13', androidVersion: '13', diskSizeGib: 64,
  lastStartedAt: null, capabilities: { probedAt: null, items: [] },
};

function transfer(stage: TransferProgress['stage'], doneRatio: number, label: string, total: number): TransferProgress {
  return {
    stage, doneBytes: Math.round(total * doneRatio), totalBytes: total,
    bytesPerSecond: stage === 'transferring' ? Math.round(12.4 * MIB) : null, label,
  };
}

const imageTotal = Math.round(2.26 * GIB);
const imageFile = 'guest-image-x86_64.iso';
const wizardBase: AppSnapshot = {
  ...sampleSnapshot,
  host: { rows: hostRowsReady, ready: true, inspectedAt: '2026-09-26T19:52:00' },
  images: { profiles: sampleProfiles, guests: [], activeGuest: null },
  wizard: { ...sampleSnapshot.wizard, imageId: 'sample-android-13' },
};

type WizardPatch = Omit<Partial<AppSnapshot>, 'wizard'> & { readonly wizard?: Partial<AppSnapshot['wizard']> };

function wizardAt(step: AppSnapshot['wizard']['step'], patch: WizardPatch = {}): AppSnapshot {
  return { ...wizardBase, ...patch, wizard: { ...wizardBase.wizard, ...patch.wizard, step } };
}

const installedImages: AppSnapshot['images'] = { profiles: sampleProfiles, guests: [sampleGuest], activeGuest: sampleGuest.name };

export interface GalleryVariant {
  readonly id: string;
  readonly label: string;
  readonly snapshot: AppSnapshot;
}

export const wizardGallery: readonly GalleryVariant[] = [
  { id: 'host-ready', label: 'S1.1 호스트 점검: 사용 가능', snapshot: wizardAt('hostCheck', { wizard: { canContinue: true } }) },
  {
    id: 'host-blocked', label: 'S1.1 호스트 점검: 사용 불가',
    snapshot: wizardAt('hostCheck', { host: { rows: hostRowsBlocked, ready: false, inspectedAt: '2026-09-26T19:52:00' } }),
  },
  { id: 'whpx-consent', label: 'S1.2 하이퍼바이저 활성화', snapshot: wizardAt('whpxConsent') },
  { id: 'reboot-pending', label: 'S1.3 다시 시작 필요', snapshot: wizardAt('rebootPending', { wizard: { canContinue: true } }) },
  { id: 'download-idle', label: 'S1.4 이미지: 다운로드 전', snapshot: wizardAt('artifactDownload') },
  {
    id: 'download-transferring', label: 'S1.4 이미지: 다운로드 중',
    snapshot: wizardAt('artifactDownload', { wizard: { download: transfer('transferring', 0.628, imageFile, imageTotal) } }),
  },
  {
    id: 'download-verifying', label: 'S1.4 이미지: 무결성 확인',
    snapshot: wizardAt('artifactDownload', { wizard: { download: transfer('verifying', 1, imageFile, imageTotal) } }),
  },
  {
    id: 'download-verified', label: 'S1.4 이미지: 완료',
    snapshot: wizardAt('artifactDownload', {
      wizard: { canContinue: true, download: transfer('verified', 1, imageFile, imageTotal) },
    }),
  },
  {
    id: 'install-disk', label: 'S1.5 설치: 디스크 크기',
    snapshot: wizardAt('guestInstall', { wizard: { diskSizeGib: 64, diskFreeBytes: 412 * GIB } }),
  },
  {
    id: 'install-guide', label: 'S1.5 설치: 설치 안내',
    snapshot: wizardAt('guestInstall', {
      images: installedImages,
      guest: { ...sampleSnapshot.guest, state: 'running', hosting: 'embedded', imageId: 'sample-android-13' },
      wizard: { installGuide: sampleInstallGuide, diskSizeGib: 64 },
    }),
  },
  {
    id: 'first-boot', label: 'S1.6 첫 부팅: 부팅 중',
    snapshot: wizardAt('firstBoot', {
      images: installedImages,
      guest: { ...sampleSnapshot.guest, state: 'starting', hosting: 'embedded', imageId: 'sample-android-13' },
    }),
  },
  {
    id: 'first-boot-done', label: 'S1.6 첫 부팅: 기능 확인 끝',
    snapshot: wizardAt('firstBoot', {
      images: installedImages,
      wizard: { canContinue: true },
      guest: {
        ...sampleSnapshot.guest, state: 'running', hosting: 'embedded', bootCompleted: true, adbConnected: true,
        imageId: 'sample-android-13', androidVersion: '13', apiLevel: 33,
        capabilities: {
          probedAt: '2026-09-26T20:01:00',
          items: [
            { id: 'bootMarker', state: 'available' }, { id: 'appList', state: 'available' },
            { id: 'displaySize', state: 'available' }, { id: 'mediaVolume', state: 'available' },
            { id: 'deviceId', state: 'available' }, { id: 'screenshot', state: 'available' },
            { id: 'foregroundApp', state: 'available' }, { id: 'multitouch', state: 'unavailable' },
            { id: 'nativeBridge', state: 'available' }, { id: 'root', state: 'unknown' },
          ],
        },
      },
    }),
  },
  {
    id: 'app-install', label: 'S1.7 앱 설치',
    snapshot: wizardAt('appInstall', {
      images: installedImages,
      wizard: { canContinue: true, canSkip: true },
      guest: { ...sampleSnapshot.guest, state: 'running', hosting: 'embedded', bootCompleted: true, adbConnected: true },
      apps: { available: true, items: sampleApps.slice(0, 1), install: transfer('transferring', 0.41, 'sample-app-d.xapk', 180 * MIB) },
    }),
  },
];

function blocked(kind: BlockerKind): AppSnapshot {
  return { ...sampleSnapshot, phase: 'main', blocker: { kind }, host: { rows: hostRowsReady, ready: false, inspectedAt: null } };
}

export const blockedGallery: readonly GalleryVariant[] = [
  { id: 'blocked-virtualization', label: 'S8 가상화 꺼짐', snapshot: blocked('virtualizationOff') },
  { id: 'blocked-qemu', label: 'S8 가상 머신 파일 없음', snapshot: blocked('qemuMissing') },
  { id: 'blocked-hypervisor', label: 'S8 하이퍼바이저 꺼짐', snapshot: blocked('hypervisorPlatformOff') },
];

/** After the wizard: the rail shell with a stopped operating system. */
export const mainSnapshot: AppSnapshot = {
  ...sampleSnapshot,
  phase: 'main',
  host: { rows: hostRowsReady, ready: true, inspectedAt: '2026-09-26T19:52:00' },
  wizard: { ...sampleSnapshot.wizard, step: 'done' },
  images: installedImages,
  guest: {
    ...sampleSnapshot.guest, imageId: 'sample-android-13', androidVersion: '13', apiLevel: 33,
    resolution: { width: 1920, height: 1080 },
    lastExit: { kind: 'userStop', at: '2026-09-26T19:58:00', logPath: null },
  },
  apps: { available: false, items: sampleApps, install: null },
};

export const mainGallery: readonly GalleryVariant[] = [
  { id: 'main-stopped', label: 'S2 무대: 꺼짐', snapshot: mainSnapshot },
];
