// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { AppSnapshot, ControllerBridge } from './contracts';
import { fixtureBridge, sampleSnapshot } from './fixtures';
import { commandNames } from './test-actions';
import { useController } from './use-controller';

describe('presentation controller', () => {
  it('loads the guest state from its bridge', async () => {
    const bridge = fixtureBridge();
    const { result } = renderHook(() => useController(bridge));
    await waitFor(() => { expect(result.current.snapshot?.guest.state).toBe('stopped'); });
  });

  it('keeps the command issue and its next action', async () => {
    const issue = { code: 'start_failed', message: '가상 머신이 시작하지 못했습니다.', nextAction: '다시 시도하십시오.' };
    const bridge = fixtureBridge(sampleSnapshot, issue);
    const { result } = renderHook(() => useController(bridge));
    await waitFor(() => { expect(result.current.snapshot).not.toBeNull(); });
    await act(() => result.current.actions.guestStart());
    expect(result.current.issue).toEqual(issue);
  });

  it('stores the snapshot a command returns', async () => {
    const running: AppSnapshot = { ...sampleSnapshot, guest: { ...sampleSnapshot.guest, state: 'running' } };
    const bridge = { ...fixtureBridge(), guestStart: vi.fn(() => Promise.resolve(running)) };
    const { result } = renderHook(() => useController(bridge));
    await waitFor(() => { expect(result.current.snapshot).not.toBeNull(); });
    await act(() => result.current.actions.guestStart());
    expect(bridge.guestStart).toHaveBeenCalledOnce();
    expect(result.current.snapshot?.guest.state).toBe('running');
  });

  it('exposes every bridge command as an action', () => {
    const bridge = fixtureBridge();
    const { result } = renderHook(() => useController(bridge));
    expect(Object.keys(result.current.actions).sort()).toEqual([...commandNames].sort());
  });

  it('forwards action arguments to the bridge', async () => {
    const guestCreate = vi.fn<ControllerBridge['guestCreate']>(() => Promise.resolve(sampleSnapshot));
    const stageRectChanged = vi.fn<ControllerBridge['stageRectChanged']>(() => Promise.resolve(sampleSnapshot));
    const openHelp = vi.fn<ControllerBridge['openHelp']>(() => Promise.resolve(sampleSnapshot));
    const guestVolumeSet = vi.fn<ControllerBridge['guestVolumeSet']>(() => Promise.resolve(sampleSnapshot));
    const copyToClipboard = vi.fn<ControllerBridge['copyToClipboard']>(() => Promise.resolve(sampleSnapshot));
    const bridge: ControllerBridge = {
      ...fixtureBridge(), guestCreate, stageRectChanged, openHelp, guestVolumeSet, copyToClipboard,
    };
    const { result } = renderHook(() => useController(bridge));
    await waitFor(() => { expect(result.current.snapshot).not.toBeNull(); });
    await act(() => result.current.actions.guestCreate('sample-android-13', 64));
    await act(() => result.current.actions.stageRectChanged({ x: 1, y: 2, width: 3, height: 4, scaleFactor: 1.5 }));
    await act(() => result.current.actions.openHelp('googleAccount'));
    await act(() => result.current.actions.guestVolumeSet(8));
    await act(() => result.current.actions.copyToClipboard('adbAddress'));
    expect(guestCreate).toHaveBeenCalledWith('sample-android-13', 64);
    expect(stageRectChanged).toHaveBeenCalledWith({ x: 1, y: 2, width: 3, height: 4, scaleFactor: 1.5 });
    expect(openHelp).toHaveBeenCalledWith('googleAccount');
    expect(guestVolumeSet).toHaveBeenCalledWith(8);
    expect(copyToClipboard).toHaveBeenCalledWith('adbAddress');
  });

  it('forwards host keys without replacing the snapshot or issue', async () => {
    const issue = { code: 'existing_issue', message: '확인하십시오.', nextAction: null };
    const snapshot = { ...sampleSnapshot, issue };
    const inputHostKey = vi.fn<ControllerBridge['inputHostKey']>(() => Promise.resolve());
    const bridge = { ...fixtureBridge(snapshot), inputHostKey };
    const { result } = renderHook(() => useController(bridge));
    await waitFor(() => { expect(result.current.snapshot).toBe(snapshot); });
    await act(() => result.current.actions.inputHostKey('KeyA', true));
    await act(() => result.current.actions.inputHostKey('KeyA', false));
    expect(inputHostKey.mock.calls).toEqual([['KeyA', true], ['KeyA', false]]);
    expect(result.current.snapshot).toBe(snapshot);
    expect(result.current.issue).toBe(issue);
  });

  it('swallows host-key failures and warns only once across controller mounts', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const inputHostKey = vi.fn<ControllerBridge['inputHostKey']>(() => Promise.reject(new Error('unavailable')));
    const bridge = { ...fixtureBridge(), inputHostKey };
    const first = renderHook(() => useController(bridge));
    await waitFor(() => { expect(first.result.current.snapshot).toBe(sampleSnapshot); });
    await act(async () => {
      await expect(first.result.current.actions.inputHostKey('KeyA', true)).resolves.toBeUndefined();
      await expect(first.result.current.actions.inputHostKey('KeyA', false)).resolves.toBeUndefined();
    });
    expect(first.result.current.snapshot).toBe(sampleSnapshot);
    expect(first.result.current.issue).toBeNull();
    expect(warn).toHaveBeenCalledOnce();
    first.unmount();
    const second = renderHook(() => useController(bridge));
    await waitFor(() => { expect(second.result.current.snapshot).toBe(sampleSnapshot); });
    await act(() => second.result.current.actions.inputHostKey('KeyB', true));
    expect(second.result.current.snapshot).toBe(sampleSnapshot);
    expect(second.result.current.issue).toBeNull();
    expect(warn).toHaveBeenCalledOnce();
  });

  it('keeps the same actions object across renders', async () => {
    const bridge = fixtureBridge();
    const { result, rerender } = renderHook(() => useController(bridge));
    const first = result.current.actions;
    await waitFor(() => { expect(result.current.snapshot).not.toBeNull(); });
    rerender();
    expect(result.current.actions).toBe(first);
  });

  it('unsubscribes even if subscription finishes after unmount', async () => {
    let finish: ((release: () => Promise<void>) => void) | undefined;
    const release = vi.fn(() => Promise.resolve());
    const bridge = { ...fixtureBridge(), watchSnapshot: () => new Promise<() => Promise<void>>((resolve) => { finish = resolve; }) };
    const { unmount } = renderHook(() => useController(bridge));
    unmount();
    await act(async () => { finish?.(release); await Promise.resolve(); });
    expect(release).toHaveBeenCalledOnce();
  });

  it('does not overwrite a pushed snapshot with a stale initial read', async () => {
    let notify: ((value: AppSnapshot) => void) | undefined;
    let finish: ((value: AppSnapshot) => void) | undefined;
    const snapshot = vi.fn(() => new Promise<AppSnapshot>((resolve) => { finish = resolve; }));
    const bridge = {
      ...fixtureBridge(), snapshot,
      watchSnapshot: (callback: (value: AppSnapshot) => void) => {
        notify = callback;
        return Promise.resolve(() => Promise.resolve());
      },
    };
    const { result } = renderHook(() => useController(bridge));
    await waitFor(() => { expect(snapshot).toHaveBeenCalledOnce(); });
    const running: AppSnapshot = { ...sampleSnapshot, guest: { ...sampleSnapshot.guest, state: 'running' } };
    act(() => { notify?.(running); });
    await act(async () => { finish?.(sampleSnapshot); await Promise.resolve(); });
    expect(result.current.snapshot?.guest.state).toBe('running');
  });
});
