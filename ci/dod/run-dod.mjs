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
  log('capture', { name, ...native(['-AppPid', String(app.pid), ...(qemuPid ? ['-QemuPid', String(qemuPid)] : []), '-Shot', path.join(output, 'shots', `${name}.png`)]) });
  fs.writeFileSync(path.join(output, `${name}.txt`), scrub(await page.locator('body').innerText()), 'utf8');
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
async function qemuWindow() {
  return until(async () => {
    const s = await snapshot(); rememberProcesses();
    if (!s.guest.pid || !ownedQemu.has(s.guest.pid)) return false;
    native(['-AppPid', String(app.pid), '-QemuPid', String(s.guest.pid)]); return s.guest.pid;
  }, 30000, 'owned SDL_app');
}
async function keys(pid, label, keySequence, text, waitMs = 800) {
  log('installer-input', { label, ...native(['-AppPid', String(app.pid), '-QemuPid', String(pid), ...(keySequence ? ['-Keys', keySequence] : []), ...(text ? ['-Text', text] : [])]) });
  await delay(waitMs); await capture(`installer-${label}`, pid);
}
function recognize(name) {
  const r = spawnSync('powershell.exe', ['-NoProfile', '-File', path.join(directory, 'ocr.ps1'), '-Image', path.join(output, 'shots', name + '.png')], { encoding: 'buffer', timeout: 30000 });
  if (r.status !== 0) throw new StopRun(`Installer OCR unavailable: ${r.stderr?.toString('utf8')}`);
  const value = JSON.parse(r.stdout.toString('utf8')); log('installer-ocr', { name, ...value }); return value.text;
}
const installerText = text => text.toLowerCase().replace(/\s+/g, ' ').trim();
function installerScreen(text) {
  // OCR sometimes splits a word ("fi lesystem", "Bl issOS") across text boxes.
  const words = text.replace(/\s/g, '');
  const has = phrase => words.includes(phrase.replace(/\s/g, ''));
  if (has('congratulations') && has('installed successfully')) return 'done';
  if (has('choose partition')) {
    if (has('efi system partition')) return 'esp-chooser';
    if (has('install blissos') || has('select a partition to install')) return 'system-chooser';
    return 'unknown'; // A chooser must never fall through to a LEFT/RIGHT action.
  }
  // OCR read this title as "Choose r i lesysten" (round 9); the body line "Please select a filesystem to
  // format" and the option list are the reliable parts. The ESP chooser offers only "Do not re-format" and
  // fat32 (read as "fdt3Z"); the system chooser is the one with ext4.
  if (has('choose filesystem') || (/lesyste[mn]/.test(words) && (has('please select') || has('choose')))) {
    if (has('ext4')) return 'filesystem-for-system';
    if (/fat3|fdt3/.test(words) || has('do not re')) return 'filesystem-for-esp';
    return 'unknown';
  }
  if (has('warning') || (/\b\d+\s*(?:s\b|sec|second)/.test(text) && /\b[0o]k\b/.test(text) && !/\byes\b|\bno\b|reboot/i.test(text))) return 'warning-countdown';
  if (has('error') || has('this is not an efi system partition')) return 'error';
  if (has('ota')) return 'ota-confirm';
  if (has('grub2') || has('choose efi boot')) return 'efi-boot-chooser';
  if (has('confirm') && has('format')) return 'confirm-format';
  if (has('question') && has('label')) return 'label-question';
  if (has('installing') || has('expect to write')) return 'installing';
  return 'unknown';
}
async function installerPid(pid) {
  const current = (await snapshot()).guest.pid;
  if (current !== pid) throw new StopRun(`Guest restarted during installer: expected QEMU ${pid}, snapshot guest.pid ${current}; refusing to follow a new process`);
}
async function installGuest(pid) {
  // Move the webview focus off the create/completion button with a real noninteractive UI click.
  // While the stage is active the product forwards every key to the guest and prevents the default.
  await page.getByRole('heading', { name: '운영체제 설치', exact: true }).click();
  // docs/evidence/M0/guest-install.md 69-94. Never send these to an existing disk.
  // No keys before the menu is recognized: keys during the firmware phase reach the firmware now and
  // left one variable store booting slowly (docs/evidence/M2/embedded-display-freeze.md). The ISO GRUB
  // menu waits about 30 s, so recognition first, then one Home to reset its countdown.
  await until(async () => {
    await capture('installer-00-grub-menu', pid);
    return /installation/i.test(recognize('installer-00-grub-menu'));
  }, 60000, 'installer GRUB menu', 100);
  await snapshot();
  await keys(pid, '01a-grub-home', 'HOME');
  for (let arrow = 1; arrow <= 4; arrow++) await keys(pid, `01a-grub-down-${arrow}`, 'DOWN');
  await keys(pid, '01-grub-installation', 'ENTER', null, 30000);
  if (!/partition/i.test(recognize('installer-01-grub-installation'))) throw new StopRun('Expected installer partition dialog; refusing blind partition keystrokes');
  await keys(pid, '02-partition-menu', null, 'c');
  await keys(pid, '03-cfdisk-confirm', 'ENTER');
  await keys(pid, '04-cfdisk-label', 'ENTER');
  await keys(pid, '05-gpt', 'ENTER');
  await keys(pid, '06-new-esp', null, 'n');
  await keys(pid, '07-esp-size', Array(16).fill('BACKSPACE').join(','), '512M');
  await keys(pid, '08-create-esp', 'ENTER');
  await keys(pid, '09-type-list', null, 't');
  await keys(pid, '10-efi-type', 'HOME,ENTER');
  await keys(pid, '11-free-space', 'DOWN');
  await keys(pid, '12-new-system', null, 'n');
  await keys(pid, '13-system-size', 'ENTER');
  await keys(pid, '14-write', null, 'W');
  await keys(pid, '15-write-yes', null, 'yes');
  await keys(pid, '16-written', 'ENTER');
  await keys(pid, '17-quit-cfdisk', null, 'q', 7000);
  const partitionText = installerText(recognize('installer-16-written')).replace(/\s/g, '');
  if (!partitionText.includes('efisystem') || !partitionText.includes('linuxfilesystem')) {
    throw new StopRun('cfdisk did not show both EFI System and Linux filesystem before Quit; refusing installer input');
  }
  const deadline = Date.now() + 12 * 60000;
  let espFormatted = false, formatTarget, previousScreen, acted = false, unknownSince;
  let check = 0, actionNumber = 30, errors = 0, done = false;
  const lastTexts = [];
  while (Date.now() < deadline) {
    await delay(1500);
    await installerPid(pid);
    const label = `installer-screen-${String(check++).padStart(3, '0')}`;
    try { await capture(label, pid); }
    catch (error) { await installerPid(pid); throw error; }
    const text = installerText(recognize(label));
    const screen = installerScreen(text);
    lastTexts.push(text); if (lastTexts.length > 4) lastTexts.shift();
    if (screen !== previousScreen) acted = false;
    previousScreen = screen;
    if (screen === 'unknown') unknownSince ??= Date.now(); else unknownSince = undefined;
    let action = 'wait', sequence;
    if (screen === 'done') action = 'finish';
    else if (!acted) {
      switch (screen) {
        case 'esp-chooser':
          formatTarget = 'esp'; sequence = 'HOME,ENTER';
          action = espFormatted ? 'select-vda1-without-reformat' : 'select-vda1'; break;
        case 'filesystem-for-esp':
          formatTarget = 'esp'; sequence = espFormatted ? 'HOME,ENTER' : 'DOWN,ENTER';
          action = espFormatted ? 'do-not-reformat' : 'select-fat32'; break;
        case 'system-chooser':
          formatTarget = 'system'; sequence = 'HOME,DOWN,ENTER'; action = 'select-vda2'; break;
        case 'filesystem-for-system':
          formatTarget = 'system'; sequence = 'DOWN,ENTER'; action = 'select-ext4'; break;
        case 'label-question': sequence = 'ENTER'; action = 'keep-label'; break;
        case 'confirm-format':
          if (!formatTarget) throw new StopRun(`Format confirmation without a classified target: ${text}`);
          sequence = 'LEFT,ENTER'; action = `confirm-format-${formatTarget}`; break;
        case 'ota-confirm': sequence = 'RIGHT,ENTER'; action = 'decline-ota'; break;
        case 'efi-boot-chooser': sequence = 'HOME,ENTER'; action = 'select-grub2'; break;
        case 'error':
          errors++; action = errors > 3 ? 'stop-after-errors' : 'acknowledge-error';
          if (errors <= 3) sequence = 'ENTER'; break;
      }
    }
    log('installer-screen', { screen, action });
    if (errors > 3) throw new StopRun(`More than three installer errors: ${JSON.stringify(lastTexts)}`);
    if (unknownSince !== undefined && Date.now() - unknownSince >= 20000) throw new StopRun(`Unknown installer screen for 20 seconds: ${text}`);
    if (screen === 'done') { done = true; break; }
    if (sequence) {
      if (Date.now() >= deadline) break;
      await installerPid(pid);
      try { await keys(pid, `${actionNumber++}-${screen}-${action}`, sequence); }
      catch (error) { await installerPid(pid); throw error; }
      await installerPid(pid);
      if (screen === 'confirm-format' && formatTarget === 'esp') espFormatted = true;
      acted = true;
    }
  }
  if (!done) throw new StopRun(`Installer exceeded 12 minutes: ${JSON.stringify(lastTexts)}`);
  await capture('installer-28-copy-finished', pid);
}
async function stopGuest() {
  const s = await snapshot(); rememberProcesses();
  if (['stopped', 'failed'].includes(s.guest.state)) return;
  if (await page.getByRole('button', { name: '나중에 하기', exact: true }).count()) await click('나중에 하기');
  await rail('무대');
  const stop = page.getByRole('button', { name: '끄기', exact: true });
  if (await stop.count()) await stop.click(); else await click('취소');
  await until(async () => ['stopped', 'failed'].includes((await snapshot()).guest.state), 60000, 'guest stopped');
}
async function quitApp() {
  log('tray-quit', native(['-AppPid', String(app.pid), '-Quit']));
  await until(() => exited, 30000, 'app exit', 200);
}
async function guestScreenshot(name) {
  const shots = path.join(home, 'screenshots');
  const before = new Set(fs.existsSync(shots) ? fs.readdirSync(shots) : []);
  await click('스크린샷');
  const file = await until(() => fs.existsSync(shots) && fs.readdirSync(shots).find(n => n.endsWith('.png') && !before.has(n)), 15000, 'product screenshot');
  fs.copyFileSync(path.join(shots, file), path.join(output, name + '.png'));
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
    // The product starts through start-unelevated.ps1 with a UAC-style filtered token: the hosted
    // runner is an elevated administrator, and WebView2 150+ opens no remote-debugging endpoint for an
    // elevated host (runs 16 and 17: window up, CDP silent). The helper inherits this environment,
    // prints {pid} once the product runs and {exit} when it ends.
    app = { pid: null };
    const launcher = spawn('pwsh', ['-NoProfile', '-File', path.join(directory, 'start-unelevated.ps1'), '-FilePath', appPath, '-WorkingDirectory', host, '-OutputFile', path.join(output, 'app-output.txt')],
      { env: { ...process.env, OME_HOME: home, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` }, stdio: ['ignore', 'pipe', 'pipe'] });
    let pending = '', launcherErr = '';
    launcher.stdout.setEncoding('utf8'); launcher.stderr.setEncoding('utf8');
    launcher.stderr.on('data', chunk => { launcherErr += chunk; });
    launcher.stdout.on('data', chunk => {
      pending += chunk; let index;
      while ((index = pending.indexOf('\n')) >= 0) {
        const line = pending.slice(0, index).trim(); pending = pending.slice(index + 1);
        if (!line) continue;
        const message = JSON.parse(line);
        if (message.pid) { app.pid = message.pid; log('app-start', message); }
        if (message.exit !== undefined) { exited = true; log('app-exit', { pid: app.pid, code: message.exit }); }
      }
    });
    launcher.on('exit', code => { if (!app.pid) { exited = true; log('app-spawn-error', { code, error: launcherErr.trim() }); } });
    await until(() => app.pid, 30000, 'product pid from the launcher');
    rememberProcesses();
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
      await click('다운로드');
      s = await until(async () => {
        const s = await snapshot(); const d = s.wizard.download;
        if (['failed', 'cancelled'].includes(d?.stage)) throw new StopRun(`Artifact download ${d.stage}`);
        if (Date.now() >= nextProgress) { log('download-progress', { progress: d }); nextProgress = Date.now() + 30000; }
        return d?.stage === 'verified' && s;
      }, 30 * 60000, 'verified artifact', 5000);
      result.measurements.downloadMs = Date.now() - start; result.measurements.downloadBytes = s.wizard.download.doneBytes;
      result.measurements.downloadAverageBytesPerSecond = s.wizard.download.doneBytes / (result.measurements.downloadMs / 1000);
    }
    await capture('04-image-verified'); await click('다음'); await capture('05-disk-size');
  });
  await step('S1.5-installer', async () => {
    const start = Date.now();
    // Start the native watcher before the UI click; PowerShell/CIM startup otherwise misses
    // the ISO GRUB countdown and enters Live Android rather than Installation.
    const watcher = spawn('pwsh', ['-NoProfile', '-File', path.join(directory, 'native.ps1'), '-AppPid', String(app.pid), '-WaitInstaller', '-HomePath', home], { stdio: ['ignore', 'pipe', 'pipe'] });
    let watcherOut = '', watcherErr = '';
    watcher.stdout.setEncoding('utf8'); watcher.stderr.setEncoding('utf8');
    watcher.stdout.on('data', s => { watcherOut += s; }); watcher.stderr.on('data', s => { watcherErr += s; });
    const watched = new Promise(resolve => watcher.once('exit', code => resolve(code)));
    await delay(1800); await click('디스크 만들기'); let pid;
    try {
      if (await withTimeout(watched, 90000, 'installer watcher exit') !== 0) throw Error(`Installer watcher failed: ${watcherErr}`);
      const observed = JSON.parse(watcherOut); log('installer-watcher', observed); pid = observed.pid;
      rememberProcesses(); if (!ownedQemu.has(pid)) throw Error('Installer process identity not confirmed');
    }
    catch (error) {
      const stderr = fs.existsSync(path.join(home, 'logs')) ? fs.readdirSync(path.join(home, 'logs')).filter(n => /qemu.*stderr/.test(n)).map(n => fs.readFileSync(path.join(home, 'logs', n), 'utf8')).join('\n') : '';
      if (Date.now() - start <= 45000 && /(?:GL|EGL|OpenGL|virgl).*(?:error|fail)|(?:error|fail).*(?:GL|EGL|virgl)/i.test(stderr)) {
        gap('Installer GL startup failed; settings software rendering retry requested.');
        await click('나중에 하기'); await rail('설정'); await page.getByRole('switch', { name: '소프트웨어 렌더링', exact: true }).check();
        result.softwareRenderingRetry = true; await capture('software-rendering-settings');
        const resume = page.getByRole('button', { name: /설정 이어서|설치 계속|마법사/ });
        if (await resume.count() !== 1) throw Error('No visible wizard resume entry after enabling software rendering');
        await resume.click(); await click('다시 시작'); pid = await qemuWindow();
      } else throw error;
    }
    await installGuest(pid); result.measurements.installMs = Date.now() - start;
    await capture('06-install-complete'); await click('설치 완료');
  });
  await step('S1.6-first-boot', async () => {
    const start = Date.now();
    if (result.softwareRenderingRetry) {
      const pid = await qemuWindow();
      await keys(pid, '29-disk-vm-options', 'HOME,DOWN,DOWN,DOWN,DOWN,ENTER', null, 500);
      await keys(pid, '30-disk-boot', 'HOME,DOWN,ENTER', null, 3000);
    } else log('first-boot-default', { note: 'Normal virgl path: leave installed GRUB default unchanged; no keys sent.' });
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
    await page.getByRole('row').filter({ hasText: result.game.package }).getByRole('button', { name: '실행', exact: true }).click(); await rail('무대');
    // No account creation or unapproved game interaction. Fifteen minutes of product captures
    // document whether a consent/resource-download dialog blocks reaching the title screen.
    for (let minute = 0; minute <= 15; minute++) {
      if (minute) await delay(60000);
      await snapshot(); await guestScreenshot(`game-${String(minute).padStart(2, '0')}`);
    }
    result.measurements.gameLaunchObservationMs = Date.now() - start;
    result.game.visualReviewRequired = true; await capture('13-game-final-stage');
  });
  await step('stop', async () => { const start = Date.now(); await stopGuest(); result.measurements.stopMs = Date.now() - start; await capture('14-stopped'); });
  await step('quit', quitApp);
  // An unexamined title/download/login screenshot cannot certify a running game.
  result.outcome = result.game.visualReviewRequired ? 'needs-review' : 'passed';
} catch (error) {
  result.error = error.stack; log('failure', { error: error.stack });
  if (page && !page.isClosed() && !exited) await capture('failure').catch(e => log('capture-failed', { error: e.message }));
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
