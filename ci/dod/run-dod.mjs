// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Mutations use visible UI/native input; app_snapshot is the only direct bridge call.
import { createRequire } from 'node:module';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import net from 'node:net';
const directory = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(directory, '../..');
const host = path.join(root, 'host');
const { chromium } = createRequire(path.join(host, 'package.json'))('playwright-core');
const required = name => { if (!process.env[name]) throw Error(`Missing ${name}`); return path.resolve(process.env[name]); };
const appPath = required('OME_DOD_APP');
const home = required('OME_HOME');
const output = required('OME_DOD_OUTPUT');
const gameDirectory = required('OME_DOD_GAME_APK_DIR');
const port = Number(process.env.OME_DOD_CDP_PORT ?? 9333);
const scrub = text => text.replaceAll(gameDirectory.replaceAll('\\', '\\\\'), '<private-fixtures>').replaceAll(gameDirectory, '<private-fixtures>').replaceAll(gameDirectory.replaceAll('\\', '/'), '<private-fixtures>').replace(/C:([\\/]+)Users\1[^\\/\r\n" ]+/gi, 'C:$1Users$1USER');
fs.mkdirSync(path.join(output, 'shots'), { recursive: true });
const plannedSteps = ['preflight', 'launch', 'S1.1-host-check', 'S1.2-WHPX', 'S1.4-download', 'S1.5-installer', 'S1.6-first-boot', 'S1.7-game-install', 'game-launch', 'stop', 'quit'];
const result = { startedAt: new Date().toISOString(), outcome: 'failed', steps: [], gaps: [], measurements: {}, probeItems: [], softwareRenderingRetry: false, utf8Check: '다시 확인' };
const writeJson = (name, value) => fs.writeFileSync(path.join(output, name), scrub(JSON.stringify(value, null, 2)) + '\n');
const log = (event, data = {}) => { const line = scrub(JSON.stringify({ at: new Date().toISOString(), event, ...data })); console.log(line); fs.appendFileSync(path.join(output, 'driver-output.txt'), line + '\n'); };
const native = args => {
  const r = spawnSync('pwsh', ['-NoProfile', '-File', path.join(directory, 'native.ps1'), ...args], { encoding: 'buffer', timeout: 45000 });
  const stdout = r.stdout?.toString('utf8') ?? '';
  const stderr = r.stderr?.toString('utf8') ?? '';
  if (r.status !== 0) throw Error(`native helper (${r.status}): ${stdout} ${stderr}`);
  return JSON.parse(stdout.trim());
};
const inventory = () => native(['-Inventory', '-HomePath', home]);
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
let app, page, browser, keepAwake, exited = false, appIdentity, homeOwned = false;
const ownedQemu = new Map();
class StopRun extends Error {}
async function until(fn, timeout, label, interval = 1000) {
  const end = Date.now() + timeout; let last;
  do {
    try { const value = await fn(); if (value) return value; } catch (error) { if (error instanceof StopRun) throw error; last = error.message; }
    if (exited) throw Error(`App exited while waiting for ${label}`);
    await delay(interval);
  } while (Date.now() < end);
  throw Error(`Timeout: ${label}${last ? ` (${last})` : ''}`);
}
function rememberProcesses() {
  const data = inventory();
  for (const item of data.processes) {
    if (item.pid === app?.pid) appIdentity = item;
    // Require this isolated home as well as the parent PID before accepting ownership.
    if (item.parent === app?.pid && item.name.startsWith('qemu-system') &&
        item.command?.toLowerCase().replaceAll('\\', '/').includes(home.toLowerCase().replaceAll('\\', '/') + '/')) ownedQemu.set(item.pid, item);
  }
  return data;
}
// A blocked product main thread (a modal dialog, a hung command) leaves an invoke pending forever, and
// until() only checks its deadline between attempts; CI run 16 sat 114 minutes past every bounded wait
// with no evidence written. Every await on the product therefore carries its own bound.
const withTimeout = (promise, ms, label) => Promise.race([promise, new Promise((_, reject) => { setTimeout(() => reject(Error(`Timeout: ${label} after ${ms} ms`)), ms).unref(); })]);
async function snapshot() {
  const s = await withTimeout(page.evaluate(() => window.__TAURI_INTERNALS__.invoke('app_snapshot')), 20000, 'app_snapshot invoke');
  log('snapshot', { snapshot: s }); return s;
}
async function capture(name, qemuPid) {
  const shot = native(['-AppPid', String(app.pid), ...(qemuPid ? ['-QemuPid', String(qemuPid)] : []), '-Shot', path.join(output, 'shots', `${name}.png`)]);
  log('capture', { name, ...shot });
  fs.writeFileSync(path.join(output, `${name}.txt`), scrub(await page.locator('body').innerText()), 'utf8');
  return shot;
}
async function desktopCapture(name) {
  try { log('desktop', { name, ...native(['-AppPid', String(app.pid), '-Desktop', '-Shot', path.join(output, 'shots', `${name}.png`)]) }); }
  catch (error) { log('desktop-capture-failed', { name, error: error.message }); }
}
const click = async name => { log('click', { name }); await page.getByRole('button', { name, exact: true }).click(); };
const rail = async name => { await page.getByRole('navigation', { name: '주 메뉴' }).getByRole('button', { name, exact: true }).click(); await delay(400); };
function gap(message) { result.gaps.push(message); log('product-gap', { message }); }
async function step(name, fn) {
  const row = { name, start: new Date().toISOString(), outcome: 'running' }; result.steps.push(row);
  log('step-begin', { name }); const start = Date.now();
  try { await fn(); row.outcome = 'passed'; }
  catch (error) { row.outcome = 'failed'; row.error = error.stack; throw error; }
  finally { row.end = new Date().toISOString(); row.durationMs = Date.now() - start; log('step-end', row); writeJson('dod-result.json', result); }
}
async function freePort() {
  await new Promise((resolve, reject) => { const s = net.createServer(); s.once('error', reject); s.listen(port, '127.0.0.1', () => s.close(resolve)); });
}
async function qemuWindow(timeout = 30000) {
  return until(async () => {
    const s = await snapshot(); rememberProcesses();
    if (!s.guest.pid || !ownedQemu.has(s.guest.pid)) return false;
    native(['-AppPid', String(app.pid), '-QemuPid', String(s.guest.pid)]); return s.guest.pid;
  }, timeout, 'owned SDL_app');
}
function recognizeImage(name, file) {
  const r = spawnSync('powershell.exe', ['-NoProfile', '-File', path.join(directory, 'ocr.ps1'), '-Image', file], { encoding: 'buffer', timeout: 30000 });
  if (r.status !== 0) throw new StopRun(`OCR unavailable: ${r.stderr?.toString('utf8')}`);
  const value = JSON.parse(r.stdout.toString('utf8')); const { lines, ...rest } = value; log('ocr', { name, ...rest }); return value;
}
async function stopGuest() {
  const s = await snapshot(); rememberProcesses();
  if (['stopped', 'failed'].includes(s.guest.state)) return;
  if (await page.getByRole('button', { name: '나중에 하기', exact: true }).count()) await click('나중에 하기');
  await rail('화면');
  const stop = page.getByRole('button', { name: '끄기', exact: true });
  if (await stop.count()) await stop.click(); else await click('취소');
  await until(async () => ['stopped', 'failed'].includes((await snapshot()).guest.state), 60000, 'guest stopped');
}
async function quitApp() {
  // The tray menu is the product's own quit path. On the hosted runner the product runs as a
  // different user than the desktop's explorer, its notification icon never registers, and the
  // menu does not open (runs 21 to 23); closing the main window is the next thing a person does.
  try { log('tray-quit', native(['-AppPid', String(app.pid), '-Quit'])); }
  catch (error) {
    gap(`Tray 종료 did not open a menu in this session; closed the main window instead: ${error.message.split('\n')[0]}`);
    log('window-close', native(['-AppPid', String(app.pid), '-Close']));
  }
  await until(() => exited, 30000, 'app exit', 200);
}
async function guestScreenshot(name) {
  const shots = path.join(home, 'screenshots');
  const before = new Set(fs.existsSync(shots) ? fs.readdirSync(shots) : []);
  await click('스크린샷');
  const file = await until(() => fs.existsSync(shots) && fs.readdirSync(shots).find(n => n.endsWith('.png') && !before.has(n)), 15000, 'product screenshot');
  const copy = path.join(output, name + '.png'); fs.copyFileSync(path.join(shots, file), copy); return copy;
}
// The guest's own notification permission prompt (Android 13 asks on the game's first start) is the
// one dialog the driver answers, with Allow, as a person does: M0 recorded that a declined prompt
// later cost the game its download service (docs/evidence/M0/findings-20260926.md 3). The button is
// read from the guest screenshot and tapped through the embedded guest window as a mouse click.
// Nothing else on the guest screen is touched: no account creation, no game interaction.
async function answerPermissionPrompt(name, seen) {
  const button = (seen.lines ?? []).find(line => /^a[il1]+ow$/i.test(line.text.trim()));
  if (!button) { log('permission-prompt-unreadable', { name, lines: seen.lines }); return false; }
  const target = { x: (button.x + button.w / 2) / seen.width, y: (button.y + button.h / 2) / seen.height };
  const pid = await qemuWindow();
  const tap = native(['-AppPid', String(app.pid), '-QemuPid', String(pid), '-Tap', `${target.x.toFixed(4)},${target.y.toFixed(4)}`]);
  log('permission-prompt', { name, button, target, ...tap });
  await delay(3000); await guestScreenshot(`${name}-after-allow`); return true;
}
// What the English OCR reads on each game capture (dev PC round 26). The splash is an icon on black
// (no text); the permission prompt names notifications; the game's title and data-download screens
// carry the publisher's copyright line, and the download prompt its size in GB (read as `S.IGB`),
// the voice-data ON/OFF toggle and the Wi-Fi advice. Korean text is not read by this engine.
function gameScreen(text) {
  const words = text.toLowerCase().replace(/\s+/g, '');
  if (words.includes('sendyounotifications')) return 'permission-prompt';
  const download = /[0-9s][.,][0-9il]+gb/.test(words) || (/wi.?fi/.test(words) && words.includes('off'));
  if (download) return 'download-prompt';
  if (/epidgames|rightsreserved/.test(words)) return 'game-ui';
  return words ? 'unknown' : 'blank';
}
// Whole-run watchdog: write the result and end the driver instead of idling until the job timeout.
const deadlineMinutes = Number(process.env.OME_DOD_DEADLINE_MIN ?? 100);
setTimeout(() => {
  log('watchdog', { minutes: deadlineMinutes, steps: result.steps.map(s => `${s.name}=${s.outcome}`) });
  result.outcome = 'failed'; result.error = `Watchdog: the run exceeded ${deadlineMinutes} minutes`;
  for (const owned of [...ownedQemu.values(), ...(appIdentity ? [appIdentity] : [])]) {
    try { native(['-KillOwnedPid', String(owned.pid), '-ExpectedCreation', owned.creation]); result.forcedCleanup = true; }
    catch (error) { log('watchdog-cleanup-failed', { pid: owned.pid, error: error.message }); }
  }
  result.endedAt = new Date().toISOString(); writeJson('dod-result.json', result); process.exit(1);
}, deadlineMinutes * 60000).unref();
try {
  await step('preflight', async () => {
    const pre = inventory(); log('preflight', { ...pre, utf8Check: result.utf8Check }); result.environment = pre;
    if (pre.processes.length) throw Error('Existing QEMU or ome.exe; no process was touched');
    const realHome = path.resolve(process.env.LOCALAPPDATA, 'OpenMobileEmulator').toLowerCase(); const h = home.toLowerCase();
    if (h === realHome || h.startsWith(realHome + path.sep) || realHome.startsWith(h + path.sep)) throw Error('Refusing developer product home or its ancestors/descendants');
    if (output === home || output.startsWith(home + path.sep)) throw Error('Evidence must be outside fresh home');
    // A fresh home may carry only a pre-verified ISO under artifacts/ (local reruns skip the 2.4 GB download).
    if (fs.existsSync(home)) {
      const entries = fs.readdirSync(home);
      const seeded = entries.length === 1 && entries[0] === 'artifacts' && fs.readdirSync(path.join(home, 'artifacts')).every(n => n.endsWith('.iso') || n.endsWith('.iso.verified'));
      if (entries.length && !seeded) throw Error('OME_HOME must be empty (or hold only artifacts/*.iso with .verified)');
    }
    for (const file of [appPath, path.join(path.dirname(appPath), 'ome-setup.exe')]) if (!fs.statSync(file).isFile()) throw Error(`Missing input ${file}`);
    const pulled = JSON.parse(fs.readFileSync(path.join(gameDirectory, 'pulled.json'), 'utf8').replace(/^﻿/, ''));
    if (pulled.package !== 'com.epidgames.trickcalrevive') throw Error('Unexpected private fixture package');
    for (const leaf of ['base.apk', 'split_config.arm64_v8a.apk', 'split_gpdeku.apk', 'split_gpdeku.config.arm64_v8a.apk']) {
      if (!fs.statSync(path.join(gameDirectory, leaf)).isFile()) throw Error('Private fixture split missing');
    }
    result.game = { package: pulled.package, versionName: pulled.versionName ?? pulled.version_name ?? null, versionCode: pulled.versionCode ?? pulled.version_code ?? null };
    if (result.game.versionName === null) gap('pulled.json has no version fields; installed version can only be read after successful installation.');
    const bundle = path.join(path.dirname(appPath), 'qemu');
    if (process.env.OME_DOD_QEMU_DIR && path.resolve(process.env.OME_DOD_QEMU_DIR) !== bundle) {
      if (fs.existsSync(bundle)) throw Error('Refusing to overwrite existing QEMU bundle');
      fs.cpSync(path.resolve(process.env.OME_DOD_QEMU_DIR), bundle, { recursive: true });
    }
    const exe = path.join(bundle, 'bin/qemu-system-x86_64.exe');
    result.qemu = { source: process.env.OME_DOD_QEMU_DIR ?? bundle, sha256: createHash('sha256').update(fs.readFileSync(exe)).digest('hex'), version: spawnSync(exe, ['--version'], { encoding: 'utf8' }).stdout };
    log('qemu-bundle', result.qemu); await freePort(); fs.mkdirSync(home, { recursive: true }); homeOwned = true;
  });
  // Keep the monitor awake for the whole run: a sleeping display stops DWM composition and every capture
  // would show stale content (docs/evidence/M2/embedded-display-freeze.md).
  keepAwake = spawn('powershell.exe', ['-NoProfile', '-File', path.join(directory, 'keep-awake.ps1')], { stdio: ['ignore', 'pipe', 'inherit'] });
  keepAwake.stdout.once('data', chunk => log('keep-awake', JSON.parse(chunk.toString('utf8').trim())));
  process.on('exit', () => { try { keepAwake?.kill(); } catch {} });
  await step('launch', async () => {
    const fd = fs.openSync(path.join(output, 'app-output.txt'), 'w');
    app = spawn(appPath, [], { cwd: host, env: { ...process.env, OME_HOME: home, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` }, stdio: ['ignore', fd, fd] }); fs.closeSync(fd);
    app.on('exit', (code, signal) => { exited = true; log('app-exit', { pid: app.pid, code, signal }); });
    app.on('error', error => { exited = true; log('app-spawn-error', { error: error.message }); });
    log('app-start', { pid: app.pid }); rememberProcesses();
    // 120 s: the hosted runner (2 cores, Hyper-V video) starts WebView2 cold; run 16 gave up at 30 s with no diagnostics.
    try { await until(async () => (await fetch(`http://127.0.0.1:${port}/json/version`)).ok, 120000, 'CDP'); }
    catch (error) { await desktopCapture('launch-no-cdp'); throw error; }
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    page = await until(() => browser.contexts().flatMap(c => c.pages()).find(p => /^(http:\/\/tauri\.localhost|tauri:\/\/localhost)\/(?:index\.html)?$/.test(p.url())), 15000, 'main page');
    page.setDefaultTimeout(12000); page.on('pageerror', e => log('pageerror', { error: e.message }));
    await until(async () => { const s = await snapshot(); return s.blocker !== null || await page.locator('.ome-wizard').count(); }, 30000, 'first UI');
    await capture('01-first-screen');
  });
  await step('S1.1-host-check', async () => {
    let s = await snapshot();
    if (!s.host.rows.length) { await delay(5000); s = await snapshot(); if (!s.host.rows.length) { gap('Host check did not run at startup; pressed visible 다시 확인.'); await click('다시 확인'); } }
    s = await until(async () => { const s = await snapshot(); return s.host.rows.length && s; }, 30000, 'host rows');
    result.hostVerdict = s.host; await capture('02-host-verdict');
    if (!s.wizard.canContinue) {
      gap(`Host check blocks wizard: ${JSON.stringify(s.blocker)}; blocked rows: ${s.host.rows.filter(r => r.status !== 'ready').map(r => r.id).join(', ')}`);
      await click('다시 확인'); await delay(1500); s = await snapshot();
      await capture('02b-host-rechecked');
      if (!s.wizard.canContinue) throw Error('Host check still blocks Continue after visible 다시 확인; no product bypass permitted');
    }
    await click('계속');
  });
  await step('S1.2-WHPX', async () => {
    let s = await snapshot(); await capture('03-whpx-or-image');
    if (s.wizard.step === 'whpxConsent') { await click('활성화'); s = await until(async () => { const n = await snapshot(); return n.wizard.step !== 'whpxConsent' && n; }, 120000, 'WHPX activation'); }
    else gap('WHPX already enabled: product skips S1.2 instead of showing an already-on screen.');
    if (s.wizard.step === 'rebootPending') throw Error('Restart requested; stop condition, never reboot automatically');
    if (s.wizard.step !== 'artifactDownload') throw Error(`Unexpected wizard step ${s.wizard.step}`);
  });
  await step('S1.4-download', async () => {
    const start = Date.now(); let nextProgress = 0;
    let s = await snapshot();
    const profile = s.images?.profiles?.find(p => p.id === s.wizard.imageId);
    if (!s.wizard.download && s.wizard.canContinue && profile?.status === 'verified') {
      // A seeded home (artifacts/*.iso with its .verified marker) is verified at first sight: download is null and the wizard shows 다음 only.
      result.measurements.downloadSkipped = true; log('download-skipped', { reason: 'image already verified in the seeded home', profile: profile.id });
    } else {
      // A SourceForge mirror dropped the 2.4 GB transfer at 953 MB in CI run 23 (3 MB/s that time,
      // 44 MB/s the two runs before). A person presses 다운로드 again; the driver does so once and
      // records it, so a second failure is still the run's verdict.
      let attempts = 0;
      await click('다운로드');
      s = await until(async () => {
        const s = await snapshot(); const d = s.wizard.download;
        if (['failed', 'cancelled'].includes(d?.stage)) {
          if (attempts++ >= 1) throw new StopRun(`Artifact download ${d.stage} twice: ${JSON.stringify(d)}`);
          log('download-retry', { attempt: attempts + 1, previous: d, issue: s.issue });
          result.measurements.downloadRetries = attempts;
          await capture(`04a-download-failed-${attempts}`);
          await click('다운로드');
          return false;
        }
        if (Date.now() >= nextProgress) { log('download-progress', { progress: d }); nextProgress = Date.now() + 30000; }
        return d?.stage === 'verified' && s;
      }, 45 * 60000, 'verified artifact', 5000);
      result.measurements.downloadMs = Date.now() - start; result.measurements.downloadBytes = s.wizard.download.doneBytes;
      result.measurements.downloadAverageBytesPerSecond = s.wizard.download.doneBytes / (result.measurements.downloadMs / 1000);
    }
    await capture('04-image-verified'); await click('다음'); await capture('05-disk-size');
  });
  await step('S1.5-installer', async () => {
    const start = Date.now();
    await delay(1800); await click('설치하기');
    // The install runs hidden (ADR-0010): the product boots the ISO's kernel with its helper, no
    // window, and moves the wizard to the first boot by itself. The driver only reads the
    // snapshot's progress; there is nothing to operate.
    let progress = null;
    await until(async () => {
      const s = await snapshot(); rememberProcesses();
      const install = s.wizard.install;
      if (install && (install.stage !== progress?.stage || install.percent !== progress?.percent)) { progress = install; log('install-progress', install); }
      if (install?.stage === 'failed') throw new StopRun(`Unattended install failed: ${install.failure ?? 'no reason'} (${install.logPath ?? 'no log'})`);
      return s.wizard.step === 'firstBoot' && s;
    }, 20 * 60000, 'unattended install', 2000);
    result.measurements.installMs = Date.now() - start; result.install = progress;
    await capture('06-install-complete');
  });
  await step('S1.6-first-boot', async () => {
    const start = Date.now();
    const softwareRendering = result.softwareRenderingRetry || (await snapshot()).settings.gpuMode === 'software';
    result.measurements.softwareRendering = softwareRendering;
    // Direct kernel boot (ADR-0010): the product passes the boot arguments itself, software
    // rendering included (nomodeset HWACCEL=0). The driver sends no keys and reads no menu.
    log('first-boot-mode', { softwareRendering });
    const s = await until(async () => { const s = await snapshot(); return s.guest.bootCompleted && s.guest.capabilities.items.some(i => i.state !== 'unknown') && s; }, 10 * 60000, 'first boot and probe', 3000);
    result.measurements.firstBootMs = Date.now() - start; result.probeItems = s.guest.capabilities.items;
    await capture('07-first-boot-probe'); log('applied-defaults', { text: await page.locator('body').innerText() }); await click('다음');
  });
  await step('S1.7-game-install', async () => {
    await capture('08-app-install');
    gap('S1.7 has no launch/capture actions; complete wizard before installing from 앱 as directed for round 2.');
    await click('완료'); await rail('앱');
    const start = Date.now(); await click('설치');
    native(['-AppPid', String(app.pid), '-FilePath', gameDirectory, '-GameSelection']);
    log('file-dialog', { closed: true, selection: 'four private split APKs' });
    const s = await until(async () => { const s = await snapshot(); return s.apps.items.some(a => a.package === result.game.package) && s; }, 300000, 'game installed');
    result.game.installedVersion = s.apps.items.find(a => a.package === result.game.package).versionName;
    result.measurements.gameInstallMs = Date.now() - start; await capture('09-game-installed');
    await rail('앱');
  });
  await step('game-launch', async () => {
    const start = Date.now();
    await page.getByRole('row').filter({ hasText: result.game.package }).getByRole('button', { name: '실행', exact: true }).click(); await rail('화면');
    // No account creation or unapproved game interaction. Fifteen minutes of product captures
    // document whether a consent/resource-download dialog blocks reaching the title screen. The
    // OS's notification permission prompt is answered (answerPermissionPrompt); dev PC round 25 sat
    // on it for the whole observation. Each capture is read (gameScreen) so the run can certify
    // the game's own data-download prompt without a human eye (dev PC round 26 reached it in
    // three minutes and held it to the end).
    let permissionAnswers = 0; const screens = [];
    for (let minute = 0; minute <= 15; minute++) {
      if (minute) await delay(60000);
      await snapshot(); const name = `game-${String(minute).padStart(2, '0')}`; const file = await guestScreenshot(name);
      const seen = recognizeImage(name, file); const screen = gameScreen(seen.text);
      screens.push({ minute, screen }); log('game-screen', { name, screen });
      if (screen === 'permission-prompt' && permissionAnswers < 2 && await answerPermissionPrompt(name, seen)) permissionAnswers++;
    }
    result.measurements.permissionPromptAnswers = permissionAnswers;
    result.game.screens = screens;
    result.game.reachedDownloadPrompt = screens.some(s => s.screen === 'download-prompt');
    // The last capture must still be the game's own screen: a crash back to the launcher or a
    // black frame after the prompt would leave the run for a human to judge.
    const last = screens.at(-1)?.screen;
    result.game.visualReviewRequired = !(result.game.reachedDownloadPrompt && ['download-prompt', 'game-ui'].includes(last));
    result.measurements.gameLaunchObservationMs = Date.now() - start;
    await capture('13-game-final-stage');
  });
  await step('stop', async () => { const start = Date.now(); await stopGuest(); result.measurements.stopMs = Date.now() - start; await capture('14-stopped'); });
  await step('quit', quitApp);
  // A game capture nobody read (neither the OCR above nor a person) cannot certify a running game.
  result.outcome = result.game.visualReviewRequired ? 'needs-review' : 'passed';
} catch (error) {
  result.error = error.stack; log('failure', { error: error.stack });
  // A failure capture focuses the main window and refuses when another window holds the foreground
  // (CI run 25: the guest window). The desktop copy needs no focus and still shows what was on screen.
  if (page && !page.isClosed() && !exited) await capture('failure').catch(async e => { log('capture-failed', { error: e.message }); await desktopCapture('failure'); });
} finally {
  await step('cleanup', async () => {
    if (app && !exited) { try { await stopGuest(); await quitApp(); } catch (error) { log('graceful-cleanup-failed', { error: error.stack }); } }
    if (app) {
      rememberProcesses();
      for (const owned of [...ownedQemu.values(), ...(appIdentity ? [appIdentity] : [])]) {
        if (!inventory().processes.some(p => p.pid === owned.pid && p.creation === owned.creation)) continue;
        log('forced-cleanup', { process: owned }); native(['-KillOwnedPid', String(owned.pid), '-ExpectedCreation', owned.creation]); result.forcedCleanup = true;
      }
    }
    await delay(800); result.finalProcesses = inventory().processes; log('final-processes', { processes: result.finalProcesses });
    if (result.finalProcesses.length) throw Error('Processes remain; unrelated processes are not terminated');
    if (homeOwned) {
      const logs = path.join(home, 'logs');
      if (fs.existsSync(logs)) {
        fs.mkdirSync(path.join(output, 'logs'), { recursive: true });
        for (const entry of fs.readdirSync(logs, { withFileTypes: true })) if (entry.isFile()) fs.writeFileSync(path.join(output, 'logs', entry.name.replace(/\.[^.]+$/, '') + '.txt'), scrub(fs.readFileSync(path.join(logs, entry.name), 'utf8')), 'utf8');
      }
      result.temporaryHome = home;
    }
  }).catch(error => { result.outcome = 'failed'; result.cleanupError = error.stack; });
  if (result.forcedCleanup) result.outcome = 'failed';
  for (const name of plannedSteps) if (!result.steps.some(s => s.name === name)) result.steps.push({ name, start: null, end: null, durationMs: null, outcome: 'not-run', reason: 'An earlier step failed; no bypass attempted' });
  result.endedAt = new Date().toISOString(); writeJson('dod-result.json', result);
  const appOutput = path.join(output, 'app-output.txt'); if (fs.existsSync(appOutput)) fs.writeFileSync(appOutput, scrub(fs.readFileSync(appOutput, 'utf8')), 'utf8');
  if (browser) await withTimeout(browser.close(), 10000, 'browser close').catch(() => {});
}
// The keep-awake helper's piped stdout kept the event loop alive after the run had ended, so the driver
// never exited on its own (CI run 16: finished at 01:48, still running when the job was cancelled at
// 03:41). End the helper and the process explicitly.
try { keepAwake?.kill(); } catch {}
process.exit(result.outcome === 'passed' ? 0 : 1);
