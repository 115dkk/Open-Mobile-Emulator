// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ScreenActions } from './actions';
import type { AppIssue, AppSnapshot, ControllerBridge } from './contracts';

export function presentationIssue(error: unknown): AppIssue {
  if (typeof error === 'object' && error !== null &&
      'code' in error && typeof error.code === 'string' &&
      'message' in error && typeof error.message === 'string' &&
      'nextAction' in error && (error.nextAction === null || typeof error.nextAction === 'string')) {
    return { code: error.code, message: error.message, nextAction: error.nextAction };
  }
  return {
    code: 'app_connection_failed',
    message: '앱 상태를 확인하지 못했습니다.',
    nextAction: '앱을 종료한 뒤 다시 실행하십시오.',
  };
}

export interface ControllerState {
  readonly snapshot: AppSnapshot | null;
  readonly issue: AppIssue | null;
  readonly actions: ScreenActions;
}

/** Presentation state only. Admission and all command decisions stay in Rust. */
export function useController(bridge: ControllerBridge): ControllerState {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [issue, setIssue] = useState<AppIssue | null>(null);
  const generation = useRef(0);
  const revision = useRef(0);

  useEffect(() => {
    const current = ++generation.current;
    let disposed = false;
    let unsubscribe: (() => Promise<void>) | undefined;
    const receive = (next: AppSnapshot) => {
      if (disposed) return;
      revision.current += 1;
      setSnapshot(next);
      setIssue(next.issue);
    };
    const fail = (error: unknown) => {
      if (!disposed) setIssue(presentationIssue(error));
    };
    // Listen before the initial read so an update cannot fall between them.
    const initialize = async () => {
      try {
        const release = await bridge.watchSnapshot(receive);
        if (disposed) { await release(); return; }
        unsubscribe = release;
      } catch (error) {
        fail(error);
      }
      if (disposed) return;
      const beforeRead = revision.current;
      try {
        const next = await bridge.snapshot();
        if (revision.current === beforeRead) receive(next);
      } catch (error) {
        fail(error);
      }
    };
    void initialize();
    return () => {
      disposed = true;
      if (generation.current === current) generation.current += 1;
      if (unsubscribe) void unsubscribe().catch(() => { /* Window teardown already drops listeners. */ });
    };
  }, [bridge]);

  const execute = useCallback(async (operation: () => Promise<AppSnapshot>) => {
    const current = generation.current;
    const beforeCommand = revision.current;
    try {
      const next = await operation();
      if (generation.current === current && revision.current === beforeCommand) {
        revision.current += 1;
        setSnapshot(next);
        setIssue(next.issue);
      }
    } catch (error) {
      if (generation.current === current) setIssue(presentationIssue(error));
    }
  }, []);

  // Every bridge command, wrapped so its result lands in this state. The mapped ScreenActions
  // type makes a missing command a compile error.
  const actions = useMemo<ScreenActions>(() => ({
    hostCheckRefresh: () => execute(() => bridge.hostCheckRefresh()),
    wizardContinue: () => execute(() => bridge.wizardContinue()),
    wizardSkip: () => execute(() => bridge.wizardSkip()),
    whpxEnable: () => execute(() => bridge.whpxEnable()),
    artifactDownloadStart: () => execute(() => bridge.artifactDownloadStart()),
    artifactDownloadCancel: () => execute(() => bridge.artifactDownloadCancel()),
    guestImageSelect: (id) => execute(() => bridge.guestImageSelect(id)),
    guestCreate: (imageId, sizeGib) => execute(() => bridge.guestCreate(imageId, sizeGib)),
    guestSelect: (name) => execute(() => bridge.guestSelect(name)),
    guestDelete: (name) => execute(() => bridge.guestDelete(name)),
    guestReinstall: (name) => execute(() => bridge.guestReinstall(name)),
    guestStart: () => execute(() => bridge.guestStart()),
    guestStop: () => execute(() => bridge.guestStop()),
    guestRestart: () => execute(() => bridge.guestRestart()),
    guestRootSet: (enabled) => execute(() => bridge.guestRootSet(enabled)),
    stageRectChanged: (rect) => execute(() => bridge.stageRectChanged(rect)),
    screenshotSave: () => execute(() => bridge.screenshotSave()),
    appInstallPick: () => execute(() => bridge.appInstallPick()),
    appUninstall: (pkg) => execute(() => bridge.appUninstall(pkg)),
    appLaunch: (pkg) => execute(() => bridge.appLaunch(pkg)),
    inputProfileSelect: (id) => execute(() => bridge.inputProfileSelect(id)),
    inputSuspendToggle: () => execute(() => bridge.inputSuspendToggle()),
    inputProfileDelete: (id) => execute(() => bridge.inputProfileDelete(id)),
    inputProfileSave: (profile) => execute(() => bridge.inputProfileSave(profile)),
    inputBindingUpsert: (profileId, binding) => execute(() => bridge.inputBindingUpsert(profileId, binding)),
    inputBindingRemove: (profileId, id) => execute(() => bridge.inputBindingRemove(profileId, id)),
    inputEditorToggle: () => execute(() => bridge.inputEditorToggle()),
    inputAutoApplySet: (enabled) => execute(() => bridge.inputAutoApplySet(enabled)),
    inputSuspendHotkeySet: (code) => execute(() => bridge.inputSuspendHotkeySet(code)),
    displayPresetApply: (id) => execute(() => bridge.displayPresetApply(id)),
    displayCustomApply: (size, densityDpi) => execute(() => bridge.displayCustomApply(size, densityDpi)),
    displayRefreshSet: (hz) => execute(() => bridge.displayRefreshSet(hz)),
    displayVsyncSet: (mode) => execute(() => bridge.displayVsyncSet(mode)),
    stageFitSet: (fit) => execute(() => bridge.stageFitSet(fit)),
    settingsSave: (settings) => execute(() => bridge.settingsSave(settings)),
    updateCheck: () => execute(() => bridge.updateCheck()),
    updateInstall: () => execute(() => bridge.updateInstall()),
    diagnosticsExport: () => execute(() => bridge.diagnosticsExport()),
    openLogsFolder: () => execute(() => bridge.openLogsFolder()),
    openScreenshotsFolder: () => execute(() => bridge.openScreenshotsFolder()),
    openRegistrationPage: () => execute(() => bridge.openRegistrationPage()),
    guestWindowToFront: () => execute(() => bridge.guestWindowToFront()),
  }), [bridge, execute]);

  return { snapshot, issue, actions };
}
