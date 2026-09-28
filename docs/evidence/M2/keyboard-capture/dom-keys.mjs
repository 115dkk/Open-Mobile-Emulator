// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Checks whether key events reach the product webview's DOM while the app is the foreground window.
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
const host = process.env.OME_HOST_DIR ?? 'C:/Open Mobile Emulator/host';
const scratch = process.env.OME_SCRATCH;
const { chromium } = createRequire(path.join(host, 'package.json'))('playwright-core');
const port = Number(process.argv[2]);
const appPid = process.argv[3];
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const page = browser.contexts().flatMap(c => c.pages()).find(p => /^(http:\/\/tauri\.localhost|tauri:\/\/localhost)\/(?:index\.html)?$/.test(p.url()));
if (!page) throw new Error('main page not found: ' + browser.contexts().flatMap(c => c.pages()).map(p => p.url()).join(', '));
await page.evaluate(() => {
  window.__keys = [];
  const record = kind => e => { window.__keys.push(`${kind} code=${e.code} key=${e.key} repeat=${e.repeat} target=${e.target?.tagName ?? '?'}`); };
  window.addEventListener('keydown', record('down'), true);
  window.addEventListener('keyup', record('up'), true);
});
console.log('listener installed; focused element:', await page.evaluate(() => document.activeElement?.tagName + '/' + (document.activeElement?.textContent ?? '').slice(0, 20)));
const r = spawnSync('pwsh', ['-NoProfile', '-File', `${scratch}/focus-and-tap.ps1`, '-AppPid', appPid], { encoding: 'utf8', timeout: 30000 });
console.log(r.stdout.trim(), r.stderr.trim());
await new Promise(resolve => setTimeout(resolve, 800));
const keys = await page.evaluate(() => window.__keys);
console.log('DOM saw', keys.length, 'events');
for (const k of keys) console.log('  ' + k);
await browser.close();
