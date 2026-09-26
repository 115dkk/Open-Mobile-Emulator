// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import type {
  AppIssue, AppSnapshot, AppsView, DisplayPreset, DisplayView, GuestView, HostReport,
  HostRow, KeymapView, LastExit, Notice, SettingsInput, SettingsView, Size, StageRect,
  TransferProgress, UpdateState, UpdateView, WizardView,
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
const rect = object<StageRect>({ x: number, y: number, width: number, height: number, scaleFactor: number });
const transfer = object<TransferProgress>({
  stage: enumeration('waiting', 'transferring', 'verifying', 'verified', 'failed', 'cancelled'),
  doneBytes: number, totalBytes: nullable(number), bytesPerSecond: nullable(number), label: string,
});
const settingsInput: Shape<SettingsInput> = {
  memoryMib: number, vcpus: number, gpuMode: enumeration('virgl', 'software'),
  closeAction: enumeration('minimizeToTray', 'stopGuest'), showFps: boolean, autoUpdateCheck: boolean,
};
const update: Check = (value, path) => {
  object<{ kind: string }>({ kind: string })(value, path);
  const kind: unknown = Reflect.get(value as object, 'kind');
  const variants = {
    idle: object<Extract<UpdateState, { kind: 'idle' }>>({ kind: enumeration('idle') }),
    checking: object<Extract<UpdateState, { kind: 'checking' }>>({ kind: enumeration('checking') }),
    upToDate: object<Extract<UpdateState, { kind: 'upToDate' }>>({ kind: enumeration('upToDate'), checkedAt: string }),
    available: object<Extract<UpdateState, { kind: 'available' }>>({ kind: enumeration('available'), version: string, notesUrl: nullable(string) }),
    downloading: object<Extract<UpdateState, { kind: 'downloading' }>>({ kind: enumeration('downloading'), progress: transfer }),
    readyToInstall: object<Extract<UpdateState, { kind: 'readyToInstall' }>>({ kind: enumeration('readyToInstall'), version: string }),
    failed: object<Extract<UpdateState, { kind: 'failed' }>>({ kind: enumeration('failed'), issue }),
  } satisfies Record<UpdateState['kind'], Check>;
  if (typeof kind !== 'string' || !Object.hasOwn(variants, kind)) fail(`${path}.kind`, 'update kind');
  variants[kind as keyof typeof variants](value, path);
};
const snapshot = object<AppSnapshot>({
  contractVersion: (value, path) => { if (value !== 1) fail(path, 'contract version 1'); },
  productVersion: string, phase: enumeration('wizard', 'main'),
  host: object<HostReport>({
    ready: boolean, inspectedAt: nullable(string),
    rows: array(object<HostRow>({
      id: enumeration('cpuVirtualization', 'hypervisorPlatform', 'rebootPending', 'whpxAvailable', 'qemuPresent', 'firmwarePresent', 'adbPresent', 'diskSpace'),
      status: enumeration('ready', 'attention', 'blocked'), detail: string,
    })),
  }),
  wizard: object<WizardView>({
    step: enumeration('hostCheck', 'whpxConsent', 'rebootPending', 'artifactDownload', 'guestInstall', 'firstBoot', 'googleRegistration', 'appInstall', 'done'),
    canContinue: boolean, canSkip: boolean, download: nullable(transfer), gsfId: nullable(string), diskSizeGib: number, diskFreeBytes: nullable(number),
  }),
  guest: object<GuestView>({
    state: enumeration('stopped', 'starting', 'running', 'stopping', 'restarting', 'failed'),
    bootCompleted: boolean, adbConnected: boolean, hosting: enumeration('none', 'embedded', 'separateWindow'),
    resolution: nullable(size), fps: nullable(number), startedAt: nullable(string),
    lastExit: nullable(object<LastExit>({ kind: enumeration('userStop', 'guestReset', 'unexpected'), at: string, logPath: nullable(string) })),
  }),
  apps: object<AppsView>({
    available: boolean, install: nullable(transfer),
    items: array(object<AppsView['items'][number]>({ package: string, label: string, versionName: nullable(string), installedAt: nullable(string) })),
  }),
  keymap: object<KeymapView>({
    activeId: nullable(string), enabled: boolean,
    profiles: array(object<KeymapView['profiles'][number]>({ id: string, name: string, bundled: boolean, bindingCount: number })),
  }),
  display: object<DisplayView>({
    activeId: nullable(string), fit: enumeration('fitWindow', 'oneToOne'),
    presets: array(object<DisplayPreset>({ id: string, size, densityDpi: number, orientation: enumeration('landscape', 'portrait'), needsReboot: boolean })),
  }),
  settings: object<SettingsView>({ ...settingsInput, homeDir: string, diskUsageBytes: nullable(number) }),
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
  hostCheckRefresh: {}, wizardContinue: {}, wizardSkip: {}, wizardRestart: {}, whpxEnable: {},
  artifactDownloadStart: {}, artifactDownloadCancel: {}, guestDiskCreate: { sizeGib: number },
  guestStart: {}, guestStop: {}, guestRestart: {}, stageRectChanged: { rect }, screenshotSave: {},
  appInstallPick: {}, appUninstall: { package: string }, appLaunch: { package: string },
  keymapSetActive: { id: nullable(string) }, keymapSetEnabled: { enabled: boolean }, keymapDelete: { id: string },
  displayPresetApply: { id: string }, stageFitSet: { fit: enumeration('fitWindow', 'oneToOne') },
  settingsSave: { settings: object<SettingsInput>(settingsInput) }, updateCheck: {}, updateInstall: {},
  diagnosticsExport: {}, openLogsFolder: {}, openScreenshotsFolder: {}, openRegistrationPage: {}, guestWindowToFront: {},
};

describe('Rust/TypeScript contract fixtures', () => {
  it('requires every snapshot field and validates nested enums', () => {
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
