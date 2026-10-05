// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type {
  AppIssue, AppItem, AppSnapshot, BlockerKind, ControllerBridge, GuestImageSummary, GuestSummary, GuestView, HostRow, InstallProgress,
  TransferProgress,
} from './contracts';
import type { OverlaySeed } from './overlay/model';

export const sampleSnapshot: AppSnapshot = {
  contractVersion: 10, productVersion: '0.1.0', phase: 'wizard', blocker: null,
  host: { rows: [], ready: false, inspectedAt: null },
  wizard: {
    step: 'hostCheck', canContinue: false, canSkip: false, download: null,
    imageId: null, install: null, diskSizeGib: 32, diskFreeBytes: null,
  },
  images: { profiles: [], guests: [], activeGuest: null },
  guest: {
    state: 'stopped', bootCompleted: false, adbConnected: false, hosting: 'none',
    resolution: null, lastExit: null, fps: null, startedAt: null, imageId: null,
    androidVersion: null, apiLevel: null, capabilities: { probedAt: null, items: [] },
    deviceId: null, deviceIdDecimal: null, googleAccounts: null, registrationOpenedAt: null,
    addAccountSupported: false, pid: null, adbAddress: '127.0.0.1:5555', rootEnabled: null, mediaVolume: null,
  },
  apps: { available: false, items: [], install: null },
  input: {
    profiles: [], activeId: null, suspended: false, editing: false, autoApply: true,
    foregroundPackage: null, multitouch: 'unknown', suspendHotkey: 'F12', overlayVisible: true,
  },
  textInput: { state: 'unavailable', inputType: null, package: null },
  display: {
    presets: [], activeId: null, custom: null, fit: 'fitWindow', refreshRateHz: null,
    refreshRates: [60, 75, 90, 120, 144], refreshSupported: false, vsync: 'off',
    vsyncSupported: false,
  },
  settings: {
    memoryMib: 8192, memoryMibMin: 4096, memoryMibMax: 16384,
    vcpus: 4, vcpusMax: 8, gpuMode: 'virgl', closeAction: 'stopGuest',
    showFps: false, autoUpdateCheck: false, homeDir: '', diskUsageBytes: null,
    screenshotsDir: '', sharedDir: '', sharedPush: null,
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
    wizardDefer: apply,
    openHelp: apply,
    appQuit: apply,
    whpxEnable: apply,
    artifactDownloadStart: apply,
    artifactDownloadCancel: apply,
    guestImageSelect: apply,
    guestCreate: apply,
    guestSelect: apply,
    guestDelete: apply,
    guestReinstall: apply,
    guestInstallCancel: apply,
    guestStart: apply,
    guestStop: apply,
    guestRestart: apply,
    guestRootSet: apply,
    guestVolumeSet: apply,
    stageRectChanged: apply,
    stageHidden: apply,
    screenshotSave: apply,
    appInstallPick: apply,
    appInstallCancel: apply,
    appUninstall: apply,
    appLaunch: apply,
    inputHostKey: () => Promise.resolve(),
    inputProfileSelect: apply,
    inputSuspendToggle: apply,
    inputOverlayToggle: apply,
    inputProfileDelete: apply,
    inputProfileSave: apply,
    inputBindingUpsert: apply,
    inputBindingRemove: apply,
    inputEditorToggle: apply,
    inputAutoApplySet: apply,
    inputSuspendHotkeySet: apply,
    textCompose: () => Promise.resolve(),
    textCommit: () => Promise.resolve(),
    textKey: () => Promise.resolve(),
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
    openHomeFolder: apply,
    openInstallFolder: apply,
    openSharedFolder: apply,
    sharedPush: apply,
    copyToClipboard: apply,
    openRegistrationPage: apply,
    googleAccountAddOpen: apply,
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
  { id: 'cpuVirtualization', status: 'ready', detail: '사용할 수 있습니다.' },
  { id: 'hypervisorPlatform', status: 'attention', detail: '꺼져 있습니다. 다음 단계에서 동의하면 켭니다.' },
  { id: 'rebootPending', status: 'ready', detail: '필요하지 않습니다.' },
  { id: 'whpxAvailable', status: 'attention', detail: '아직 실행 준비가 되지 않았습니다. 다음 단계에서 켜면 사용할 수 있습니다.' },
  { id: 'qemuPresent', status: 'ready', detail: '준비되어 있습니다.' },
  { id: 'firmwarePresent', status: 'ready', detail: '준비되어 있습니다.' },
  { id: 'adbPresent', status: 'ready', detail: '준비되어 있습니다.' },
  { id: 'diskSpace', status: 'ready', detail: '설치에 충분한 여유가 있습니다.' },
];

export const hostRowsBlocked: readonly HostRow[] = hostRowsReady.map((row): HostRow => row.id === 'diskSpace'
  ? { id: 'diskSpace', status: 'blocked', detail: '여유가 40 GB보다 적습니다. 파일을 정리한 뒤 다시 확인하십시오.' }
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

/** Serial log of the hidden install helper, kept in the guest directory (ADR-0010). */
const installLogPath = 'C:\\Users\\사용자\\AppData\\Local\\OpenMobileEmulator\\vm\\android-13\\install-serial.log';

export const sampleApps: readonly AppItem[] = [
  { package: 'com.example.sample.a', label: '샘플 앱 A', versionName: '1.4.2', versionCode: 142, installedAt: '2026-09-24T10:00:00' },
  { package: 'com.example.sample.b', label: '샘플 앱 B', versionName: '3.0.1', versionCode: 301, installedAt: '2026-09-25T10:00:00' },
  { package: 'com.example.sample.c', label: '샘플 앱 C', versionName: '0.9.0', versionCode: 90, installedAt: '2026-09-26T10:00:00' },
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

/** An app install in flight: the second of two files, with the byte ratio adb reports. */
function installing(label: string, ratio: number, total: number): InstallProgress {
  return {
    label, stage: 'transferring', doneItems: 1, totalItems: 2, ratio,
    doneBytes: Math.round(total * ratio), totalBytes: total,
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

const stageLogPath = 'C:\\Users\\사용자\\AppData\\Local\\OpenMobileEmulator\\logs\\guest-20260926-1958.log';

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
    id: 'install-progress', label: 'S1.5 설치: 설치 중',
    snapshot: wizardAt('guestInstall', {
      images: installedImages,
      guest: { ...sampleSnapshot.guest, state: 'running', imageId: 'sample-android-13' },
      wizard: { diskSizeGib: 64, install: { stage: 'copy', percent: 58, failure: null, logPath: installLogPath } },
    }),
  },
  {
    id: 'install-failed', label: 'S1.5 설치: 설치 실패',
    snapshot: wizardAt('guestInstall', {
      images: installedImages,
      guest: { ...sampleSnapshot.guest, state: 'failed', imageId: 'sample-android-13' },
      wizard: { diskSizeGib: 64, install: { stage: 'failed', percent: 10, failure: 'mke2fs', logPath: installLogPath } },
    }),
  },
  {
    id: 'install-installed', label: 'S1.5 설치: 이미 설치됨',
    snapshot: wizardAt('guestInstall', { images: installedImages, wizard: { diskSizeGib: 64, canContinue: true } }),
  },
  {
    id: 'first-boot', label: 'S1.6 첫 부팅: 부팅 중',
    snapshot: wizardAt('firstBoot', {
      images: installedImages,
      guest: { ...sampleSnapshot.guest, state: 'starting', hosting: 'embedded', imageId: 'sample-android-13' },
    }),
  },
  {
    id: 'first-boot-hidden', label: 'S1.6 첫 부팅: 창 숨김, 운영체제 부팅 중',
    snapshot: wizardAt('firstBoot', {
      images: installedImages,
      guest: {
        ...sampleSnapshot.guest, state: 'running', hosting: 'embedded', imageId: 'sample-android-13',
        startedAt: new Date(Date.now() - 75_000).toISOString(),
      },
    }),
  },
  {
    id: 'first-boot-failed', label: 'S1.6 첫 부팅: 부팅 실패',
    snapshot: wizardAt('firstBoot', {
      images: installedImages,
      guest: {
        ...sampleSnapshot.guest, state: 'failed', imageId: 'sample-android-13',
        lastExit: { kind: 'bootTimeout', at: '2026-09-26T19:58:00', logPath: null },
      },
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
      apps: { available: true, items: sampleApps.slice(0, 1), install: installing('sample-app-d.xapk', 0.41, 180 * MIB) },
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
  { id: 'main-stopped', label: 'S2 화면: 꺼짐', snapshot: mainSnapshot },
];

// ---- Stage (S2) and apps (S3) variants: screen worker B. Other workers append their own blocks. ----

export const stageDisplayPresets: AppSnapshot['display']['presets'] = [
  { id: 'hd-landscape', size: { width: 1280, height: 720 }, densityDpi: 240, orientation: 'landscape', needsReboot: false },
  { id: 'fhd-landscape', size: { width: 1920, height: 1080 }, densityDpi: 320, orientation: 'landscape', needsReboot: false },
  { id: 'hd-portrait', size: { width: 720, height: 1280 }, densityDpi: 240, orientation: 'portrait', needsReboot: true },
];

export const stageInputProfile: AppSnapshot['input']['profiles'][number] = {
  id: 'user-1', name: '내 프로필 1', bundled: false, targetPackage: 'com.example.sample.a',
  referenceAspect: { width: 16, height: 9 }, anchor: 'center', bindings: [],
};

const stageProbe: AppSnapshot['guest']['capabilities'] = {
  probedAt: '2026-09-26T20:01:00',
  items: [
    { id: 'bootMarker', state: 'available' }, { id: 'appList', state: 'available' },
    { id: 'displaySize', state: 'available' }, { id: 'mediaVolume', state: 'available' },
    { id: 'deviceId', state: 'available' }, { id: 'screenshot', state: 'available' },
    { id: 'foregroundApp', state: 'available' }, { id: 'multitouch', state: 'unavailable' },
    { id: 'nativeBridge', state: 'available' }, { id: 'root', state: 'unknown' },
  ],
};

/** The rail shell with presets and an input profile, the operating system stopped after a normal exit. */
export const stageSnapshot: AppSnapshot = {
  ...mainSnapshot,
  display: { ...mainSnapshot.display, presets: stageDisplayPresets, activeId: 'fhd-landscape' },
  input: { ...mainSnapshot.input, profiles: [stageInputProfile], activeId: stageInputProfile.id },
};

type GuestPatch = Partial<AppSnapshot['guest']>;
type StagePatch = Omit<Partial<AppSnapshot>, 'guest'>;

function stageAt(guest: GuestPatch, patch: StagePatch = {}): AppSnapshot {
  return { ...stageSnapshot, ...patch, guest: { ...stageSnapshot.guest, ...guest } };
}

const runningGuest: GuestPatch = {
  state: 'running', hosting: 'embedded', bootCompleted: true, adbConnected: true, fps: 58,
  startedAt: '2026-09-27T09:10:00', capabilities: stageProbe, deviceId: '3f2a9c41d07b5e68', rootEnabled: false,
  mediaVolume: 9,
};

const runningApps: AppSnapshot['apps'] = { available: true, items: sampleApps, install: null };

function failedAt(kind: 'startFailed' | 'bootTimeout' | 'crash'): AppSnapshot {
  return stageAt({ state: 'failed', lastExit: { kind, at: '2026-09-26T19:58:00', logPath: stageLogPath } });
}

/** A running operating system with apps, for the apps screen and the stage. */
export const runningSnapshot: AppSnapshot = stageAt(runningGuest, { apps: runningApps });

export const stageGallery: readonly GalleryVariant[] = [
  { id: 'stage-stopped', label: 'S2 화면: 꺼짐, 정상 종료', snapshot: stageSnapshot },
  {
    id: 'stage-stopped-abnormal', label: 'S2 화면: 꺼짐, 비정상 종료',
    snapshot: stageAt({ lastExit: { kind: 'crash', at: '2026-09-26T19:58:00', logPath: stageLogPath } }),
  },
  {
    id: 'stage-starting', label: 'S2 화면: 시작 중',
    snapshot: stageAt({ state: 'starting', hosting: 'embedded', lastExit: null }),
  },
  {
    // The guest window stays hidden until Android reports boot completion; the counter runs from
    // the start time, so this variant starts 42 seconds before the gallery opens.
    id: 'stage-booting', label: 'S2 화면: 실행 중, 부팅 중(창 숨김)',
    snapshot: stageAt({
      state: 'running', hosting: 'embedded', bootCompleted: false, adbConnected: false, lastExit: null,
      startedAt: new Date(Date.now() - 42_000).toISOString(),
    }),
  },
  { id: 'stage-running', label: 'S2 화면: 실행 중', snapshot: runningSnapshot },
  {
    id: 'stage-running-auto', label: 'S2 화면: 실행 중, 자동 적용과 일시 중지, fps',
    snapshot: stageAt(runningGuest, {
      apps: runningApps,
      input: { ...stageSnapshot.input, foregroundPackage: 'com.example.sample.a', suspended: true },
      settings: { ...stageSnapshot.settings, showFps: true },
    }),
  },
  {
    id: 'stage-running-editing', label: 'S2 화면: 실행 중, 매핑 편집',
    snapshot: stageAt(runningGuest, { apps: runningApps, input: { ...stageSnapshot.input, editing: true } }),
  },
  {
    id: 'stage-running-separate', label: 'S2 화면: 실행 중, 별도 창',
    snapshot: stageAt({ ...runningGuest, hosting: 'separateWindow' }, { apps: runningApps }),
  },
  {
    id: 'stage-restarting', label: 'S2 화면: 다시 시작 중',
    snapshot: stageAt({ ...runningGuest, state: 'restarting', bootCompleted: false, adbConnected: false, fps: null }),
  },
  {
    id: 'stage-stopping', label: 'S2 화면: 끄는 중',
    snapshot: stageAt({ ...runningGuest, state: 'stopping', adbConnected: false, fps: null }),
  },
  { id: 'stage-failed-start', label: 'S2 화면: 실패, 가상 머신 시작', snapshot: failedAt('startFailed') },
  { id: 'stage-failed-boot', label: 'S2 화면: 실패, 운영체제 부팅', snapshot: failedAt('bootTimeout') },
  { id: 'stage-failed-crash', label: 'S2 화면: 실패, 크래시', snapshot: failedAt('crash') },
  {
    id: 'stage-issue', label: 'S2 화면: 오류 알림',
    snapshot: {
      ...runningSnapshot,
      issue: { code: 'screenshot_failed', message: '스크린샷을 저장하지 못했습니다.', nextAction: '저장 위치의 여유 공간을 확인하십시오.' },
    },
  },
];

export const appsGallery: readonly GalleryVariant[] = [
  { id: 'apps-running', label: 'S3 앱: 목록', snapshot: runningSnapshot },
  {
    id: 'apps-installing', label: 'S3 앱: 설치 중',
    snapshot: {
      ...runningSnapshot,
      apps: { ...runningApps, install: installing('sample-app-d.xapk', 0.41, 180 * MIB) },
    },
  },
  { id: 'apps-stopped', label: 'S3 앱: 운영체제 꺼짐', snapshot: { ...stageSnapshot, apps: { available: false, items: sampleApps, install: null } } },
  { id: 'apps-empty', label: 'S3 앱: 앱 없음', snapshot: { ...runningSnapshot, apps: { available: true, items: [], install: null } } },
];

// ---- Input (S4), display (S5) and settings (S6) variants: screen worker C. Values only; profile,
// preset and guest shapes are read off AppSnapshot so the contract import above stays untouched. ----

type InputProfileFixture = AppSnapshot['input']['profiles'][number];
type DisplayPresetFixture = AppSnapshot['display']['presets'][number];
type GuestSummaryFixture = AppSnapshot['images']['guests'][number];

const wide = { width: 16, height: 9 } as const;

export const sampleInputProfiles: readonly InputProfileFixture[] = [
  {
    id: 'bundled-sample-a', name: '샘플 앱 A 기본 입력', bundled: true, targetPackage: 'com.example.sample.a',
    referenceAspect: wide, anchor: 'center',
    bindings: [
      { id: 'space-tap', trigger: { kind: 'key', code: 'Space' }, action: { kind: 'tap', at: { x: 0.5, y: 0.5 }, hold: false } },
      {
        id: 'left-swipe', trigger: { kind: 'key', code: 'ArrowLeft' },
        action: { kind: 'swipe', from: { x: 0.65, y: 0.5 }, to: { x: 0.35, y: 0.5 }, durationMs: 240 },
      },
      {
        id: 'right-swipe', trigger: { kind: 'key', code: 'ArrowRight' },
        action: { kind: 'swipe', from: { x: 0.35, y: 0.5 }, to: { x: 0.65, y: 0.5 }, durationMs: 240 },
      },
    ],
  },
  {
    id: 'user-1', name: '내 프로필 1', bundled: false, targetPackage: 'com.example.sample.b',
    referenceAspect: wide, anchor: 'center',
    bindings: [
      {
        id: 'move', trigger: { kind: 'keySet', up: 'KeyW', down: 'KeyS', left: 'KeyA', right: 'KeyD' },
        action: { kind: 'joystick', center: { x: 0.16, y: 0.72 }, radius: 0.16 },
      },
      { id: 'skill', trigger: { kind: 'key', code: 'KeyQ' }, action: { kind: 'tap', at: { x: 0.84, y: 0.42 }, hold: false } },
      { id: 'guard', trigger: { kind: 'key', code: 'KeyE' }, action: { kind: 'tap', at: { x: 0.9, y: 0.7 }, hold: true } },
      {
        id: 'dash', trigger: { kind: 'key', code: 'Space' },
        action: { kind: 'swipe', from: { x: 0.62, y: 0.8 }, to: { x: 0.76, y: 0.8 }, durationMs: 200 },
      },
      { id: 'menu', trigger: { kind: 'mouseButton', button: 'right' }, action: { kind: 'mouseTap', at: { x: 0.95, y: 0.08 } } },
      { id: 'touch', trigger: { kind: 'mouseButton', button: 'left' }, action: { kind: 'mouseTap', at: null } },
      { id: 'scroll-up', trigger: { kind: 'wheel', direction: 'up' }, action: { kind: 'wheelSwipe', at: { x: 0.5, y: 0.4 }, distance: 0.2 } },
      { id: 'back', trigger: { kind: 'key', code: 'Escape' }, action: { kind: 'passThrough' } },
    ],
  },
  {
    id: 'user-2', name: '내 프로필 2', bundled: false, targetPackage: null, referenceAspect: wide, anchor: 'edges',
    bindings: [
      { id: 'confirm', trigger: { kind: 'key', code: 'Digit1' }, action: { kind: 'tap', at: { x: 0.5, y: 0.86 }, hold: false } },
    ],
  },
];

/** The three presets Rust exposes. Only the portrait card changes orientation from a landscape screen. */
export const sampleDisplayPresets: readonly DisplayPresetFixture[] = [
  { id: 'hd-720', size: { width: 1280, height: 720 }, densityDpi: 160, orientation: 'landscape', needsReboot: false },
  { id: 'full-hd', size: { width: 1920, height: 1080 }, densityDpi: 240, orientation: 'landscape', needsReboot: false },
  { id: 'portrait-720', size: { width: 720, height: 1280 }, densityDpi: 160, orientation: 'portrait', needsReboot: true },
];

const probedCapabilities: AppSnapshot['guest']['capabilities'] = {
  probedAt: '2026-09-26T20:01:00',
  items: [
    { id: 'bootMarker', state: 'available' }, { id: 'appList', state: 'available' },
    { id: 'displaySize', state: 'available' }, { id: 'mediaVolume', state: 'available' },
    { id: 'deviceId', state: 'available' }, { id: 'screenshot', state: 'available' },
    { id: 'foregroundApp', state: 'available' }, { id: 'multitouch', state: 'unavailable' },
    { id: 'nativeBridge', state: 'available' }, { id: 'root', state: 'available' },
  ],
};

const settingsGuests: readonly GuestSummaryFixture[] = [
  {
    name: 'android-13', imageId: 'sample-android-13', androidVersion: '13', diskSizeGib: 64,
    lastStartedAt: '2026-09-26T19:58:04', capabilities: probedCapabilities,
  },
  {
    name: 'android-15', imageId: 'sample-android-15', androidVersion: '15', diskSizeGib: 32,
    lastStartedAt: null, capabilities: { probedAt: null, items: [] },
  },
];

const sampleNotices: AppSnapshot['notices'] = [
  { at: '2026-09-26T19:52:00', level: 'info', message: '호스트 점검 결과를 저장했습니다.' },
  { at: '2026-09-26T19:58:04', level: 'info', message: '운영체제를 시작했습니다.' },
  { at: '2026-09-26T19:58:31', level: 'info', message: '화면을 연결했습니다.' },
  { at: '2026-09-26T19:59:12', level: 'info', message: '부팅을 마쳤습니다.' },
  { at: '2026-09-26T20:03:40', level: 'warning', message: '앱 관리 연결이 끊겨 다시 연결했습니다.' },
  { at: '2026-09-26T20:10:02', level: 'info', message: '샘플 앱 D 설치를 시작했습니다.' },
];

/** The rail screens after the wizard: a 64 GB, 16-thread host with two installed systems. */
const railBase: AppSnapshot = {
  ...mainSnapshot,
  images: { profiles: sampleProfiles, guests: settingsGuests, activeGuest: 'android-13' },
  guest: { ...mainSnapshot.guest, capabilities: probedCapabilities, rootEnabled: false },
  input: { ...mainSnapshot.input, profiles: sampleInputProfiles, activeId: 'user-1' },
  display: { ...mainSnapshot.display, presets: sampleDisplayPresets, activeId: 'full-hd' },
  settings: {
    ...mainSnapshot.settings,
    memoryMibMax: 61440, vcpusMax: 16,
    homeDir: 'C:\\Users\\사용자\\AppData\\Local\\OpenMobileEmulator', diskUsageBytes: Math.round(38.2 * GIB),
    screenshotsDir: 'C:\\Users\\사용자\\AppData\\Local\\OpenMobileEmulator\\screenshots',
    sharedDir: 'C:\\Users\\사용자\\AppData\\Local\\OpenMobileEmulator\\shared',
  },
  update: { currentVersion: '0.1.0', state: { kind: 'upToDate', checkedAt: '2026-09-26T09:10:00' } },
  notices: sampleNotices,
};

/** The same system while it runs: adb connected, device ID read, apps listed. */
export const railRunningSnapshot: AppSnapshot = {
  ...railBase,
  guest: {
    ...railBase.guest, state: 'running', bootCompleted: true, adbConnected: true, hosting: 'embedded',
    startedAt: '2026-09-26T19:58:04', lastExit: null, deviceId: '3f8a1c2e9b7d4051',
  },
  apps: { available: true, items: sampleApps, install: null },
  input: { ...railBase.input, foregroundPackage: 'com.example.sample.b' },
};

export const railStoppedSnapshot: AppSnapshot = railBase;

export const inputGallery: readonly GalleryVariant[] = [
  { id: 'input-running', label: 'S4 입력: 실행 중', snapshot: railRunningSnapshot },
  { id: 'input-stopped', label: 'S4 입력: 꺼짐', snapshot: railStoppedSnapshot },
  {
    id: 'input-bundled', label: 'S4 입력: 동봉 프리셋',
    snapshot: { ...railRunningSnapshot, input: { ...railRunningSnapshot.input, activeId: 'bundled-sample-a' } },
  },
  {
    id: 'input-empty', label: 'S4 입력: 프로필 없음',
    snapshot: { ...railStoppedSnapshot, input: { ...railStoppedSnapshot.input, profiles: [], activeId: null } },
  },
];

export const displayGallery: readonly GalleryVariant[] = [
  { id: 'display-basic', label: 'S5 표시: 기본', snapshot: railRunningSnapshot },
  {
    id: 'display-refresh', label: 'S5 표시: 주사율과 수직 동기화',
    snapshot: {
      ...railRunningSnapshot,
      display: { ...railRunningSnapshot.display, refreshSupported: true, refreshRateHz: 120, vsyncSupported: true, vsync: 'on' },
    },
  },
  {
    id: 'display-custom', label: 'S5 표시: 사용자 지정 해상도',
    snapshot: {
      ...railStoppedSnapshot,
      display: {
        ...railStoppedSnapshot.display, activeId: null, custom: { size: { width: 2560, height: 1440 }, densityDpi: 320 },
        refreshSupported: true, refreshRateHz: 100, refreshRates: [60, 75, 90, 100, 120, 144],
      },
    },
  },
];

/** The running system with its device ID read and the account screen reachable (M2-SCREENS.md 6). */
const googleGuest: GuestView = {
  ...railRunningSnapshot.guest, deviceIdDecimal: '4578502957678280785', addAccountSupported: true,
};

export const settingsGallery: readonly GalleryVariant[] = [
  { id: 'settings-stopped', label: 'S6 설정: 꺼짐', snapshot: railStoppedSnapshot },
  { id: 'settings-running', label: 'S6 설정: 실행 중', snapshot: railRunningSnapshot },
  {
    id: 'settings-google-reading', label: 'S6 설정: Google 계정, 기기 ID 읽는 중',
    snapshot: { ...railRunningSnapshot, guest: { ...railRunningSnapshot.guest, deviceId: null, deviceIdDecimal: null, googleAccounts: null } },
  },
  {
    id: 'settings-google-id', label: 'S6 설정: Google 계정, 등록 전',
    snapshot: { ...railRunningSnapshot, guest: { ...googleGuest, googleAccounts: 0 } },
  },
  {
    id: 'settings-google-registered', label: 'S6 설정: Google 계정, 등록 페이지를 연 뒤',
    snapshot: { ...railRunningSnapshot, guest: { ...googleGuest, googleAccounts: 0, registrationOpenedAt: '2026-09-27T20:14:00' } },
  },
  {
    id: 'settings-google-account', label: 'S6 설정: Google 계정, 로그인됨',
    snapshot: { ...railRunningSnapshot, guest: { ...googleGuest, googleAccounts: 1, registrationOpenedAt: '2026-09-27T20:14:00' } },
  },
  {
    id: 'settings-network', label: 'S6 설정: 다른 PC 연결 허용, 새 버전',
    snapshot: {
      ...railRunningSnapshot,
      settings: { ...railRunningSnapshot.settings, adbAccess: 'network' },
      update: {
        currentVersion: '0.1.0',
        state: {
          kind: 'available', version: '0.2.0',
          notesUrl: 'https://github.com/115dkk/Open-Mobile-Emulator/releases/tag/v0.2.0',
          asset: { name: 'OpenMobileEmulator-0.2.0-setup.exe', sizeBytes: 48 * MIB },
        },
      },
    },
  },
  {
    id: 'settings-update-downloading', label: 'S6 설정: 업데이트 다운로드 중',
    snapshot: {
      ...railStoppedSnapshot,
      update: { currentVersion: '0.1.0', state: { kind: 'downloading', progress: transfer('transferring', 0.37, 'ome-0.2.0-setup.exe', 96 * MIB) } },
    },
  },
  {
    id: 'settings-update-failed', label: 'S6 설정: 업데이트 실패',
    snapshot: {
      ...railStoppedSnapshot,
      update: {
        currentVersion: '0.1.0',
        state: {
          kind: 'failed',
          issue: { code: 'update_check_failed', message: '새 버전을 확인하지 못했습니다.', nextAction: '인터넷 연결을 확인한 뒤 다시 시도하십시오.' },
        },
      },
    },
  },
];

// ---- Overlay window (S4-오버레이): the page over a neutral placeholder in a 1280x800 frame. Every
// binding kind once (tap, hold, joystick, swipe, mouse button fixed and where pressed, wheel, pass
// through), markers more than 40px apart at 1280x800 and at the test DOM's 1024x768. ----

/** A gallery state of the overlay page; `seed` is the editor state it starts from. */
export interface OverlayGalleryVariant extends GalleryVariant {
  readonly seed?: OverlaySeed | undefined;
}

export const overlayProfile: InputProfileFixture = {
  id: 'overlay-1', name: '내 프로필 1', bundled: false, targetPackage: 'com.example.sample.b',
  referenceAspect: { width: 16, height: 10 }, anchor: 'center',
  bindings: [
    {
      id: 'move', trigger: { kind: 'keySet', up: 'KeyW', down: 'KeyS', left: 'KeyA', right: 'KeyD' },
      action: { kind: 'joystick', center: { x: 0.16, y: 0.68 }, radius: 0.16 },
    },
    { id: 'skill', trigger: { kind: 'key', code: 'KeyQ' }, action: { kind: 'tap', at: { x: 0.8, y: 0.45 }, hold: false } },
    { id: 'guard', trigger: { kind: 'key', code: 'KeyE' }, action: { kind: 'tap', at: { x: 0.9, y: 0.62 }, hold: true } },
    {
      id: 'dash', trigger: { kind: 'key', code: 'Space' },
      action: { kind: 'swipe', from: { x: 0.56, y: 0.8 }, to: { x: 0.72, y: 0.8 }, durationMs: 240 },
    },
    { id: 'menu', trigger: { kind: 'mouseButton', button: 'right' }, action: { kind: 'mouseTap', at: { x: 0.92, y: 0.14 } } },
    { id: 'touch', trigger: { kind: 'mouseButton', button: 'left' }, action: { kind: 'mouseTap', at: null } },
    { id: 'scroll', trigger: { kind: 'wheel', direction: 'up' }, action: { kind: 'wheelSwipe', at: { x: 0.5, y: 0.4 }, distance: 0.2 } },
    { id: 'back', trigger: { kind: 'key', code: 'Escape' }, action: { kind: 'passThrough' } },
  ],
};

const overlayInput: AppSnapshot['input'] = {
  ...runningSnapshot.input,
  profiles: [...sampleInputProfiles.filter((profile) => profile.bundled), overlayProfile],
  activeId: overlayProfile.id, overlayVisible: true, editing: false, suspended: false,
};

/** The running system with markers shown. */
export const overlaySnapshot: AppSnapshot = {
  ...runningSnapshot,
  guest: { ...runningSnapshot.guest, resolution: { width: 1280, height: 800 } },
  input: overlayInput,
};

const overlayEditing: AppSnapshot = { ...overlaySnapshot, input: { ...overlayInput, editing: true } };

export const overlayGallery: readonly OverlayGalleryVariant[] = [
  { id: 'overlay-showing', label: 'S4 오버레이: 표지', snapshot: overlaySnapshot },
  {
    id: 'overlay-suspended', label: 'S4 오버레이: 매핑 일시 중지',
    snapshot: { ...overlaySnapshot, input: { ...overlayInput, suspended: true } },
  },
  { id: 'overlay-editing', label: 'S4 오버레이: 편집, 표지 선택', snapshot: overlayEditing, seed: { selectedId: 'move' } },
  {
    id: 'overlay-editing-card', label: 'S4 오버레이: 편집, 표지 카드',
    snapshot: overlayEditing, seed: { selectedId: 'dash', cardOpen: true },
  },
  {
    id: 'overlay-editing-bundled', label: 'S4 오버레이: 편집, 동봉 프리셋',
    snapshot: { ...overlayEditing, input: { ...overlayEditing.input, activeId: 'bundled-sample-a' } },
  },
];

// ---- Title bar (P4-B): the product-drawn window top over the shell, restored and maximized. ----
// The maximized state belongs to the window, not the snapshot, so the gallery fixes it per variant.

export const titleBarGallery: readonly GalleryVariant[] = [
  { id: 'titlebar-default', label: '제목 표시줄: 기본', snapshot: mainSnapshot },
  { id: 'titlebar-maximized', label: '제목 표시줄: 최대화', snapshot: mainSnapshot },
];

export const titleBarMaximizedIds: ReadonlySet<string> = new Set(['titlebar-maximized']);
