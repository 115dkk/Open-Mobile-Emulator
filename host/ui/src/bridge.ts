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

async function nativeVoid(command: string, args: Record<string, unknown>): Promise<void> {
  requireNative();
  return invoke<void>(command, args);
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
  wizardDefer: () => native('wizard_defer'),
  openHelp: (topic) => native('open_help', { topic }),
  appQuit: () => native('app_quit'),
  whpxEnable: () => native('whpx_enable'),
  artifactDownloadStart: () => native('artifact_download_start'),
  artifactDownloadCancel: () => native('artifact_download_cancel'),
  guestImageSelect: (id) => native('guest_image_select', { id }),
  guestCreate: (imageId, sizeGib) => native('guest_create', { imageId, sizeGib }),
  guestSelect: (id) => native('guest_select', { id }),
  guestDelete: (id) => native('guest_delete', { id }),
  guestReinstall: (name) => native('guest_reinstall', { name }),
  guestInstallCancel: () => native('guest_install_cancel'),
  guestStart: () => native('guest_start'),
  guestStop: () => native('guest_stop'),
  guestRestart: () => native('guest_restart'),
  guestRootSet: (enabled) => native('guest_root_set', { enabled }),
  guestVolumeSet: (index) => native('guest_volume_set', { index }),
  stageRectChanged: (rect) => native('stage_rect_changed', { rect }),
  stageHidden: () => native('stage_hidden'),
  screenshotSave: () => native('screenshot_save'),
  appInstallPick: () => native('app_install_pick'),
  appInstallCancel: () => native('app_install_cancel'),
  appUninstall: (pkg) => native('app_uninstall', { package: pkg }),
  appLaunch: (pkg) => native('app_launch', { package: pkg }),
  inputHostKey: (code, pressed) => nativeVoid('input_host_key', { code, pressed }),
  inputProfileSelect: (id) => native('input_profile_select', { id }),
  inputSuspendToggle: () => native('input_suspend_toggle'),
  inputOverlayToggle: () => native('input_overlay_toggle'),
  inputProfileDelete: (id) => native('input_profile_delete', { id }),
  inputProfileSave: (profile) => native('input_profile_save', { profile }),
  inputBindingUpsert: (profileId, binding) => native('input_binding_upsert', { profileId, binding }),
  inputBindingRemove: (profileId, id) => native('input_binding_remove', { profileId, id }),
  inputEditorToggle: () => native('input_editor_toggle'),
  inputAutoApplySet: (enabled) => native('input_auto_apply_set', { enabled }),
  inputSuspendHotkeySet: (code) => native('input_suspend_hotkey_set', { code }),
  textCompose: (text) => nativeVoid('text_compose', { text }),
  textCommit: (text) => nativeVoid('text_commit', { text }),
  textKey: (key) => nativeVoid('text_key', { key }),
  displayPresetApply: (id) => native('display_preset_apply', { id }),
  displayCustomApply: (size, densityDpi) => native('display_custom_apply', { size, densityDpi }),
  displayRefreshSet: (hz) => native('display_refresh_set', { hz }),
  displayVsyncSet: (mode) => native('display_vsync_set', { mode }),
  stageFitSet: (fit) => native('stage_fit_set', { fit }),
  settingsSave: (settings) => native('settings_save', { settings }),
  updateCheck: () => native('update_check'),
  updateInstall: () => native('update_install'),
  diagnosticsExport: () => native('diagnostics_export'),
  openLogsFolder: () => native('open_logs_folder'),
  openScreenshotsFolder: () => native('open_screenshots_folder'),
  openHomeFolder: () => native('open_home_folder'),
  openInstallFolder: () => native('open_install_folder'),
  openSharedFolder: () => native('open_shared_folder'),
  sharedPush: () => native('shared_push'),
  copyToClipboard: (item) => native('copy_to_clipboard', { item }),
  guestWindowToFront: () => native('guest_window_to_front'),
  watchSnapshot: (notify) => watch<AppSnapshot>(EVENT_SNAPSHOT, notify),
  watchProgress: (notify) => watch<TransferProgress>(EVENT_PROGRESS, notify),
};
