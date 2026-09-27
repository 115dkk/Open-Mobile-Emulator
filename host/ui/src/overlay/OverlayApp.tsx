// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The overlay page (ADR-0005, ARCHITECTURE.md 3.17): a transparent second window Rust lays exactly over
// the operating system's screen inside the stage. Its own box is that screen. Outside the editor it only
// draws the applied profile's markers and lets every click through; in the editor it is the editor.
// A pure component over the snapshot, like the screens; `OverlayWindow` connects it to the bridge.
import { useState } from 'react';
import type { ScreenActions } from '../actions';
import type { AppSnapshot, InputProfile, InputView, Size } from '../contracts';
import { Icon } from '../components';
import { keyLabel } from '../input-profile';
import { BindingLines, BindingMarker } from './Markers';
import { activeProfile, overlayMode } from './model';
import type { OverlaySeed } from './model';
import { OverlayEditor } from './OverlayEditor';
import { useElementSize } from './use-element-size';
import { usePresence } from './use-presence';

export interface OverlayAppProps {
  readonly snapshot: AppSnapshot;
  readonly actions: ScreenActions;
  /** QA gallery only: the editor state to start from. */
  readonly seed?: OverlaySeed | undefined;
}

/** Markers only: nothing here takes a click, a key or focus. */
function ShowingLayer({ profile, input, size }: { readonly profile: InputProfile; readonly input: InputView; readonly size: Size }) {
  const present = usePresence(profile.id, profile.bindings);
  return (
    <>
      <div className={input.suspended ? 'ome-overlay-markers ome-overlay-markers-suspended' : 'ome-overlay-markers'}>
        {present.map(({ key, binding, leaving }) => (
          <div key={`${key}/lines`} className={leaving ? 'ome-overlay-binding ome-overlay-binding-leaving' : 'ome-overlay-binding'}>
            <BindingLines action={binding.action} size={size} />
          </div>
        ))}
        {present.map(({ key, binding, leaving }) => (
          <div key={key} className={leaving ? 'ome-overlay-binding ome-overlay-binding-leaving' : 'ome-overlay-binding'}>
            <BindingMarker binding={binding} action={binding.action} size={size} />
          </div>
        ))}
      </div>
      {input.suspended && (
        <div className="ome-overlay-top">
          <span className="ome-overlay-chip">
            <Icon name="pause" size={14} strokeWidth={2} />
            {`매핑 일시 중지 (${keyLabel(input.suspendHotkey)})`}
          </span>
        </div>
      )}
    </>
  );
}

export function OverlayApp({ snapshot, actions, seed }: OverlayAppProps) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const size = useElementSize(root);
  const mode = overlayMode(snapshot);
  const profile = activeProfile(snapshot);
  if (mode === 'hidden' || profile === null) return null;
  return (
    <div ref={setRoot} className={`ome-overlay ome-overlay-${mode}`} data-mode={mode}>
      {mode === 'editing' ? (
        <OverlayEditor key={profile.id} profile={profile} snapshot={snapshot} actions={actions} size={size} seed={seed} />
      ) : (
        <ShowingLayer profile={profile} input={snapshot.input} size={size} />
      )}
    </div>
  );
}
