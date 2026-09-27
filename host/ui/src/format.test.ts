// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { describe, expect, it } from 'vitest';
import { byteUnit, formatBytes, formatBytesIn, formatDuration, formatPercent, formatRate, formatTimestamp } from './format';

const GIB = 1024 ** 3;

describe('formatting', () => {
  it('writes sizes with a space before the unit', () => {
    expect(formatBytes(2.26 * GIB)).toBe('2.26 GB');
    expect(formatBytes(412 * GIB)).toBe('412 GB');
    expect(formatBytes(38.2 * GIB)).toBe('38.2 GB');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatRate(12.4 * 1024 ** 2)).toBe('12.4 MB/s');
  });

  it('keeps one transfer in one unit', () => {
    expect(formatBytesIn(0.84 * GIB, byteUnit(2.26 * GIB))).toBe('0.84 GB');
  });

  it('writes remaining time in hours, minutes and seconds', () => {
    expect(formatDuration(70)).toBe('1분 10초');
    expect(formatDuration(40)).toBe('40초');
    expect(formatDuration(7500)).toBe('2시간 5분');
  });

  it('floors percentages so 100% means done', () => {
    expect(formatPercent(0.999)).toBe('99%');
    expect(formatPercent(1)).toBe('100%');
  });

  it('shows local minutes and leaves unknown text alone', () => {
    expect(formatTimestamp('2026-09-26T19:52:00')).toBe('2026-09-26 19:52');
    expect(formatTimestamp('어제')).toBe('어제');
  });
});
