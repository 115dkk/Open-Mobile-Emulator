// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { EVENT_PROGRESS, EVENT_SNAPSHOT } from './contracts';
import type { AppIssue, AppSnapshot, ControllerBridge, TransferProgress } from './contracts';

function requireNative(): void {
  if (!isTauri()) {
    const issue: AppIssue = {
      code: 'native_app_required',
      message: '설치한 앱에서 실행하십시오.',
      nextAction: null,
    };
    throw Object.assign(new Error(issue.message), issue);
  }
}

async function native(command: string, args?: Record<string, unknown>): Promise<AppSnapshot> {
  requireNative();
  return invoke<AppSnapshot>(command, args);
}

async function watch<T>(event: string, notify: (payload: T) => void): Promise<() => Promise<void>> {
  requireNative();
  const unlisten = await listen<T>(event, ({ payload }) => { notify(payload); });
  return () => { unlisten(); return Promise.resolve(); };
}

export const controllerBridge: ControllerBridge = {
  snapshot: () => native('app_snapshot'),
  hostCheckRefresh: () => native('host_check_refresh'),
  wizardContinue: () => native('wizard_continue'),
  wizardSkip: () => native('wizard_skip'),
  wizardRestart: () => native('wizard_restart'),
  whpxEnable: () => native('whpx_enable'),
  artifactDownloadStart: () => native('artifact_download_start'),
  artifactDownloadCancel: () => native('artifact_download_cancel'),
  guestDiskCreate: (sizeGib) => native('guest_disk_create', { sizeGib }),
  guestStart: () => native('guest_start'),
  guestStop: () => native('guest_stop'),
  guestRestart: () => native('guest_restart'),
  stageRectChanged: (rect) => native('stage_rect_changed', { rect }),
  screenshotSave: () => native('screenshot_save'),
  appInstallPick: () => native('app_install_pick'),
  appUninstall: (pkg) => native('app_uninstall', { package: pkg }),
  appLaunch: (pkg) => native('app_launch', { package: pkg }),
  keymapSetActive: (id) => native('keymap_set_active', { id }),
  keymapSetEnabled: (enabled) => native('keymap_set_enabled', { enabled }),
  keymapDelete: (id) => native('keymap_delete', { id }),
  displayPresetApply: (id) => native('display_preset_apply', { id }),
  stageFitSet: (fit) => native('stage_fit_set', { fit }),
  settingsSave: (settings) => native('settings_save', { settings }),
  updateCheck: () => native('update_check'),
  updateInstall: () => native('update_install'),
  diagnosticsExport: () => native('diagnostics_export'),
  openLogsFolder: () => native('open_logs_folder'),
  openScreenshotsFolder: () => native('open_screenshots_folder'),
  openRegistrationPage: () => native('open_registration_page'),
  guestWindowToFront: () => native('guest_window_to_front'),
  watchSnapshot: (notify) => watch<AppSnapshot>(EVENT_SNAPSHOT, notify),
  watchProgress: (notify) => watch<TransferProgress>(EVENT_PROGRESS, notify),
};
