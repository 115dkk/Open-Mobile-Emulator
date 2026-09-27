// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The overlay window's root: the same snapshot and commands as the main window, through the same
// controller. Until the first snapshot arrives it draws nothing, since it sits over the game.
import type { ControllerBridge } from '../contracts';
import { useController } from '../use-controller';
import { OverlayApp } from './OverlayApp';
import type { OverlaySeed } from './model';

export function OverlayWindow({ bridge, seed }: { readonly bridge: ControllerBridge; readonly seed?: OverlaySeed | undefined }) {
  const { snapshot, issue, actions } = useController(bridge);
  if (snapshot === null) return null;
  // A failed command keeps the last snapshot; its issue is shown through the same field (App does the same).
  const view = snapshot.issue === issue ? snapshot : { ...snapshot, issue };
  return <OverlayApp snapshot={view} actions={actions} seed={seed} />;
}
