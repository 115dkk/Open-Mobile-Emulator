// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Screen-facing commands resolve to nothing. Snapshot commands update controller state; host keys
// are forwarded without changing the snapshot or showing an issue.
import type { AppSnapshot, ControllerBridge } from './contracts';

type CommandKey = Exclude<keyof ControllerBridge, 'snapshot' | 'watchSnapshot' | 'watchProgress' | 'inputHostKey'>;

/** Snapshot commands update controller state; host-key failures are only logged once per session. */
export type ScreenActions = {
  readonly [K in CommandKey]: (...args: Parameters<ControllerBridge[K]>) => Promise<void>;
} & {
  readonly inputHostKey: (code: string, pressed: boolean) => Promise<void>;
};

/** Props every screen receives. Screens draw the snapshot and call actions; they decide nothing. */
export interface ScreenProps {
  readonly snapshot: AppSnapshot;
  readonly actions: ScreenActions;
  /** Moves the rail to another screen. Only the stage is a destination (S4 `무대에서 편집`). */
  readonly navigate?: ((screen: 'stage') => void) | undefined;
}
