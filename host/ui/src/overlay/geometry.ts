// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The overlay's viewport is the guest screen (ARCHITECTURE.md 3.17): a logical point (0..1 on both
// axes) is a share of the viewport, a joystick radius a share of its shorter side, a wheel swipe
// distance a share of its height. Pure functions; the page measures its own size and nothing else.
import type { CSSProperties } from 'react';
import type { BindingAction, LogicalPoint, Size } from '../contracts';

export interface PixelPoint { readonly x: number; readonly y: number }

/** Markers closer than this collide: their 40px hit areas would overlap (DESIGN.md 4, 8). */
export const MIN_MARKER_GAP_PX = 40;
/** A press that moves less than this is a click, not a drag. */
export const DRAG_THRESHOLD_PX = 4;
/** A swipe drawn shorter than this is not a swipe. */
export const MIN_SWIPE_PX = 16;
export const MIN_JOYSTICK_RADIUS_PX = 24;
/** Half the shorter side: the stick's circle spans the whole screen height of a landscape display. */
export const MAX_JOYSTICK_RADIUS = 0.5;

function round3(value: number): number {
  return Math.round(value * 1000) / 1000;
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

/** A share as a CSS percentage with at most three decimals: 0.424 is `42.4%`. */
export function percent(share: number): string {
  return `${String(Math.round(share * 100_000) / 1000)}%`;
}

/** `left: x*100%; top: y*100%` of the viewport. */
export function placeStyle(point: LogicalPoint): CSSProperties {
  return { left: percent(point.x), top: percent(point.y) };
}

export function toPixels(point: LogicalPoint, size: Size): PixelPoint {
  return { x: point.x * size.width, y: point.y * size.height };
}

export function shortSide(size: Size): number {
  return Math.min(size.width, size.height);
}

/** A joystick radius is a share of the viewport's shorter side (ome-input). */
export function joystickRadiusPx(radius: number, size: Size): number {
  return radius * shortSide(size);
}

/** A wheel swipe distance is a logical length along the vertical axis. */
export function wheelDistancePx(distance: number, size: Size): number {
  return distance * size.height;
}

/** A pixel offset inside the viewport as a logical point, kept on the screen and rounded to 0.001. */
export function toLogical(pixel: PixelPoint, size: Size): LogicalPoint {
  if (size.width <= 0 || size.height <= 0) return { x: 0.5, y: 0.5 };
  return { x: round3(clamp(pixel.x / size.width, 0, 1)), y: round3(clamp(pixel.y / size.height, 0, 1)) };
}

/** A radius drawn in pixels as a share of the shorter side, never smaller than the stick's chip. */
export function radiusFromPixels(pixels: number, size: Size): number {
  const side = shortSide(size);
  if (side <= 0) return MAX_JOYSTICK_RADIUS;
  return round3(clamp(Math.max(pixels, MIN_JOYSTICK_RADIUS_PX) / side, 0.001, MAX_JOYSTICK_RADIUS));
}

export function pixelDistance(a: LogicalPoint, b: LogicalPoint, size: Size): number {
  return Math.hypot((a.x - b.x) * size.width, (a.y - b.y) * size.height);
}

/** Where a binding's marker sits: tap point, stick center, swipe start, fixed mouse or wheel point. */
export function markerAnchor(action: BindingAction): LogicalPoint | null {
  switch (action.kind) {
    case 'tap': return action.at;
    case 'joystick': return action.center;
    case 'swipe': return action.from;
    case 'mouseTap': return action.at;
    case 'wheelSwipe': return action.at;
    case 'passThrough': return null;
  }
}

/** True when `point` would sit closer than the minimum gap to any of `others`. */
export function crowded(point: LogicalPoint, others: readonly LogicalPoint[], size: Size): boolean {
  return others.some((other) => pixelDistance(point, other, size) < MIN_MARKER_GAP_PX);
}

/** Moves a whole swipe so its start lands on `start`, keeping both ends on the screen. */
export function translateSwipe(from: LogicalPoint, to: LogicalPoint, start: LogicalPoint): { from: LogicalPoint; to: LogicalPoint } {
  const dx = clamp(start.x - from.x, -Math.min(from.x, to.x), 1 - Math.max(from.x, to.x));
  const dy = clamp(start.y - from.y, -Math.min(from.y, to.y), 1 - Math.max(from.y, to.y));
  return {
    from: { x: round3(from.x + dx), y: round3(from.y + dy) },
    to: { x: round3(to.x + dx), y: round3(to.y + dy) },
  };
}

/** Which part of a marker a drag holds. */
export type DragPart = 'marker' | 'radius' | 'end';

/**
 * The action after dragging `part` to `point`: the marker moves the binding (a swipe moves whole),
 * the radius handle sets the stick's radius from its distance to the center, the end handle moves
 * the swipe's end.
 */
export function applyDrag(action: BindingAction, part: DragPart, point: LogicalPoint, size: Size): BindingAction {
  switch (part) {
    case 'marker': return moveMarker(action, point);
    case 'radius':
      return action.kind === 'joystick'
        ? { ...action, radius: radiusFromPixels(pixelDistance(action.center, point, size), size) }
        : action;
    case 'end':
      return action.kind === 'swipe' ? { ...action, to: point } : action;
  }
}

function moveMarker(action: BindingAction, point: LogicalPoint): BindingAction {
  switch (action.kind) {
    case 'tap': return { ...action, at: point };
    case 'joystick': return { ...action, center: point };
    case 'swipe': return { ...action, ...translateSwipe(action.from, action.to, point) };
    case 'mouseTap': return action.at === null ? action : { ...action, at: point };
    case 'wheelSwipe': return { ...action, at: point };
    case 'passThrough': return action;
  }
}

/** The two wings of an arrow head at `tip`, pointing away from `tail`, as SVG polyline points. */
export function arrowHead(tail: PixelPoint, tip: PixelPoint, length = 10): string {
  const angle = Math.atan2(tip.y - tail.y, tip.x - tail.x);
  const wing = (turn: number) => {
    const x = tip.x - length * Math.cos(angle + turn);
    const y = tip.y - length * Math.sin(angle + turn);
    return `${String(round3(x))},${String(round3(y))}`;
  };
  return `${wing(0.5)} ${String(round3(tip.x))},${String(round3(tip.y))} ${wing(-0.5)}`;
}

/** Center of the duration chip, just past the arrow tip along the swipe. */
export function pastTip(tail: PixelPoint, tip: PixelPoint): PixelPoint {
  const length = Math.hypot(tip.x - tail.x, tip.y - tail.y);
  if (length === 0) return { x: tip.x, y: tip.y + 24 };
  const ux = (tip.x - tail.x) / length;
  const uy = (tip.y - tail.y) / length;
  const reach = Math.abs(ux) * 40 + Math.abs(uy) * 22;
  return { x: tip.x + ux * reach, y: tip.y + uy * reach };
}

function px(value: number): string {
  return `${String(Math.round(value * 10) / 10)}px`;
}

/** How far a marker's drawing reaches left and right of it, and the height to hang a card from. */
export interface Extent {
  readonly left: number;
  readonly right: number;
  readonly y: number;
}

/** Half the width of a one-letter key chip with its hit area. */
const KEY_CHIP_REACH = 20;
/** The duration chip past a swipe's arrow tip. */
const DURATION_REACH = 64;

/**
 * Half the width of a marker chip with its hit area, estimated from its words (13px labels: about
 * 7.5px a Latin letter, 13px a Hangul syllable) so a card can stand clear of it without measuring.
 */
export function chipReach(text: string, icon = false): number {
  let width = 16 + (icon ? 18 : 0);
  for (const char of text) width += char.charCodeAt(0) < 0x80 ? 7.5 : 13;
  return Math.max(KEY_CHIP_REACH, width / 2 + 4);
}

/**
 * The horizontal span a binding draws: its chip (`reach` either side), the stick's circle, or the
 * swipe from its start chip to its duration chip.
 */
export function markerExtent(action: BindingAction, size: Size, reach = KEY_CHIP_REACH): Extent | null {
  switch (action.kind) {
    case 'tap': {
      const at = toPixels(action.at, size);
      return { left: at.x - reach, right: at.x + reach, y: at.y };
    }
    case 'joystick': {
      const center = toPixels(action.center, size);
      const radius = Math.max(joystickRadiusPx(action.radius, size), 36);
      return { left: center.x - radius, right: center.x + radius, y: center.y };
    }
    case 'swipe': {
      const from = toPixels(action.from, size);
      const to = toPixels(action.to, size);
      return {
        left: Math.min(from.x - reach, to.x - (to.x < from.x ? DURATION_REACH : 0)),
        right: Math.max(from.x + reach, to.x + (to.x >= from.x ? DURATION_REACH : 0)),
        y: from.y,
      };
    }
    case 'mouseTap':
    case 'wheelSwipe': {
      if (action.at === null) return null;
      const at = toPixels(action.at, size);
      return { left: at.x - reach, right: at.x + reach, y: at.y };
    }
    case 'passThrough':
      return null;
  }
}

/** A point's own span, for a card beside a place that has no marker yet. */
export function pointExtent(point: LogicalPoint, size: Size): Extent {
  const at = toPixels(point, size);
  return { left: at.x - KEY_CHIP_REACH, right: at.x + KEY_CHIP_REACH, y: at.y };
}

/**
 * A floating box beside a marker's drawing: right of it, or left of it when `width` would leave the
 * viewport; hanging down from the marker in the upper half and standing up from it in the lower half,
 * so no height has to be measured and the box never covers the marker it belongs to.
 */
export function floatBeside(extent: Extent, width: number, viewport: Size, gap = 12, margin = 8): CSSProperties {
  const toRight = extent.right + gap + width <= viewport.width - margin;
  const horizontal: CSSProperties = toRight
    ? { left: px(Math.max(margin, extent.right + gap)) }
    : { right: px(Math.max(margin, viewport.width - extent.left + gap)) };
  const vertical: CSSProperties = extent.y <= viewport.height / 2
    ? { top: px(Math.max(margin, extent.y - 24)) }
    : { bottom: px(Math.max(margin, viewport.height - extent.y - 24)) };
  return { ...horizontal, ...vertical };
}

/** A one-line prompt beside `anchor`, vertically centred on it, flipped left near the right edge. */
export function promptBeside(anchor: PixelPoint, width: number, viewport: Size, gap = 24, margin = 8): { style: CSSProperties; side: 'left' | 'right' } {
  const toRight = anchor.x + gap + width <= viewport.width - margin;
  const top = clamp(anchor.y, margin + 16, Math.max(margin + 16, viewport.height - margin - 16));
  return toRight
    ? { style: { left: px(anchor.x + gap), top: px(top) }, side: 'right' }
    : { style: { right: px(Math.max(margin, viewport.width - anchor.x + gap)), top: px(top) }, side: 'left' };
}
