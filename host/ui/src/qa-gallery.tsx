// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Synthetic gallery: the real App over fixture snapshots, with a state and theme picker.
// Loaded only by qa-main.tsx (build mode `qa`); the product build rejects this module.
import { useMemo, useState } from 'react';
import { App } from './App';
import { blockedGallery, fixtureBridge, mainGallery, wizardGallery } from './fixtures';
import type { GalleryVariant } from './fixtures';

const GROUPS: readonly { readonly label: string; readonly variants: readonly GalleryVariant[] }[] = [
  { label: '첫 실행 마법사', variants: wizardGallery },
  { label: '막힘 화면', variants: blockedGallery },
  { label: '주 화면', variants: mainGallery },
];

type Theme = 'system' | 'dark' | 'light';

function allVariants(): GalleryVariant[] {
  return GROUPS.flatMap((group) => group.variants);
}

function readInitial(): string {
  const requested = new URLSearchParams(window.location.search).get('state');
  return allVariants().find((variant) => variant.id === requested)?.id ?? allVariants()[0]?.id ?? '';
}

function applyTheme(theme: Theme) {
  if (theme === 'system') document.documentElement.removeAttribute('data-theme');
  else document.documentElement.setAttribute('data-theme', theme);
}

export function Gallery() {
  const [selected, setSelected] = useState(readInitial);
  const [theme, setTheme] = useState<Theme>('system');
  const variant = allVariants().find((item) => item.id === selected);
  const bridge = useMemo(() => fixtureBridge(variant?.snapshot), [variant]);

  return (
    <>
      <App key={selected} bridge={bridge} />
      <div className="ome-qa-picker" role="region" aria-label="갤러리 상태">
        <label className="ome-qa-field">
          <span>상태</span>
          <select
            value={selected}
            onChange={(event) => {
              const next = event.currentTarget.value;
              setSelected(next);
              const url = new URL(window.location.href);
              url.searchParams.set('state', next);
              window.history.replaceState(null, '', url);
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
              applyTheme(next);
            }}
          >
            <option value="system">시스템</option>
            <option value="dark">어두운 테마</option>
            <option value="light">밝은 테마</option>
          </select>
        </label>
      </div>
    </>
  );
}
