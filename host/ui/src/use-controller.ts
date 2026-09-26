// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useCallback, useEffect, useRef, useState } from 'react';
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

/** Presentation state only. Admission and all command decisions stay in Rust. */
export function useController(bridge: ControllerBridge) {
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

  return {
    snapshot,
    issue,
    hostCheckRefresh: () => execute(() => bridge.hostCheckRefresh()),
    guestStart: () => execute(() => bridge.guestStart()),
  };
}
