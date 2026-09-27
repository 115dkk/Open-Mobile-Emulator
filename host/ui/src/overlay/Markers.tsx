// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// One binding drawn over the operating system's screen (DESIGN.md 8): a tap is a 32px circle with the
// key, a hold adds a dot; a joystick is a dashed circle of its radius with the four keys in a cross; a
// swipe is a line from the start circle to an arrow head with a duration chip; mouse buttons and the
// wheel are chips in the user's words with a mouse outline. Outside the editor everything here is
// drawn only; in the editor the marker is a button and the selected one shows its handles.
import type { CSSProperties, PointerEvent as ReactPointerEvent, ReactNode } from 'react';
import type { Binding, BindingAction, Size, Trigger } from '../contracts';
import { Icon } from '../components';
import { keyLabel, triggerLabel } from '../input-profile';
import {
  arrowHead, joystickRadiusPx, markerAnchor, pastTip, placeStyle, toPixels, wheelDistancePx,
} from './geometry';
import type { DragPart, PixelPoint } from './geometry';
import { markerName } from './model';

function pixelStyle(point: PixelPoint): CSSProperties {
  return { left: `${String(point.x)}px`, top: `${String(point.y)}px` };
}

function Cross({ trigger }: { readonly trigger: Trigger }) {
  if (trigger.kind !== 'keySet') return <span className="ome-overlay-marker">{triggerLabel(trigger)}</span>;
  return (
    <span className="ome-overlay-cross">
      <span className="ome-overlay-cross-up">{keyLabel(trigger.up)}</span>
      <span className="ome-overlay-cross-left">{keyLabel(trigger.left)}</span>
      <span className="ome-overlay-cross-right">{keyLabel(trigger.right)}</span>
      <span className="ome-overlay-cross-down">{keyLabel(trigger.down)}</span>
    </span>
  );
}

/** What sits inside the marker at the binding's anchor. */
export function MarkerFace({ trigger, action }: { readonly trigger: Trigger; readonly action: BindingAction }) {
  switch (action.kind) {
    case 'tap':
      return (
        <span className={action.hold ? 'ome-overlay-marker ome-overlay-marker-hold' : 'ome-overlay-marker'}>
          {triggerLabel(trigger)}
        </span>
      );
    case 'swipe':
      return <span className="ome-overlay-marker">{triggerLabel(trigger)}</span>;
    case 'joystick':
      return <Cross trigger={trigger} />;
    case 'mouseTap':
    case 'wheelSwipe':
      return (
        <span className="ome-overlay-marker ome-overlay-marker-mouse">
          <Icon name="mouse" size={14} />
          <span>{triggerLabel(trigger)}</span>
        </span>
      );
    case 'passThrough':
      return null;
  }
}

function SwipeLines({ action, size }: { readonly action: Extract<BindingAction, { kind: 'swipe' }>; readonly size: Size }) {
  const from = toPixels(action.from, size);
  const to = toPixels(action.to, size);
  return (
    <svg className="ome-overlay-lines" viewBox={`0 0 ${String(size.width)} ${String(size.height)}`} aria-hidden="true">
      <line x1={from.x} y1={from.y} x2={to.x} y2={to.y} />
      <polyline points={arrowHead(from, to)} />
    </svg>
  );
}

/** The wheel's swipe runs through its point, `distance` of the screen height long. */
function WheelExtent({ action, size }: { readonly action: Extract<BindingAction, { kind: 'wheelSwipe' }>; readonly size: Size }) {
  const center = toPixels(action.at, size);
  const half = wheelDistancePx(action.distance, size) / 2;
  const top = center.y - half;
  const bottom = center.y + half;
  return (
    <svg className="ome-overlay-lines ome-overlay-lines-extent" viewBox={`0 0 ${String(size.width)} ${String(size.height)}`} aria-hidden="true">
      <line className="ome-overlay-extent" x1={center.x} y1={top} x2={center.x} y2={bottom} />
      <line x1={center.x - 5} y1={top} x2={center.x + 5} y2={top} />
      <line x1={center.x - 5} y1={bottom} x2={center.x + 5} y2={bottom} />
    </svg>
  );
}

export interface MarkerEdit {
  readonly selected: boolean;
  /** A pointer went down on the marker or one of its handles. */
  readonly onPress: (part: DragPart, event: ReactPointerEvent<HTMLElement>) => void;
  /** Keyboard activation of the marker: open its card. */
  readonly onOpen: () => void;
}

interface BindingMarkerProps {
  readonly binding: Binding;
  /** The action as drawn, which differs from the binding's while a drag is live. */
  readonly action: BindingAction;
  readonly size: Size;
  readonly edit?: MarkerEdit | undefined;
}

/**
 * The lines under every marker: a swipe's line and arrow head, the wheel's extent, the stick's dashed
 * circle. Drawn in their own pass so no line ever crosses another binding's chip.
 */
export function BindingLines({ action, size }: { readonly action: BindingAction; readonly size: Size }) {
  switch (action.kind) {
    case 'joystick': {
      const center = toPixels(action.center, size);
      const diameter = `${String(joystickRadiusPx(action.radius, size) * 2)}px`;
      return <span className="ome-overlay-ring" style={{ ...pixelStyle(center), width: diameter, height: diameter }} />;
    }
    case 'swipe':
      return <SwipeLines action={action} size={size} />;
    case 'wheelSwipe':
      return <WheelExtent action={action} size={size} />;
    case 'tap':
    case 'mouseTap':
    case 'passThrough':
      return null;
  }
}

/**
 * One binding's chips over the lines: the marker at its anchor, a swipe's duration chip, and the
 * selected marker's handles. A mouse button acting where it is pressed has no place on screen.
 */
export function BindingMarker({ binding, action, size, edit }: BindingMarkerProps) {
  const anchor = markerAnchor(action);
  if (anchor === null) return null;
  const face = <MarkerFace trigger={binding.trigger} action={action} />;
  const pin: ReactNode = edit === undefined ? (
    <span className="ome-overlay-pin" style={placeStyle(anchor)}>{face}</span>
  ) : (
    <button
      type="button"
      className="ome-overlay-pin ome-overlay-hit"
      style={placeStyle(anchor)}
      aria-label={markerName(binding)}
      data-selected={edit.selected ? 'true' : undefined}
      onPointerDown={(event) => { edit.onPress('marker', event); }}
      onClick={(event) => { if (event.detail === 0) edit.onOpen(); }}
    >
      {face}
    </button>
  );
  const handles = edit?.selected === true;

  switch (action.kind) {
    case 'joystick': {
      const center = toPixels(action.center, size);
      const radius = joystickRadiusPx(action.radius, size);
      return (
        <>
          {pin}
          {handles && (
            <span
              className="ome-overlay-handle ome-overlay-handle-radius"
              aria-hidden="true"
              style={pixelStyle({ x: center.x + radius, y: center.y })}
              onPointerDown={(event) => { edit?.onPress('radius', event); }}
            />
          )}
        </>
      );
    }
    case 'swipe': {
      const from = toPixels(action.from, size);
      const to = toPixels(action.to, size);
      return (
        <>
          {pin}
          <span className="ome-overlay-duration" style={pixelStyle(pastTip(from, to))}>{`${String(action.durationMs)} ms`}</span>
          {handles && (
            <span
              className="ome-overlay-handle ome-overlay-handle-end"
              aria-hidden="true"
              style={pixelStyle(to)}
              onPointerDown={(event) => { edit?.onPress('end', event); }}
            />
          )}
        </>
      );
    }
    case 'tap':
    case 'mouseTap':
    case 'wheelSwipe':
    case 'passThrough':
      return pin;
  }
}
