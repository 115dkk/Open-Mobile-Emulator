// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { AppIssue, AppSnapshot, ControllerBridge } from './contracts';

export const sampleSnapshot: AppSnapshot = {
  contractVersion: 1, productVersion: '0.1.0', phase: 'wizard',
  host: { rows: [], ready: false, inspectedAt: null },
  wizard: { step: 'hostCheck', canContinue: false, canSkip: false, download: null, gsfId: null, diskSizeGib: 32, diskFreeBytes: null },
  guest: { state: 'stopped', bootCompleted: false, adbConnected: false, hosting: 'none', resolution: null, lastExit: null, fps: null, startedAt: null },
  apps: { available: false, items: [], install: null },
  keymap: { profiles: [], activeId: null, enabled: false },
  display: { presets: [], activeId: null, fit: 'fitWindow' },
  settings: { memoryMib: 8192, vcpus: 4, gpuMode: 'virgl', closeAction: 'minimizeToTray', showFps: false, autoUpdateCheck: false, homeDir: '', diskUsageBytes: null },
  update: { currentVersion: '0.1.0', state: { kind: 'idle' } },
  notices: [], issue: null,
};

export function fixtureBridge(snapshot: AppSnapshot = sampleSnapshot, failure: AppIssue | null = null): ControllerBridge {
  const read = () => Promise.resolve(snapshot);
  const apply = () => failure ? Promise.reject(Object.assign(new Error(failure.message), failure)) : read();
  return {
    snapshot: read,
    hostCheckRefresh: apply,
    wizardContinue: apply,
    wizardSkip: apply,
    wizardRestart: apply,
    whpxEnable: apply,
    artifactDownloadStart: apply,
    artifactDownloadCancel: apply,
    guestDiskCreate: apply,
    guestStart: apply,
    guestStop: apply,
    guestRestart: apply,
    stageRectChanged: apply,
    screenshotSave: apply,
    appInstallPick: apply,
    appUninstall: apply,
    appLaunch: apply,
    keymapSetActive: apply,
    keymapSetEnabled: apply,
    keymapDelete: apply,
    displayPresetApply: apply,
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
