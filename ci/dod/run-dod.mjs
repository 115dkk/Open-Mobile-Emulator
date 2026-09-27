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
const result = { startedAt: new Date().toISOString(), outcome: 'failed', steps: [], gaps: [], measurements: {}, probeItems: [], softwareRenderingRetry: false };
const writeJson = (name, value) => fs.writeFileSync(path.join(output, name), scrub(JSON.stringify(value, null, 2)) + '\n');
const log = (event, data = {}) => { const line = scrub(JSON.stringify({ at: new Date().toISOString(), event, ...data })); console.log(line); fs.appendFileSync(path.join(output, 'driver-output.txt'), line + '\n'); };
const native = args => {
  const r = spawnSync('pwsh', ['-NoProfile', '-File', path.join(directory, 'native.ps1'), ...args], { encoding: 'utf8', timeout: 45000 });
  if (r.status !== 0) throw Error(`native helper (${r.status}): ${r.stdout} ${r.stderr}`);
  return JSON.parse(r.stdout.trim());
};
const inventory = () => native(['-Inventory', '-HomePath', home]);
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
let app, page, browser, exited = false, appIdentity, homeOwned = false;
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
async function snapshot() {
  const s = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('app_snapshot'));
  log('snapshot', { snapshot: s }); return s;
}
async function capture(name, qemuPid) {
  log('capture', { name, ...native(['-AppPid', String(app.pid), ...(qemuPid ? ['-QemuPid', String(qemuPid)] : []), '-Shot', path.join(output, 'shots', `${name}.png`)]) });
  fs.writeFileSync(path.join(output, `${name}.txt`), scrub(await page.locator('body').innerText()));
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
async function installGuest(pid) {
  // docs/evidence/M0/guest-install.md 69-94. Never send these to an existing disk.
  await keys(pid, '01-grub-installation', 'HOME,DOWN,DOWN,DOWN,DOWN,ENTER', null, 30000);
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
  await keys(pid, '18-select-esp', 'HOME,ENTER');
  await keys(pid, '19-esp-fat32', 'DOWN,ENTER');
  await keys(pid, '20-esp-label', 'ENTER');
  await keys(pid, '21-esp-confirm', 'LEFT,ENTER', null, 4000);
  await keys(pid, '22-select-system', 'HOME,DOWN,ENTER');
  await keys(pid, '23-system-ext4', 'DOWN,ENTER');
  await keys(pid, '24-system-label', 'ENTER');
  await keys(pid, '25-system-confirm', 'LEFT,ENTER', null, 5000);
  await keys(pid, '26-no-ota', 'RIGHT,ENTER');
  await keys(pid, '27-efi-grub2', 'HOME,ENTER', null, 120000);
  // M0 saw no system-rw prompt. No Run/Reboot key: product 설치 완료 stops the installer.
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
try {
  await step('preflight', async () => {
    const pre = inventory(); log('preflight', pre); result.environment = pre;
    if (pre.processes.length) throw Error('Existing QEMU or ome.exe; no process was touched');
    const realHome = path.resolve(process.env.LOCALAPPDATA, 'OpenMobileEmulator').toLowerCase(); const h = home.toLowerCase();
    if (h === realHome || h.startsWith(realHome + path.sep) || realHome.startsWith(h + path.sep)) throw Error('Refusing developer product home or its ancestors/descendants');
    if (output === home || output.startsWith(home + path.sep)) throw Error('Evidence must be outside fresh home');
    if (fs.existsSync(home) && fs.readdirSync(home).length) throw Error('OME_HOME must be empty');
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
  await step('launch', async () => {
    const fd = fs.openSync(path.join(output, 'app-output.txt'), 'w');
    app = spawn(appPath, [], { cwd: host, env: { ...process.env, OME_HOME: home, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` }, stdio: ['ignore', fd, fd] }); fs.closeSync(fd);
    app.on('exit', (code, signal) => { exited = true; log('app-exit', { pid: app.pid, code, signal }); });
    app.on('error', error => { exited = true; log('app-spawn-error', { error: error.message }); });
    log('app-start', { pid: app.pid }); rememberProcesses();
    await until(async () => (await fetch(`http://127.0.0.1:${port}/json/version`)).ok, 30000, 'CDP');
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
    const start = Date.now(); await click('다운로드'); let nextProgress = 0;
    const s = await until(async () => {
      const s = await snapshot(); const d = s.wizard.download;
      if (['failed', 'cancelled'].includes(d?.stage)) throw new StopRun(`Artifact download ${d.stage}`);
      if (Date.now() >= nextProgress) { log('download-progress', { progress: d }); nextProgress = Date.now() + 30000; }
      return d?.stage === 'verified' && s;
    }, 30 * 60000, 'verified artifact', 5000);
    result.measurements.downloadMs = Date.now() - start; result.measurements.downloadBytes = s.wizard.download.doneBytes;
    result.measurements.downloadAverageBytesPerSecond = s.wizard.download.doneBytes / (result.measurements.downloadMs / 1000);
    await capture('04-image-verified'); await click('다음'); await capture('05-disk-size');
  });
  await step('S1.5-installer', async () => {
    const start = Date.now(); await click('디스크 만들기'); let pid;
    try { pid = await qemuWindow(); }
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
    const start = Date.now(); const pid = await qemuWindow();
    await keys(pid, '29-disk-vm-options', 'HOME,DOWN,DOWN,DOWN,DOWN,ENTER', null, 500);
    await keys(pid, '30-disk-boot', result.softwareRenderingRetry ? 'HOME,DOWN,ENTER' : 'HOME,ENTER', null, 3000);
    const s = await until(async () => { const s = await snapshot(); return s.guest.bootCompleted && s.guest.capabilities.items.some(i => i.state !== 'unknown') && s; }, 10 * 60000, 'first boot and probe', 3000);
    result.measurements.firstBootMs = Date.now() - start; result.probeItems = s.guest.capabilities.items;
    await capture('07-first-boot-probe'); log('applied-defaults', { text: await page.locator('body').innerText() }); await click('다음');
  });
  await step('S1.7-game-install', async () => {
    await capture('08-app-install'); const start = Date.now();
    gap('S1.7 opens the native file picker with 파일 고르기, not 설치.');
    await click('파일 고르기');
    native(['-AppPid', String(app.pid), '-FilePath', gameDirectory, '-GameSelection']);
    log('file-dialog', { closed: true, selection: 'four private split APKs' });
    const s = await until(async () => { const s = await snapshot(); return s.apps.items.some(a => a.package === result.game.package) && s; }, 300000, 'game installed');
    result.game.installedVersion = s.apps.items.find(a => a.package === result.game.package).versionName;
    result.measurements.gameInstallMs = Date.now() - start; await capture('09-game-installed');
    gap('S1.7 has no 실행 or 스크린샷. Complete wizard, launch from 앱 and capture from 무대.');
    await click('완료'); await rail('앱');
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
        for (const entry of fs.readdirSync(logs, { withFileTypes: true })) if (entry.isFile()) fs.writeFileSync(path.join(output, 'logs', entry.name.replace(/\.[^.]+$/, '') + '.txt'), scrub(fs.readFileSync(path.join(logs, entry.name), 'utf8')));
      }
      result.temporaryHome = home;
    }
  }).catch(error => { result.outcome = 'failed'; result.cleanupError = error.stack; });
  if (result.forcedCleanup) result.outcome = 'failed';
  for (const name of plannedSteps) if (!result.steps.some(s => s.name === name)) result.steps.push({ name, start: null, end: null, durationMs: null, outcome: 'not-run', reason: 'An earlier step failed; no bypass attempted' });
  result.endedAt = new Date().toISOString(); writeJson('dod-result.json', result);
  const appOutput = path.join(output, 'app-output.txt'); if (fs.existsSync(appOutput)) fs.writeFileSync(appOutput, scrub(fs.readFileSync(appOutput, 'utf8')));
  if (browser) await browser.close().catch(() => {});
}
process.exitCode = result.outcome === 'passed' ? 0 : 1;
