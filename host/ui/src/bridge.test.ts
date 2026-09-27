// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Event } from '@tauri-apps/api/event';
import { controllerBridge } from './bridge';
import { sampleSnapshot } from './fixtures';
import type { AppSnapshot, Binding, InputProfile, TransferProgress } from './contracts';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockResolvedValue(sampleSnapshot);
});

const binding: Binding = {
  id: 'tap', trigger: { kind: 'key', code: 'Space' },
  action: { kind: 'tap', at: { x: 0.5, y: 0.5 }, hold: false },
};
const profile: InputProfile = {
  id: 'sample', name: 'Sample', bundled: false, targetPackage: null,
  referenceAspect: { width: 16, height: 9 }, anchor: 'center', bindings: [binding],
};

describe('native bridge', () => {
  it('maps every typed command to the exact registered IPC name', async () => {
    const rect = { x: 0, y: 0, width: 640, height: 480, scaleFactor: 1.5 };
    const settings = sampleSnapshot.settings;
    const cases: readonly [() => Promise<AppSnapshot>, string, Record<string, unknown> | undefined][] = [
      [() => controllerBridge.snapshot(), 'app_snapshot', undefined],
      [() => controllerBridge.hostCheckRefresh(), 'host_check_refresh', undefined],
      [() => controllerBridge.wizardContinue(), 'wizard_continue', undefined],
      [() => controllerBridge.wizardSkip(), 'wizard_skip', undefined],
      [() => controllerBridge.whpxEnable(), 'whpx_enable', undefined],
      [() => controllerBridge.artifactDownloadStart(), 'artifact_download_start', undefined],
      [() => controllerBridge.artifactDownloadCancel(), 'artifact_download_cancel', undefined],
      [() => controllerBridge.guestImageSelect('image'), 'guest_image_select', { id: 'image' }],
      [() => controllerBridge.guestCreate('image', 32), 'guest_create', { imageId: 'image', sizeGib: 32 }],
      [() => controllerBridge.guestSelect('default'), 'guest_select', { name: 'default' }],
      [() => controllerBridge.guestDelete('old'), 'guest_delete', { name: 'old' }],
      [() => controllerBridge.guestReinstall('default'), 'guest_reinstall', { name: 'default' }],
      [() => controllerBridge.guestStart(), 'guest_start', undefined],
      [() => controllerBridge.guestStop(), 'guest_stop', undefined],
      [() => controllerBridge.guestRestart(), 'guest_restart', undefined],
      [() => controllerBridge.guestRootSet(true), 'guest_root_set', { enabled: true }],
      [() => controllerBridge.stageRectChanged(rect), 'stage_rect_changed', { rect }],
      [() => controllerBridge.screenshotSave(), 'screenshot_save', undefined],
      [() => controllerBridge.appInstallPick(), 'app_install_pick', undefined],
      [() => controllerBridge.appUninstall('dev.ome.sample'), 'app_uninstall', { package: 'dev.ome.sample' }],
      [() => controllerBridge.appLaunch('dev.ome.sample'), 'app_launch', { package: 'dev.ome.sample' }],
      [() => controllerBridge.inputProfileSelect(null), 'input_profile_select', { id: null }],
      [() => controllerBridge.inputSuspendToggle(), 'input_suspend_toggle', undefined],
      [() => controllerBridge.inputProfileDelete('sample'), 'input_profile_delete', { id: 'sample' }],
      [() => controllerBridge.inputProfileSave(profile), 'input_profile_save', { profile }],
      [() => controllerBridge.inputBindingUpsert('sample', binding), 'input_binding_upsert', { profileId: 'sample', binding }],
      [() => controllerBridge.inputBindingRemove('sample', 'tap'), 'input_binding_remove', { profileId: 'sample', id: 'tap' }],
      [() => controllerBridge.inputEditorToggle(), 'input_editor_toggle', undefined],
      [() => controllerBridge.inputAutoApplySet(true), 'input_auto_apply_set', { enabled: true }],
      [() => controllerBridge.inputSuspendHotkeySet('F12'), 'input_suspend_hotkey_set', { code: 'F12' }],
      [() => controllerBridge.displayPresetApply('sample'), 'display_preset_apply', { id: 'sample' }],
      [() => controllerBridge.displayCustomApply({ width: 1600, height: 904 }, 240), 'display_custom_apply', { size: { width: 1600, height: 904 }, densityDpi: 240 }],
      [() => controllerBridge.displayRefreshSet(120), 'display_refresh_set', { hz: 120 }],
      [() => controllerBridge.displayVsyncSet('adaptive'), 'display_vsync_set', { mode: 'adaptive' }],
      [() => controllerBridge.stageFitSet('fitWindow'), 'stage_fit_set', { fit: 'fitWindow' }],
      [() => controllerBridge.settingsSave(settings), 'settings_save', { settings }],
      [() => controllerBridge.updateCheck(), 'update_check', undefined],
      [() => controllerBridge.updateInstall(), 'update_install', undefined],
      [() => controllerBridge.diagnosticsExport(), 'diagnostics_export', undefined],
      [() => controllerBridge.openLogsFolder(), 'open_logs_folder', undefined],
      [() => controllerBridge.openScreenshotsFolder(), 'open_screenshots_folder', undefined],
      [() => controllerBridge.openRegistrationPage(), 'open_registration_page', undefined],
      [() => controllerBridge.guestWindowToFront(), 'guest_window_to_front', undefined],
    ];
    for (const [operation, command, args] of cases) {
      await expect(operation()).resolves.toEqual(sampleSnapshot);
      expect(invoke).toHaveBeenLastCalledWith(command, args);
    }
  });

  it('rejects native calls and subscriptions outside Tauri', async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(controllerBridge.snapshot()).rejects.toMatchObject({ code: 'native_app_required', message: '설치한 앱에서 실행하십시오.' });
    await expect(controllerBridge.watchSnapshot(vi.fn())).rejects.toMatchObject({ code: 'native_app_required' });
    await expect(controllerBridge.watchProgress(vi.fn())).rejects.toMatchObject({ code: 'native_app_required' });
    expect(invoke).not.toHaveBeenCalled();
    expect(listen).not.toHaveBeenCalled();
  });

  it('delivers snapshot and progress payloads and returns async cleanup', async () => {
    const release = vi.fn();
    vi.mocked(listen).mockResolvedValue(release);
    const notifySnapshot = vi.fn();
    const notifyProgress = vi.fn();
    const unwatchSnapshot = await controllerBridge.watchSnapshot(notifySnapshot);
    const unwatchProgress = await controllerBridge.watchProgress(notifyProgress);
    const snapshotHandler = vi.mocked(listen).mock.calls[0]?.[1];
    const progressHandler = vi.mocked(listen).mock.calls[1]?.[1];
    const progress: TransferProgress = { stage: 'transferring', doneBytes: 10, totalBytes: 20, bytesPerSecond: 5, label: '내려받는 중입니다.' };
    const snapshotEvent: Event<AppSnapshot> = { event: 'snapshot', id: 1, payload: sampleSnapshot };
    snapshotHandler?.(snapshotEvent);
    progressHandler?.({ event: 'progress', id: 2, payload: progress });
    expect(notifySnapshot).toHaveBeenCalledWith(sampleSnapshot);
    expect(notifyProgress).toHaveBeenCalledWith(progress);
    await unwatchSnapshot();
    await unwatchProgress();
    expect(release).toHaveBeenCalledTimes(2);
  });
});
