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
async function capture(name, qemuPid, options = {}) {
  const shot = native(['-AppPid', String(app.pid), ...(qemuPid ? ['-QemuPid', String(qemuPid)] : []), '-Shot', path.join(output, 'shots', `${name}.png`), ...(options.highlight ? ['-Highlight'] : [])]);
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
// `excludePid` is the installer's QEMU: after 설치 완료 the product sends system_powerdown, the
// installer's userspace ignores it, and the supervisor kills the process at its 30 s deadline
// (ome-supervisor STOP_DEADLINE) before starting the installed disk. Until then the snapshot still
// names the old process, and the first boot's window is the one with a new PID.
async function qemuWindow(excludePid = null, timeout = 30000) {
  return until(async () => {
    const s = await snapshot(); rememberProcesses();
    if (!s.guest.pid || !ownedQemu.has(s.guest.pid) || s.guest.pid === excludePid) return false;
    native(['-AppPid', String(app.pid), '-QemuPid', String(s.guest.pid)]); return s.guest.pid;
  }, timeout, 'owned SDL_app');
}
let installerQemuPid = null;
// Installer and GRUB keys go the way a person's do: into the product window, whose webview forwards
// each press and release to QEMU over QMP (`input-send-event`). Before the guest reports its boot
// marker the ome-input gate passes every key through raw, profile bindings included, so this path is
// open for the whole installer and for the installed GRUB. The press and the release are separate
// QMP commands a few milliseconds apart, so the guest firmware never sees a key held long enough to
// repeat. The host-side SDL path (SendInput into the QEMU window) held each key about 180 ms and the
// runner's scheduling jitter stretched one DOWN past the firmware's 500 ms typematic threshold
// (CI run 27: one press moved the selection six rows). OME_DOD_SDL_KEYS=1 restores that path.
const BROWSER_KEYS = { HOME: 'Home', END: 'End', UP: 'ArrowUp', DOWN: 'ArrowDown', LEFT: 'ArrowLeft', RIGHT: 'ArrowRight', ENTER: 'Enter', TAB: 'Tab', ESC: 'Escape', BACKSPACE: 'Backspace', SPACE: 'Space' };
const TEXT_CODES = { ' ': 'Space', '.': 'Period', '-': 'Minus', '/': 'Slash', '_': 'Shift+Minus', '+': 'Shift+Equal', '=': 'Equal', ',': 'Comma' };
function textChord(character) {
  if (/^[a-z]$/.test(character)) return `Key${character.toUpperCase()}`;
  if (/^[A-Z]$/.test(character)) return `Shift+Key${character}`;
  if (/^[0-9]$/.test(character)) return `Digit${character}`;
  return TEXT_CODES[character] ?? null;
}
// keyboard.press() fires keydown and keyup within a millisecond; the product handles each
// forwarded event on its own blocking task, so the release could overtake the press (a press with
// no release, then the next press of that key suppressed as a repeat: CI runs 30 and 35 lost about
// one key in three). A person's press lasts tens of milliseconds, so the driver's does too.
async function pressSlow(chord) {
  const parts = chord.split('+');
  for (const part of parts) { await page.keyboard.down(part); await delay(40); }
  await delay(40);
  for (const part of [...parts].reverse()) { await page.keyboard.up(part); await delay(40); }
}
async function sdlKeys(pid, label, keySequence, text, waitMs) {
  log('installer-input', { label, path: 'sdl', ...native(['-AppPid', String(app.pid), '-QemuPid', String(pid), ...(keySequence ? ['-Keys', keySequence] : []), ...(text ? ['-Text', text] : [])]) });
  await delay(waitMs); await capture(`installer-${label}`, pid);
}
async function keys(pid, label, keySequence, text, waitMs = 800) {
  if (process.env.OME_DOD_SDL_KEYS === '1') return sdlKeys(pid, label, keySequence, text, waitMs);
  // The gate admits keys only while the main window is the foreground window.
  const focus = native(['-AppPid', String(app.pid), '-Foreground']);
  if (!focus.settled) log('foreground-unsettled', { label, ...focus });
  // CI run 30 lost the first key of two sequences ('5' of 512M, 'y' of yes) right after the focus
  // helper acted; the window activation is still settling in the product. A short pause first.
  await delay(300);
  const sent = [];
  for (const key of keySequence ? keySequence.split(',') : []) {
    // A named key from BROWSER_KEYS, or one character typed as a chord (capitals with Shift).
    const browser = BROWSER_KEYS[key] ?? (key.length === 1 ? textChord(key) : null);
    if (!browser) throw new StopRun(`Unknown key ${key}`);
    await pressSlow(browser); sent.push(browser); await delay(150);
  }
  for (const character of text ?? '') {
    // keyboard.type() marks a capital with the shift modifier flag only; the product forwards
    // event.code, so Shift must be its own press for cfdisk's capital W to arrive as W.
    const chord = textChord(character);
    if (!chord) throw new StopRun(`Unmapped character ${JSON.stringify(character)}`);
    await pressSlow(chord); sent.push(chord); await delay(120);
  }
  log('installer-input', { label, path: 'product', settled: focus.settled, foreground: focus.foreground, others: focus.others, sent });
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
  // Console OCR reads Confirm as "Cont Irm", Question as "Uuestion" and Would as "Uould" (CI run 39).
  if (/con[ft][il]rm/.test(words) && has('format')) return 'confirm-format';
  if (/[qu]uestion/.test(words) && (has('label') || has('customize') || has('drive name'))) return 'label-question';
  if (has('installing') || has('expect to write')) return 'installing';
  return 'unknown';
}
async function installerPid(pid) {
  const current = (await snapshot()).guest.pid;
  if (current !== pid) throw new StopRun(`Guest restarted during installer: expected QEMU ${pid}, snapshot guest.pid ${current}; refusing to follow a new process`);
}
// The Bliss GRUB theme prints "Enter: Boot Selected  E: Edit Selected  C: Grub Terminal" under every
// menu, on the ISO and on the installed disk alike. At 100 % display scaling the entries are too
// small for OCR (CI runs 25 and 26) while that footer still reads, so the footer is the menu signal;
// the entry the keys select is verified by the screen that follows. Every frame of the wait is kept.
const GRUB_MENU = /installation|edit\s*selected|boot\s*selected|grub\s*te/i;
async function waitGrubMenu(pid, prefix) {
  let attempt = 0;
  await until(async () => {
    const name = `${prefix}-${String(attempt++).padStart(2, '0')}`;
    await capture(name, pid);
    return GRUB_MENU.test(recognize(name));
  }, 60000, `${prefix} GRUB menu`, 100);
}
// Waits until the guest screen reads `pattern`, keeping every frame as `${label}-NN`; returns the text.
// Console OCR at this scale turns 2 into Z, v into u and W into U, so callers write tolerant patterns.
// OCR splits words at will ("Part it ion", "Fi lesystem"), so screen patterns are matched against
// the text with every space removed and lower-cased, and are written that way.
const squash = text => text.toLowerCase().replace(/\s+/g, '');
async function waitScreen(pid, label, pattern, timeoutMs = 60000) {
  let attempt = 0, last = '';
  await until(async () => {
    const name = `${label}-${String(attempt++).padStart(2, '0')}`;
    await capture(name, pid); last = squash(recognize(name));
    return pattern.test(last);
  }, timeoutMs, `${label} ${pattern}`, 100);
  return last;
}
// Sends keys and requires `pattern` on the screen within `settleMs`; sends the same keys again up to
// `attempts` times. Callers pick sequences that are safe to repeat on the screen they expect to leave.
// `retryWhen` names the screen the keys are meant to leave: the keys are sent again only while that
// screen is still showing, never into whatever else appeared (CI run 35 pressed Enter into the
// dialog after the one it meant, twice).
async function keysUntil(pid, label, keySequence, text, pattern, { settleMs = 10000, attempts = 3, waitMs = 800, retryWhen = null } = {}) {
  let last = '';
  const stillThere = seen => (typeof retryWhen === 'function' ? retryWhen(seen) : retryWhen.test(seen));
  for (let attempt = 1; attempt <= attempts; attempt++) {
    const name = attempt === 1 ? label : `${label}-retry${attempt}`;
    if (attempt > 1 && retryWhen) {
      const frame = `installer-${name}-before`;
      await capture(frame, pid); last = squash(recognize(frame));
      if (pattern.test(last)) return last;
      if (!stillThere(last)) {
        log('installer-retry-held', { label, attempt, seen: last.slice(0, 200) });
        try { return await waitScreen(pid, `installer-${name}-wait`, pattern, settleMs); }
        catch (error) { if (error instanceof StopRun) throw error; continue; }
      }
    }
    await keys(pid, name, keySequence, text, waitMs);
    try { return await waitScreen(pid, `installer-${name}-check`, pattern, settleMs); }
    catch (error) {
      if (error instanceof StopRun) throw error;
      log('installer-expect-miss', { label, attempt, pattern: String(pattern) });
    }
  }
  throw new StopRun(`Installer screen after ${label} never showed ${pattern}; last ${last.slice(0, 200)}`);
}
// Matched against squash()ed text. The console OCR reads 2 as z, 1 as i or l, v as u, w as u.
const CFDISK = {
  size512M: /size:?5[1il][2z]m/,
  row512M: /5[1il][2z]m/,
  efiSystem: /efisyste/,
  linuxFilesystem: /linuxfi?lesyste/,
  typeList: /linux(root|swap|home|server)|efisyste|biosboot/,
  table: /freespace|\[(quit|write|urite|type|delete)\]/,
  sizePrompt: /partitionsize/,
  labelType: /labeltype/,
  confirmTool: /cfdiskprogram|cgdisk/,
  partitionList: /choosepartition.*modify/,
  installerInfo: /don.?tkno[wu]whatthisis|documentationformore/,
  writeQuestion: /areyousure|type.?yes/,
  writeResult: /altered|synci|didnotwrite/,
  written: /altered|synci/,
  installerBack: /choosepartition|pleaseselect|restart(ing)?theinstal/,
};
// Moves the GRUB selection to `targetRow` one key at a time, reading the selection bar from each
// frame (native.ps1 -Highlight). A lost or repeated key is corrected by the next frame instead of
// trusted (CI runs 27 and 32 each booted the wrong entry after blind Down presses).
async function grubSelect(pid, prefix, targetRow, expectedRows) {
  let frame = 0, oddFrames = 0;
  for (let attempt = 0; attempt < 14; attempt++) {
    const shot = await capture(`${prefix}-${String(frame++).padStart(2, '0')}`, pid, { highlight: true });
    const highlight = shot.highlight;
    if (!highlight || highlight.row < 0) { await delay(500); continue; }
    if (highlight.entries.length !== expectedRows) {
      // A window over the guest hides rows (CI run 36: an adb console window); read again before
      // giving up, and never send a key on a partial reading.
      log('grub-rows-unexpected', { prefix, oddFrames, highlight });
      if (++oddFrames >= 4) throw new StopRun(`GRUB menu shows ${highlight.entries.length} rows, expected ${expectedRows}: ${JSON.stringify(highlight)}`);
      await delay(700); continue;
    }
    oddFrames = 0;
    if (highlight.row === targetRow) { log('grub-selected', { prefix, row: highlight.row, entries: highlight.entries }); return highlight; }
    const key = highlight.row < targetRow ? 'DOWN' : 'UP';
    await keys(pid, `${prefix}-${String(frame++).padStart(2, '0')}-${key.toLowerCase()}`, key, null, 600);
  }
  throw new StopRun(`GRUB selection did not reach row ${targetRow} within 14 frames`);
}
// Presses Enter on the verified selection and confirms it took: the menu is gone (an entry boots)
// or a submenu with `submenuRows` entries shows. A lost Enter is pressed again; a moved selection stops.
async function grubEnter(pid, label, selectedRow, submenuRows = null) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    const suffix = attempt > 1 ? `-retry${attempt}` : '';
    await keys(pid, `${label}${suffix}`, 'ENTER', null, 1500);
    const highlight = (await capture(`installer-${label}-after${suffix}`, pid, { highlight: true })).highlight;
    const menuGone = !highlight || highlight.row < 0 || highlight.entries.length === 0;
    if (submenuRows === null ? menuGone : highlight?.entries.length === submenuRows) { log('grub-entered', { label, attempt }); return; }
    if (!menuGone && highlight.row !== selectedRow) throw new StopRun(`GRUB selection is on row ${highlight.row}, not ${selectedRow}, before Enter took effect`);
    log('grub-enter-retry', { label, attempt, highlight });
  }
  throw new StopRun(`GRUB Enter on ${label} did not take effect after three presses`);
}
async function installGuest(pid) {
  // Move the webview focus off the create/completion button with a real noninteractive UI click.
  // While the stage is active the product forwards every key to the guest and prevents the default.
  await page.getByRole('heading', { name: '운영체제 설치', exact: true }).click();
  // docs/evidence/M0/guest-install.md 69-94. Never send these to an existing disk.
  // No keys before the menu is recognized: keys during the firmware phase reach the firmware now and
  // left one variable store booting slowly (docs/evidence/M2/embedded-display-freeze.md). The ISO GRUB
  // menu waits about 30 s, so recognition first, then one Home to reset its countdown.
  await waitGrubMenu(pid, 'installer-00-grub-menu');
  await snapshot();
  await keys(pid, '01a-grub-home', 'HOME');
  // ISO menu: Live, Live w/ FFMPEG, Live PC-Mode, Live PC-Mode w/ FFMPEG, Installation, VM Options,
  // Debugging, Advanced options (docs/evidence/M0/guest-install.md).
  await grubSelect(pid, 'installer-01a-grub-select', 4, 8);
  await grubEnter(pid, '01-grub-installation', 4);
  // The installer's first dialog took 30 s in runs 29 and 30 and longer in run 31: poll for it.
  // The installer first shows a message box ("UEFI System detected! Please select a partition as
  // an EFI System Partition ... OK"), then the Choose Partition list with Create/Modify partitions.
  // Only the list takes the c hotkey (CI run 35 sent it into the message box). The box takes Enter
  // when it has not gone on its own after fifteen seconds.
  {
    let frame = 0, infoSince = null;
    await until(async () => {
      const name = `installer-01-partition-dialog-${String(frame++).padStart(2, '0')}`;
      await capture(name, pid); const text = squash(recognize(name));
      if (CFDISK.partitionList.test(text)) return true;
      if (CFDISK.installerInfo.test(text)) {
        infoSince ??= Date.now();
        if (Date.now() - infoSince >= 15000) { await keys(pid, `01b-dismiss-info-${frame}`, 'ENTER', null, 500); infoSince = null; }
      }
      return false;
    }, 180000, 'installer Choose Partition list', 100);
  }
  // docs/evidence/M0/guest-install.md 69-94, each step verified on screen before the next.
  // `c` highlights Create/Modify partitions, Enter opens the cfdisk-or-cgdisk question.
  const tableOnly = seen => CFDISK.table.test(seen) && !CFDISK.sizePrompt.test(seen) && !CFDISK.typeList.test(seen) && !CFDISK.writeQuestion.test(seen);
  await keysUntil(pid, '02-create-modify', 'c,ENTER', null, CFDISK.confirmTool, { retryWhen: seen => CFDISK.partitionList.test(seen) && !CFDISK.confirmTool.test(seen), settleMs: 15000 });
  await keysUntil(pid, '03-continue-cfdisk', 'ENTER', null, CFDISK.labelType, { retryWhen: CFDISK.confirmTool });
  await keysUntil(pid, '04-gpt', 'ENTER', null, CFDISK.table, { retryWhen: CFDISK.labelType });
  await keysUntil(pid, '05-new-esp', 'n', null, CFDISK.sizePrompt, { retryWhen: tableOnly });
  // The size field is prefilled with the free size; clear it, then type. A lost character shows on
  // the prompt line, and the retry clears and types again.
  await keysUntil(pid, '06-esp-size', `${Array(8).fill('BACKSPACE').join(',')},5,1,2,M`, null, CFDISK.size512M, { settleMs: 6000, retryWhen: CFDISK.sizePrompt });
  await keysUntil(pid, '07-create-esp', 'ENTER', null, CFDISK.row512M, { retryWhen: CFDISK.sizePrompt });
  // The type list opens on Linux filesystem; Home goes to its first entry. CI run 38 read the list
  // as EFI System, MBR partition scheme, Intel Fast Flash, ... (run 30 had landed on MBR partition
  // scheme with Home, Enter), so the list text decides how many Down presses EFI System needs.
  for (let attempt = 1; ; attempt++) {
    const list = await keysUntil(pid, `08-type-list${attempt > 1 ? `-retry${attempt}` : ''}`, 't', null, CFDISK.typeList, { retryWhen: tableOnly });
    const efiAt = list.indexOf('efisyste'), mbrAt = list.indexOf('mbrpartitionscheme');
    const efiFirst = efiAt >= 0 && (mbrAt < 0 || efiAt < mbrAt);
    const sequence = (efiFirst === (attempt % 2 === 1)) ? 'HOME,ENTER' : 'HOME,DOWN,ENTER';
    log('installer-type-order', { attempt, efiAt, mbrAt, sequence });
    const table = await keysUntil(pid, `09-efi-type${attempt > 1 ? `-retry${attempt}` : ''}`, sequence, null, CFDISK.table, { retryWhen: CFDISK.typeList });
    if (CFDISK.efiSystem.test(table)) break;
    if (attempt >= 3) throw new StopRun(`Partition 1 type is not EFI System after three tries: ${table.slice(0, 300)}`);
    log('installer-type-retry', { attempt, table: table.slice(0, 300) });
  }
  // Down onto the free-space row (Down again stays there), New, default size = the rest.
  await keysUntil(pid, '10-new-system', 'DOWN,n', null, CFDISK.sizePrompt, { retryWhen: tableOnly });
  const tableBeforeWrite = await keysUntil(pid, '11-system-size', 'ENTER', null, CFDISK.linuxFilesystem, { retryWhen: CFDISK.sizePrompt });
  if (!CFDISK.efiSystem.test(tableBeforeWrite)) throw new StopRun(`Table lost EFI System before write: ${tableBeforeWrite.slice(0, 300)}`);
  // Write needs a capital W and the literal word yes; the confirmation shows what was typed, and a
  // refused write reads "Did not write partition table to disk".
  let written = '';
  for (let attempt = 1; attempt <= 3 && !CFDISK.written.test(written); attempt++) {
    await keysUntil(pid, `12-write${attempt > 1 ? `-retry${attempt}` : ''}`, 'W', null, CFDISK.writeQuestion, { retryWhen: tableOnly });
    await keys(pid, `13-write-yes${attempt > 1 ? `-retry${attempt}` : ''}`, 'y,e,s,ENTER', null, 1500);
    written = await waitScreen(pid, `installer-13-write-result${attempt > 1 ? `-retry${attempt}` : ''}`, CFDISK.writeResult, 15000);
    if (!CFDISK.written.test(written)) log('installer-write-refused', { attempt, seen: written.slice(0, 200) });
  }
  if (!CFDISK.written.test(written)) throw new StopRun('cfdisk did not write the partition table after three tries');
  // Quit returns to the installer, which restarts itself and lists the new partitions.
  await keysUntil(pid, '14-quit-cfdisk', 'q', null, CFDISK.installerBack, { settleMs: 30000, waitMs: 4000, retryWhen: tableOnly });
  const deadline = Date.now() + 12 * 60000;
  let espFormatted = false, formatTarget, previousScreen, acted = false, unknownSince, actedAt, attempts = 0;
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
    if (screen !== previousScreen) { acted = false; attempts = 0; }
    // A key the product's gate dropped (the main window lost the foreground for a moment) leaves the
    // screen unchanged; one more attempt after 20 s, never a third, keeps a slow screen from doubling up.
    else if (acted && attempts < 2 && Date.now() - actedAt >= 20000 && !['installing', 'warning-countdown', 'done', 'unknown'].includes(screen)) {
      log('installer-retry', { screen, attempts }); acted = false;
    }
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
      acted = true; actedAt = Date.now(); attempts++;
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
    await installGuest(pid); result.measurements.installMs = Date.now() - start; installerQemuPid = pid;
    await capture('06-install-complete'); await click('설치 완료');
  });
  await step('S1.6-first-boot', async () => {
    const start = Date.now();
    // The product itself picks software rendering on a host without OpenGL 2.0 (GDI Generic); the
    // installed GRUB default then hangs in early userspace under std VGA, so the first boot needs the
    // ISO authors' "No HW Acceleration" entry (docs/evidence/M0/guest-install.md, attempt 4 and 5).
    const softwareRendering = result.softwareRenderingRetry || (await snapshot()).settings.gpuMode === 'software';
    result.measurements.softwareRendering = softwareRendering;
    if (softwareRendering) {
      const pid = await qemuWindow(installerQemuPid, 120000);
      // Keys before the menu would reach the firmware (docs/evidence/M2/embedded-display-freeze.md).
      await waitGrubMenu(pid, 'first-boot-grub-menu');
      // Installed menu: four Bliss entries, VM Options, Debugging, Advanced options, BlissOS at
      // hd0,gpt1; VM Options holds Virgl, No HW Acceleration and their debug variants.
      await keys(pid, '29a-grub-home', 'HOME');
      await grubSelect(pid, 'first-boot-grub-select', 4, 8);
      await grubEnter(pid, '29-disk-vm-options', 4, 4);
      await grubSelect(pid, 'first-boot-vm-select', 1, 4);
      await grubEnter(pid, '30-disk-boot', 1);
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
