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
    const bridge: ControllerBridge = { ...fixtureBridge(), guestCreate, stageRectChanged };
    const { result } = renderHook(() => useController(bridge));
    await waitFor(() => { expect(result.current.snapshot).not.toBeNull(); });
    await act(() => result.current.actions.guestCreate('sample-android-13', 64));
    await act(() => result.current.actions.stageRectChanged({ x: 1, y: 2, width: 3, height: 4, scaleFactor: 1.5 }));
    expect(guestCreate).toHaveBeenCalledWith('sample-android-13', 64);
    expect(stageRectChanged).toHaveBeenCalledWith({ x: 1, y: 2, width: 3, height: 4, scaleFactor: 1.5 });
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
