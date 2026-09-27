// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Synthetic gallery: the real App over fixture snapshots, with a state and theme picker.
// Loaded only by qa-main.tsx (build mode `qa`); the product build rejects this module.
import { useEffect, useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import { App } from './App';
import { blockedGallery, fixtureBridge, mainGallery, wizardGallery } from './fixtures';
import type { GalleryVariant } from './fixtures';
// Stage and apps variants (screen worker B).
import { appsGallery, stageGallery } from './fixtures';
import { useRailPick } from './qa-rail-pick';
// Input, display and settings variants (screen worker C).
import { displayGallery, inputGallery, settingsGallery } from './fixtures';
// The overlay window's page (S4-오버레이), drawn in a 1280x800 frame instead of the App.
import { overlayGallery } from './fixtures';
import type { OverlayGalleryVariant } from './fixtures';
import { OverlayWindow } from './overlay/OverlayWindow';
import './styles/overlay.css';

const GROUPS: readonly { readonly label: string; readonly variants: readonly GalleryVariant[] }[] = [
  { label: '첫 실행 마법사', variants: wizardGallery },
  { label: '막힘 화면', variants: blockedGallery },
  { label: '주 화면', variants: mainGallery },
  { label: '무대 (S2)', variants: stageGallery },
  { label: '앱 (S3)', variants: appsGallery },
  { label: '입력 (S4)', variants: inputGallery },
  { label: '표시 (S5)', variants: displayGallery },
  { label: '설정 (S6)', variants: settingsGallery },
  { label: '오버레이 (S4)', variants: overlayGallery },
];

/** Variants drawn on a rail screen other than the stage: variant id to rail label (worker B). */
const RAIL_PICKS: ReadonlyMap<string, string> = new Map([
  ...appsGallery.map((item) => [item.id, '앱'] as const),
  // Worker C: the input, display and settings screens.
  ...inputGallery.map((item) => [item.id, '입력'] as const),
  ...displayGallery.map((item) => [item.id, '표시'] as const),
  ...settingsGallery.map((item) => [item.id, '설정'] as const),
]);

type Theme = 'system' | 'dark' | 'light';

/**
 * Stand-in for the operating system's screen under the overlay: a dark gradient with a faint grid,
 * never game imagery (R6). Fixed at 1280x800 so the page measures the same box in every browser.
 */
const OVERLAY_FRAME: CSSProperties = {
  position: 'fixed',
  top: 0,
  left: 0,
  width: '1280px',
  height: '800px',
  overflow: 'hidden',
  backgroundColor: 'var(--stage-frame)',
  backgroundImage: [
    'linear-gradient(color-mix(in srgb, var(--line) 40%, transparent) 1px, transparent 1px)',
    'linear-gradient(90deg, color-mix(in srgb, var(--line) 40%, transparent) 1px, transparent 1px)',
    'linear-gradient(160deg, var(--surface-raised), var(--stage-frame) 75%)',
  ].join(', '),
  backgroundSize: '40px 40px, 40px 40px, 100% 100%',
};

function OverlayPreview({ variant }: { readonly variant: OverlayGalleryVariant }) {
  const bridge = useMemo(() => fixtureBridge(variant.snapshot), [variant]);
  return (
    <div style={OVERLAY_FRAME}>
      <OverlayWindow bridge={bridge} seed={variant.seed} />
    </div>
  );
}

function allVariants(): GalleryVariant[] {
  return GROUPS.flatMap((group) => group.variants);
}

function readInitial(): string {
  const requested = new URLSearchParams(window.location.search).get('state');
  return allVariants().find((variant) => variant.id === requested)?.id ?? allVariants()[0]?.id ?? '';
}

/** `?theme=dark|light` picks the theme up front, so a headless browser can shoot both themes. */
function readInitialTheme(): Theme {
  const requested = new URLSearchParams(window.location.search).get('theme');
  return requested === 'dark' || requested === 'light' ? requested : 'system';
}

/** `?picker=hidden` drops the picker, which otherwise covers the bottom-right corner of the screen. */
function pickerHidden(): boolean {
  return new URLSearchParams(window.location.search).get('picker') === 'hidden';
}

function writeParam(name: string, value: string) {
  const url = new URL(window.location.href);
  url.searchParams.set(name, value);
  window.history.replaceState(null, '', url);
}

function applyTheme(theme: Theme) {
  if (theme === 'system') document.documentElement.removeAttribute('data-theme');
  else document.documentElement.setAttribute('data-theme', theme);
}

export function Gallery() {
  const [selected, setSelected] = useState(readInitial);
  const [theme, setTheme] = useState<Theme>(readInitialTheme);
  const variant = allVariants().find((item) => item.id === selected);
  const overlay = overlayGallery.find((item) => item.id === selected);
  const bridge = useMemo(() => fixtureBridge(variant?.snapshot), [variant]);
  useRailPick(selected, RAIL_PICKS);
  useEffect(() => {
    // The overlay window fixes the dark theme (overlay.html), whatever the picker says.
    applyTheme(overlay === undefined ? theme : 'dark');
  }, [theme, overlay]);

  return (
    <>
      {overlay === undefined ? <App key={selected} bridge={bridge} /> : <OverlayPreview key={selected} variant={overlay} />}
      {pickerHidden() ? null : (
      <div className="ome-qa-picker" role="region" aria-label="갤러리 상태">
        <label className="ome-qa-field">
          <span>상태</span>
          <select
            value={selected}
            onChange={(event) => {
              const next = event.currentTarget.value;
              setSelected(next);
              writeParam('state', next);
            }}
          >
            {GROUPS.map((group) => (
              <optgroup key={group.label} label={group.label}>
                {group.variants.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}
              </optgroup>
            ))}
          </select>
        </label>
        <label className="ome-qa-field">
          <span>테마</span>
          <select
            value={theme}
            onChange={(event) => {
              const value = event.currentTarget.value;
              const next: Theme = value === 'dark' || value === 'light' ? value : 'system';
              setTheme(next);
              writeParam('theme', next);
            }}
          >
            <option value="system">시스템</option>
            <option value="dark">어두운 테마</option>
            <option value="light">밝은 테마</option>
          </select>
        </label>
      </div>
      )}
    </>
  );
}
