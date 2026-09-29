// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Walks the wizard to the installer (clicks 디스크 만들기), prints the QEMU pid once running, and exits.
import { createRequire } from 'node:module';
import path from 'node:path';
const host = 'C:/Open Mobile Emulator/host';
const { chromium } = createRequire(path.join(host, 'package.json'))('playwright-core');
const port = Number(process.argv[2]);
const delay = ms => new Promise(r => setTimeout(r, ms));
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const page = browser.contexts().flatMap(c => c.pages()).find(p => /^(http:\/\/tauri\.localhost|tauri:\/\/localhost)\/(?:index\.html)?$/.test(p.url()));
if (!page) throw new Error('main page not found');
page.setDefaultTimeout(8000);
const snap = async () => page.evaluate(() => window.__TAURI_INTERNALS__.invoke('app_snapshot'));
const clickIf = async (re) => { const b = page.getByRole('button', { name: re }); if (await b.count()) { await b.first().click(); await delay(1000); return true; } return false; };
for (let round = 0; round < 10; round++) {
  const s = await snap();
  if (s.wizard?.step === 'hostCheck') { await clickIf(/^계속$/); continue; }
  if (s.wizard?.step === 'artifactDownload') {
    if (s.wizard.download?.stage !== 'verified') { await clickIf(/^다운로드$/); let n = 0; while ((await snap()).wizard?.download?.stage !== 'verified' && n++ < 120) await delay(2000); }
    await clickIf(/^다음$/); continue;
  }
  if (s.wizard?.step === 'guestInstall') { await clickIf(/^디스크 만들기$/); break; }
  throw new Error('unexpected step ' + s.wizard?.step);
}
let pid = null;
for (let n = 0; n < 40 && !pid; n++) { const s = await snap(); pid = s.guest.pid; if (!pid) await delay(250); }
console.log(String(pid ?? 0));
await browser.close();
