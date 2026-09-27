// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { AppIssue, AppSnapshot, ControllerBridge } from './contracts';

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
