// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import type {
  AppIssue, AppSnapshot, AppsView, Binding, BindingAction, CapabilityItem, CapabilityReport,
  CustomDisplay, DisplayPreset, DisplayView, GuestImageSummary, GuestSummary, GuestView,
  HostReport, HostRow, ImagesView, InputProfile, InputView, InstallProgress, LastExit, LogicalPoint, Notice,
  SettingsInput, SettingsView, Size, StageRect, TransferProgress, Trigger, UpdateState,
  UpdateView, WizardView,
} from './contracts';

type Check = (value: unknown, path: string) => void;
type Shape<T> = { [Key in keyof T]-?: Check };
const fail = (path: string, expected: string): never => { throw new Error(`${path}: expected ${expected}`); };
const string: Check = (value, path) => { if (typeof value !== 'string') fail(path, 'string'); };
const number: Check = (value, path) => { if (typeof value !== 'number' || !Number.isFinite(value)) fail(path, 'finite number'); };
const boolean: Check = (value, path) => { if (typeof value !== 'boolean') fail(path, 'boolean'); };
const nullable = (check: Check): Check => (value, path) => { if (value !== null) check(value, path); };
const enumeration = (...values: readonly string[]): Check => (value, path) => {
  if (typeof value !== 'string' || !values.includes(value)) fail(path, values.join(' | '));
};
function object<T>(shape: Shape<T>): Check {
  return (value, path) => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return fail(path, 'object');
    for (const [key, check] of Object.entries<Check>(shape)) {
      if (!Object.hasOwn(value, key)) fail(`${path}.${key}`, 'required key');
      check(Reflect.get(value, key), `${path}.${key}`);
    }
  };
}
const array = (check: Check): Check => (value, path) => {
  if (!Array.isArray(value)) return fail(path, 'array');
  value.forEach((item: unknown, index: number) => { check(item, `${path}[${index}]`); });
};

const issue = object<AppIssue>({ code: string, message: string, nextAction: nullable(string) });
const size = object<Size>({ width: number, height: number });
const point = object<LogicalPoint>({ x: number, y: number });
const rect = object<StageRect>({ x: number, y: number, width: number, height: number, scaleFactor: number });
const transfer = object<TransferProgress>({
  stage: enumeration('waiting', 'transferring', 'verifying', 'verified', 'failed', 'cancelled'),
  doneBytes: number, totalBytes: nullable(number), bytesPerSecond: nullable(number), label: string,
});
const install = object<InstallProgress>({
  label: string, stage: enumeration('waiting', 'transferring', 'verifying', 'verified', 'failed', 'cancelled'),
  doneItems: number, totalItems: number, ratio: nullable(number), doneBytes: number, totalBytes: nullable(number),
});
const capability = enumeration('available', 'unavailable', 'unknown');
const capabilityItem = object<CapabilityItem>({
  id: enumeration('bootMarker', 'appList', 'displaySize', 'mediaVolume', 'deviceId', 'screenshot', 'foregroundApp', 'multitouch', 'nativeBridge', 'root'),
  state: capability,
});
const capabilityReport = object<CapabilityReport>({ probedAt: nullable(string), items: array(capabilityItem) });
const trigger: Check = (value, path) => {
  object<{ kind: string }>({ kind: string })(value, path);
  const kind: unknown = Reflect.get(value as object, 'kind');
  const variants = {
    key: object<Extract<Trigger, { kind: 'key' }>>({ kind: enumeration('key'), code: string }),
    mouseButton: object<Extract<Trigger, { kind: 'mouseButton' }>>({ kind: enumeration('mouseButton'), button: enumeration('left', 'right', 'middle') }),
    wheel: object<Extract<Trigger, { kind: 'wheel' }>>({ kind: enumeration('wheel'), direction: enumeration('up', 'down') }),
    keySet: object<Extract<Trigger, { kind: 'keySet' }>>({ kind: enumeration('keySet'), up: string, down: string, left: string, right: string }),
  } satisfies Record<Trigger['kind'], Check>;
  if (typeof kind !== 'string' || !Object.hasOwn(variants, kind)) fail(`${path}.kind`, 'trigger kind');
  variants[kind as keyof typeof variants](value, path);
};
const action: Check = (value, path) => {
  object<{ kind: string }>({ kind: string })(value, path);
  const kind: unknown = Reflect.get(value as object, 'kind');
  const variants = {
    tap: object<Extract<BindingAction, { kind: 'tap' }>>({ kind: enumeration('tap'), at: point, hold: boolean }),
    swipe: object<Extract<BindingAction, { kind: 'swipe' }>>({ kind: enumeration('swipe'), from: point, to: point, durationMs: number }),
    joystick: object<Extract<BindingAction, { kind: 'joystick' }>>({ kind: enumeration('joystick'), center: point, radius: number }),
    mouseTap: object<Extract<BindingAction, { kind: 'mouseTap' }>>({ kind: enumeration('mouseTap'), at: nullable(point) }),
    wheelSwipe: object<Extract<BindingAction, { kind: 'wheelSwipe' }>>({ kind: enumeration('wheelSwipe'), at: point, distance: number }),
    passThrough: object<Extract<BindingAction, { kind: 'passThrough' }>>({ kind: enumeration('passThrough') }),
  } satisfies Record<BindingAction['kind'], Check>;
  if (typeof kind !== 'string' || !Object.hasOwn(variants, kind)) fail(`${path}.kind`, 'action kind');
  variants[kind as keyof typeof variants](value, path);
};
const binding = object<Binding>({ id: string, trigger, action });
const profile = object<InputProfile>({
  id: string, name: string, bundled: boolean, targetPackage: nullable(string), referenceAspect: size,
  anchor: enumeration('center', 'edges'), bindings: array(binding),
});
const settingsInput: Shape<SettingsInput> = {
  memoryMib: number, vcpus: number, gpuMode: enumeration('virgl', 'software'),
  closeAction: enumeration('stopGuest', 'minimizeToTray'), showFps: boolean, autoUpdateCheck: boolean,
  adbAccess: enumeration('localhost', 'network'), bindingOverlayDefault: boolean,
};
const update: Check = (value, path) => {
  object<{ kind: string }>({ kind: string })(value, path);
  const kind: unknown = Reflect.get(value as object, 'kind');
  const variants = {
    idle: object<Extract<UpdateState, { kind: 'idle' }>>({ kind: enumeration('idle') }),
    checking: object<Extract<UpdateState, { kind: 'checking' }>>({ kind: enumeration('checking') }),
    upToDate: object<Extract<UpdateState, { kind: 'upToDate' }>>({ kind: enumeration('upToDate'), checkedAt: string }),
    available: object<Extract<UpdateState, { kind: 'available' }>>({
      kind: enumeration('available'), version: string, notesUrl: nullable(string),
      asset: object({ name: string, sizeBytes: number }),
    }),
    downloading: object<Extract<UpdateState, { kind: 'downloading' }>>({ kind: enumeration('downloading'), progress: transfer }),
    readyToInstall: object<Extract<UpdateState, { kind: 'readyToInstall' }>>({ kind: enumeration('readyToInstall'), version: string }),
    failed: object<Extract<UpdateState, { kind: 'failed' }>>({ kind: enumeration('failed'), issue }),
  } satisfies Record<UpdateState['kind'], Check>;
  if (typeof kind !== 'string' || !Object.hasOwn(variants, kind)) fail(`${path}.kind`, 'update kind');
  variants[kind as keyof typeof variants](value, path);
};
const imageSummary = object<GuestImageSummary>({
  id: string, displayName: string, androidVersion: string, apiLevel: number,
  distribution: enumeration('bliss', 'androidX86', 'selfBuilt'),
  translator: enumeration('houdini', 'ndkTranslation', 'digitalis', 'none'),
  sizeBytes: nullable(number), status: enumeration('verified', 'candidate', 'deprecated'),
  releasedAt: nullable(string), verifiedGames: number, recommended: boolean,
});
const guestSummary = object<GuestSummary>({
  name: string, imageId: string, androidVersion: string, diskSizeGib: number,
  lastStartedAt: nullable(string), capabilities: capabilityReport,
});
const snapshot = object<AppSnapshot>({
  contractVersion: (value, path) => { if (value !== 6) fail(path, 'contract version 6'); },
  productVersion: string, phase: enumeration('wizard', 'main'),
  blocker: nullable(object({ kind: enumeration('virtualizationOff', 'qemuMissing', 'hypervisorPlatformOff') })),
  host: object<HostReport>({
    ready: boolean, inspectedAt: nullable(string),
    rows: array(object<HostRow>({
      id: enumeration('cpuVirtualization', 'hypervisorPlatform', 'rebootPending', 'whpxAvailable', 'qemuPresent', 'firmwarePresent', 'adbPresent', 'diskSpace'),
      status: enumeration('ready', 'attention', 'blocked'), detail: string,
    })),
  }),
  wizard: object<WizardView>({
    step: enumeration('hostCheck', 'whpxConsent', 'rebootPending', 'artifactDownload', 'guestInstall', 'firstBoot', 'appInstall', 'done'),
    canContinue: boolean, canSkip: boolean, download: nullable(transfer), imageId: nullable(string),
    installGuide: array(string), diskSizeGib: number, diskFreeBytes: nullable(number),
  }),
  images: object<ImagesView>({ profiles: array(imageSummary), guests: array(guestSummary), activeGuest: nullable(string) }),
  guest: object<GuestView>({
    state: enumeration('stopped', 'starting', 'running', 'stopping', 'restarting', 'failed'),
    bootCompleted: boolean, adbConnected: boolean, hosting: enumeration('none', 'embedded', 'separateWindow'),
    resolution: nullable(size), fps: nullable(number), startedAt: nullable(string),
    lastExit: nullable(object<LastExit>({ kind: enumeration('userStop', 'guestReset', 'bootTimeout', 'crash', 'startFailed'), at: string, logPath: nullable(string) })),
    imageId: nullable(string), androidVersion: nullable(string), apiLevel: nullable(number),
    capabilities: capabilityReport, deviceId: nullable(string), adbAddress: nullable(string), rootEnabled: nullable(boolean),
    mediaVolume: nullable(number), deviceIdDecimal: nullable(string), googleAccounts: nullable(number),
    registrationOpenedAt: nullable(string), addAccountSupported: boolean, pid: nullable(number),
  }),
  apps: object<AppsView>({
    available: boolean, install: nullable(install),
    items: array(object<AppsView['items'][number]>({
      package: string, label: string, versionName: nullable(string), versionCode: nullable(number), installedAt: nullable(string),
    })),
  }),
  input: object<InputView>({
    profiles: array(profile), activeId: nullable(string), suspended: boolean, editing: boolean,
    autoApply: boolean, foregroundPackage: nullable(string), multitouch: capability, suspendHotkey: string,
    overlayVisible: boolean,
  }),
  display: object<DisplayView>({
    activeId: nullable(string), custom: nullable(object<CustomDisplay>({ size, densityDpi: number })),
    fit: enumeration('fitWindow', 'oneToOne'), refreshRateHz: nullable(number), refreshRates: array(number),
    refreshSupported: boolean, vsync: enumeration('off', 'on', 'adaptive'), vsyncSupported: boolean,
    presets: array(object<DisplayPreset>({ id: string, size, densityDpi: number, orientation: enumeration('landscape', 'portrait'), needsReboot: boolean })),
  }),
  settings: object<SettingsView>({
    ...settingsInput, memoryMibMin: number, memoryMibMax: number, vcpusMax: number,
    homeDir: string, diskUsageBytes: nullable(number),
  }),
  update: object<UpdateView>({ currentVersion: string, state: update }),
  notices: array(object<Notice>({ at: string, level: enumeration('info', 'warning', 'error'), message: string })),
  issue: nullable(issue),
});

function fixture(name: string): unknown {
  const path = resolve(import.meta.dirname, '../../../tests/fixtures/contract', name);
  let text: string;
  try { text = readFileSync(path, 'utf8'); } catch (error) {
    throw new Error(`Rust contract fixture is missing: ${path}. Run the ome-runtime contract fixture test first.`, { cause: error });
  }
  return JSON.parse(text) as unknown;
}

const commandShapes: Record<string, Record<string, Check>> = {
  hostCheckRefresh: {}, wizardContinue: {}, wizardSkip: {}, wizardDefer: {},
  openHelp: { topic: enumeration('virtualizationBios', 'hypervisorPlatform', 'googleAccount', 'adbSecurity', 'qemuSource', 'thirdPartyNotices', 'releaseNotes') },
  appQuit: {}, whpxEnable: {}, artifactDownloadStart: {}, artifactDownloadCancel: {}, guestImageSelect: { id: string },
  guestCreate: { imageId: string, sizeGib: number }, guestSelect: { id: string },
  guestDelete: { id: string }, guestReinstall: { name: string }, guestStart: {}, guestStop: {},
  guestRestart: {}, guestRootSet: { enabled: boolean }, guestVolumeSet: { index: number },
  stageRectChanged: { rect }, stageHidden: {}, screenshotSave: {},
  appInstallPick: {}, appInstallCancel: {}, appUninstall: { package: string }, appLaunch: { package: string },
  inputProfileSelect: { id: nullable(string) }, inputSuspendToggle: {}, inputOverlayToggle: {}, inputProfileDelete: { id: string },
  inputProfileSave: { profile }, inputBindingUpsert: { profileId: string, binding },
  inputBindingRemove: { profileId: string, id: string }, inputEditorToggle: {},
  inputAutoApplySet: { enabled: boolean }, inputSuspendHotkeySet: { code: string },
  displayPresetApply: { id: string }, displayCustomApply: { size, densityDpi: number },
  displayRefreshSet: { hz: nullable(number) }, displayVsyncSet: { mode: enumeration('off', 'on', 'adaptive') },
  stageFitSet: { fit: enumeration('fitWindow', 'oneToOne') },
  settingsSave: { settings: object<SettingsInput>(settingsInput) }, updateCheck: {}, updateInstall: {},
  diagnosticsExport: {}, openLogsFolder: {}, openScreenshotsFolder: {}, openHomeFolder: {},
  copyToClipboard: { item: enumeration('deviceId', 'adbAddress') }, openRegistrationPage: {}, googleAccountAddOpen: {}, guestWindowToFront: {},
};

describe('Rust/TypeScript contract fixtures', () => {
  it('requires every snapshot field and validates nested variants', () => {
    snapshot(fixture('snapshot.sample.json'), 'snapshot');
  });

  it('validates every command and its typed arguments', () => {
    const kinds = new Set<string>();
    array((value, path) => {
      object<{ kind: string }>({ kind: enumeration(...Object.keys(commandShapes)) })(value, path);
      const kind = Reflect.get(value as object, 'kind') as string;
      const shape = commandShapes[kind];
      if (!shape) fail(path, 'known command');
      object({ kind: enumeration(kind), ...shape })(value, path);
      kinds.add(kind);
    })(fixture('commands.sample.json'), 'commands');
    expect([...kinds].sort()).toEqual(Object.keys(commandShapes).sort());
  });

  it('rejects missing nullable fields and unknown enums', () => {
    expect(() => issue({ code: 'example', message: 'example' }, 'issue')).toThrow('nextAction');
    expect(() => update({ kind: 'unknown' }, 'update')).toThrow('update.kind');
  });
});
