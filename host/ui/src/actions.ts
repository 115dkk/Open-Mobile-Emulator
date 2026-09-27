// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Screen-facing commands. `useController` wraps every bridge command with its `execute`, so a command
// resolves to nothing and its result (new snapshot or AppIssue) lands in the controller state.
import type { AppSnapshot, ControllerBridge } from './contracts';

type CommandKey = Exclude<keyof ControllerBridge, 'snapshot' | 'watchSnapshot' | 'watchProgress'>;

/** Every `ControllerBridge` command, resolved into the controller's snapshot and issue state. */
export type ScreenActions = {
  readonly [K in CommandKey]: (...args: Parameters<ControllerBridge[K]>) => Promise<void>;
};

/** Props every screen receives. Screens draw the snapshot and call actions; they decide nothing. */
export interface ScreenProps {
  readonly snapshot: AppSnapshot;
  readonly actions: ScreenActions;
  /** Moves the rail to another screen. Only the stage is a destination (S4 `무대에서 편집`). */
  readonly navigate?: ((screen: 'stage') => void) | undefined;
}
