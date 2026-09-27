// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { describe, expect, it } from 'vitest';
import {
  applyDrag, chipReach, crowded, floatBeside, joystickRadiusPx, markerExtent, placeStyle, pointExtent, radiusFromPixels, toLogical,
  toPixels, translateSwipe, wheelDistancePx,
} from './geometry';

const screen1280 = { width: 1280, height: 800 };

describe('overlay geometry: the viewport is the guest screen', () => {
  it('places a logical point at x*100% and y*100% of the viewport', () => {
    expect(placeStyle({ x: 0.424, y: 0.705 })).toEqual({ left: '42.4%', top: '70.5%' });
    expect(placeStyle({ x: 0, y: 1 })).toEqual({ left: '0%', top: '100%' });
    expect(placeStyle({ x: 1 / 3, y: 0.16 })).toEqual({ left: '33.333%', top: '16%' });
    expect(toPixels({ x: 0.25, y: 0.5 }, screen1280)).toEqual({ x: 320, y: 400 });
  });

  it('measures a joystick radius on the shorter side and a wheel distance on the height', () => {
    expect(joystickRadiusPx(0.15, screen1280)).toBe(120);
    expect(joystickRadiusPx(0.15, { width: 720, height: 1280 })).toBe(108);
    expect(wheelDistancePx(0.2, screen1280)).toBe(160);
    expect(wheelDistancePx(0.2, { width: 720, height: 1280 })).toBe(256);
  });

  it('turns pixels back into logical points, kept on the screen and rounded to 0.001', () => {
    expect(toLogical({ x: 320, y: 400 }, screen1280)).toEqual({ x: 0.25, y: 0.5 });
    expect(toLogical({ x: -40, y: 900 }, screen1280)).toEqual({ x: 0, y: 1 });
    expect(toLogical({ x: 1280 / 3, y: 100 }, screen1280)).toEqual({ x: 0.333, y: 0.125 });
    expect(radiusFromPixels(120, screen1280)).toBe(0.15);
    expect(radiusFromPixels(5, screen1280)).toBe(0.03);
    expect(radiusFromPixels(2000, screen1280)).toBe(0.5);
  });

  it('keeps markers 40px apart', () => {
    expect(crowded({ x: 0.5, y: 0.5 }, [{ x: 0.5 + 39 / 1280, y: 0.5 }], screen1280)).toBe(true);
    expect(crowded({ x: 0.5, y: 0.5 }, [{ x: 0.5 + 41 / 1280, y: 0.5 }], screen1280)).toBe(false);
    expect(crowded({ x: 0.5, y: 0.5 }, [], screen1280)).toBe(false);
  });

  it('drags a marker, a whole swipe, a stick radius and a swipe end', () => {
    const at = { x: 0.2, y: 0.3 };
    expect(applyDrag({ kind: 'tap', at, hold: true }, 'marker', { x: 0.6, y: 0.7 }, screen1280))
      .toEqual({ kind: 'tap', at: { x: 0.6, y: 0.7 }, hold: true });
    expect(applyDrag({ kind: 'mouseTap', at: null }, 'marker', { x: 0.6, y: 0.7 }, screen1280))
      .toEqual({ kind: 'mouseTap', at: null });
    expect(applyDrag({ kind: 'joystick', center: { x: 0.5, y: 0.5 }, radius: 0.1 }, 'radius', { x: 0.5 + 200 / 1280, y: 0.5 }, screen1280))
      .toEqual({ kind: 'joystick', center: { x: 0.5, y: 0.5 }, radius: 0.25 });
    const swipe = { kind: 'swipe', from: { x: 0.5, y: 0.5 }, to: { x: 0.8, y: 0.5 }, durationMs: 200 } as const;
    expect(applyDrag(swipe, 'marker', { x: 0.1, y: 0.2 }, screen1280))
      .toEqual({ ...swipe, from: { x: 0.1, y: 0.2 }, to: { x: 0.4, y: 0.2 } });
    expect(applyDrag(swipe, 'end', { x: 0.5, y: 0.9 }, screen1280)).toEqual({ ...swipe, to: { x: 0.5, y: 0.9 } });
  });

  it('keeps both ends of a moved swipe on the screen', () => {
    expect(translateSwipe({ x: 0.5, y: 0.5 }, { x: 0.8, y: 0.5 }, { x: 0.9, y: 0.5 }))
      .toEqual({ from: { x: 0.7, y: 0.5 }, to: { x: 1, y: 0.5 } });
  });

  it('floats a card to the right of its marker, or to the left near the right edge', () => {
    expect(floatBeside(pointExtent({ x: 100 / 1280, y: 100 / 800 }, screen1280), 312, screen1280)).toEqual({ left: '132px', top: '76px' });
    expect(floatBeside(pointExtent({ x: 1200 / 1280, y: 700 / 800 }, screen1280), 312, screen1280)).toEqual({ right: '112px', bottom: '76px' });
  });

  it("keeps a card clear of the swipe it belongs to and of the stick's circle", () => {
    // Swipe from x 717 to 922 at 1280 wide: its duration chip reaches 986, so the card goes left of 697.
    const swipe = markerExtent({ kind: 'swipe', from: { x: 0.56, y: 0.8 }, to: { x: 0.72, y: 0.8 }, durationMs: 240 }, screen1280);
    expect(swipe?.left).toBeCloseTo(696.8);
    expect(swipe?.right).toBeCloseTo(985.6);
    if (swipe === null) throw new Error('A swipe has an extent.');
    expect(floatBeside(swipe, 312, screen1280)).toEqual({ right: '595.2px', bottom: '136px' });
    // A stick of radius 0.16 around x 205: the card starts right of the circle, past its radius handle.
    const stick = markerExtent({ kind: 'joystick', center: { x: 0.16, y: 0.68 }, radius: 0.16 }, screen1280);
    expect(stick?.left).toBeCloseTo(76.8);
    expect(stick?.right).toBeCloseTo(332.8);
    if (stick === null) throw new Error('A stick has an extent.');
    expect(floatBeside(stick, 312, screen1280)).toEqual({ left: '344.8px', bottom: '232px' });
    expect(markerExtent({ kind: 'mouseTap', at: null }, screen1280)).toBeNull();
    // A wider chip reaches further: `Space` about 31px either side, a mouse button's words about 57px.
    expect(chipReach('Q')).toBe(20);
    expect(chipReach('Space')).toBeCloseTo(30.75);
    expect(chipReach('오른쪽 버튼', true)).toBeCloseTo(57.25);
  });
});
