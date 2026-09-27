// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Number and time formatting for screens. Units are separated by a space (DESIGN.md 9).

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

function trimDigits(value: number): string {
  if (value >= 100) return value.toFixed(0);
  if (value >= 10) return value.toFixed(1);
  return value.toFixed(2);
}

/** Index into B, KB, MB, GB, TB that `formatBytes` would pick for this amount. */
export function byteUnit(bytes: number): number {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit;
}

/** Bytes in a fixed unit, so amounts of one transfer read alike: `0.84 GB` next to `2.26 GB`. */
export function formatBytesIn(bytes: number, unit: number): string {
  const index = Math.min(Math.max(0, Math.floor(unit)), UNITS.length - 1);
  const value = Math.max(0, bytes) / 1024 ** index;
  return index === 0 ? `${String(Math.round(value))} B` : `${trimDigits(value)} ${UNITS[index] ?? 'B'}`;
}

/** Bytes in binary multiples with Windows labels: `2.26 GB`, `412 GB`, `640 KB`. */
export function formatBytes(bytes: number): string {
  return formatBytesIn(bytes, byteUnit(bytes));
}

/** Transfer rate: `12.4 MB/s`. */
export function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

/** Whole GiB as the product writes disk sizes: `32 GB`. */
export function formatGib(gib: number): string {
  return `${String(gib)} GB`;
}

/** Remaining time: `1분 10초`, `2시간 5분`, `40초`. */
export function formatDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  if (hours > 0) return minutes > 0 ? `${String(hours)}시간 ${String(minutes)}분` : `${String(hours)}시간`;
  if (minutes > 0) return rest > 0 ? `${String(minutes)}분 ${String(rest)}초` : `${String(minutes)}분`;
  return `${String(rest)}초`;
}

function pad(value: number): string {
  return String(value).padStart(2, '0');
}

/** Local date and minute: `2026-09-26 19:52`. Unparseable input is shown as given. */
export function formatTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return `${String(date.getFullYear())}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Whole percent of a ratio: `63%`. */
export function formatPercent(ratio: number): string {
  return `${String(Math.floor(Math.min(1, Math.max(0, ratio)) * 100))}%`;
}
