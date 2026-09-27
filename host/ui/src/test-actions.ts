// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Test helper: a ScreenActions object whose every command is a resolved spy.
import { vi } from 'vitest';
import type { Mock } from 'vitest';
import type { ScreenActions } from './actions';
import { fixtureBridge } from './fixtures';

export type MockActions = { readonly [K in keyof ScreenActions]: Mock<ScreenActions[K]> };

const NOT_COMMANDS = new Set(['snapshot', 'watchSnapshot', 'watchProgress']);

/** Names of every bridge command, which are exactly the ScreenActions keys. */
export const commandNames: readonly string[] = Object.keys(fixtureBridge()).filter((name) => !NOT_COMMANDS.has(name));

export function mockActions(): MockActions {
  return Object.fromEntries(commandNames.map((name) => [name, vi.fn(() => Promise.resolve())])) as unknown as MockActions;
}
