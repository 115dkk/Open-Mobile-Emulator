// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The overlay editor (M2-SCREENS.md 4, S4-오버레이; ADR-0005): a tool bar at the top, the applied
// profile's markers as buttons, and a local draft of its bindings. Clicking an empty spot and pressing
// a key makes a marker; markers are dragged to move them; a click opens the small card. 저장 sends the
// whole profile once and Rust validates it; on an AppIssue the draft stays and the issue shows here.
import { useEffect, useEffectEvent, useState } from 'react';
import type { CSSProperties, MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent } from 'react';
import type { ScreenActions } from '../actions';
import type {
  AppIssue, AppSnapshot, Binding, BindingAction, InputProfile, LogicalPoint, MouseButton, Size, Trigger, WheelDirection,
} from '../contracts';
import { Button, Icon, IssueNotice, Segmented } from '../components';
import type { SegmentedOption } from '../components';
import { keyLabel, newId, triggerLabel } from '../input-profile';
import {
  DRAG_THRESHOLD_PX, MIN_SWIPE_PX, applyDrag, chipReach, crowded, floatBeside, joystickRadiusPx, markerAnchor, markerExtent,
  pixelDistance, placeStyle, pointExtent, promptBeside, toLogical, toPixels,
} from './geometry';
import type { DragPart, PixelPoint } from './geometry';
import { LeaveDialog } from './LeaveDialog';
import { BindingCard, NewMouseCard, NewWheelCard } from './MarkerCard';
import { BindingLines, BindingMarker, MarkerFace } from './Markers';
import {
  DEFAULT_JOYSTICK_RADIUS, DEFAULT_SWIPE_MS, DEFAULT_WHEEL_DISTANCE, DIRECTION_WORD, JOYSTICK_ORDER, judgeKey, markerName,
  sameValue,
} from './model';
import type { OverlaySeed, Tool } from './model';
import { clientOrigin } from './use-element-size';

const TOOLS: readonly SegmentedOption<Tool>[] = [
  { value: 'tap', label: '탭' },
  { value: 'joystick', label: '조이스틱' },
  { value: 'swipe', label: '스와이프' },
  { value: 'mouse', label: '마우스 버튼' },
  { value: 'wheel', label: '휠' },
];

const CARD_WIDTH = 312;
const PROMPT_WIDTH = 200;
/** Cards of mouse buttons acting where pressed stand above the tray in the bottom-left corner. */
const TRAY_CARD_STYLE: CSSProperties = { left: '16px', bottom: '64px' };

/** What the editor waits for after a click: a key, four keys, or a pick in a new marker's card. */
type Pending =
  | { readonly kind: 'tap'; readonly at: LogicalPoint }
  | { readonly kind: 'swipe'; readonly from: LogicalPoint; readonly to: LogicalPoint }
  | { readonly kind: 'joystick'; readonly center: LogicalPoint; readonly keys: readonly string[] }
  | { readonly kind: 'mouse'; readonly at: LogicalPoint; readonly fixed: boolean }
  | { readonly kind: 'wheel'; readonly at: LogicalPoint }
  | { readonly kind: 'rekey'; readonly id: string }
  | { readonly kind: 'rekeySet'; readonly id: string; readonly keys: readonly string[] };

type Notice = 'invalid' | 'taken' | null;

/** A pointer holding a marker or one of its handles. */
interface Drag {
  readonly id: string;
  readonly part: DragPart;
  readonly pointerId: number;
  readonly startX: number;
  readonly startY: number;
  /** Pointer minus the held point, in pixels, so the marker does not jump under the pointer. */
  readonly grab: PixelPoint;
  readonly moved: boolean;
  readonly point: LogicalPoint | null;
}

/** A pointer pressed on the empty screen: a click, or a swipe being drawn. */
interface Sketch {
  readonly pointerId: number;
  readonly startX: number;
  readonly startY: number;
  readonly from: LogicalPoint;
  readonly to: LogicalPoint;
  readonly moved: boolean;
}

type Leave =
  | { readonly phase: 'closed' }
  | { readonly phase: 'asking' }
  | { readonly phase: 'saving'; readonly issueBefore: AppIssue | null };

function capturePointer(element: Element, pointerId: number): void {
  // Test DOMs have no pointer capture; the events still reach the handlers there.
  if (typeof element.setPointerCapture === 'function') element.setPointerCapture(pointerId);
}

function anchorsExcept(bindings: readonly Binding[], exceptId: string | null): LogicalPoint[] {
  const points: LogicalPoint[] = [];
  for (const binding of bindings) {
    if (binding.id === exceptId) continue;
    const anchor = markerAnchor(binding.action);
    if (anchor !== null) points.push(anchor);
  }
  return points;
}

function keySet(keys: readonly string[]): Trigger | null {
  const [up, down, left, right] = keys;
  if (up === undefined || down === undefined || left === undefined || right === undefined) return null;
  return { kind: 'keySet', up, down, left, right };
}

function waitsForKey(pending: Pending | null): boolean {
  return pending !== null && pending.kind !== 'mouse' && pending.kind !== 'wheel';
}

function promptText(pending: Pending): string {
  if (pending.kind === 'joystick' || pending.kind === 'rekeySet') {
    const next = JOYSTICK_ORDER[pending.keys.length] ?? 'right';
    return `${DIRECTION_WORD[next]} 키를 누르십시오`;
  }
  return '키를 누르십시오';
}

const NOTICE_TEXT: Readonly<Record<Exclude<Notice, null>, string>> = {
  taken: '이미 쓰는 키입니다.',
  invalid: '지정할 수 없는 키입니다.',
};

export interface OverlayEditorProps {
  readonly profile: InputProfile;
  readonly snapshot: AppSnapshot;
  readonly actions: ScreenActions;
  /** The page's own size: the guest screen in CSS pixels. */
  readonly size: Size;
  readonly seed?: OverlaySeed | undefined;
}

/** Mount with `key={profile.id}`: a different profile starts a new draft. */
export function OverlayEditor({ profile, snapshot, actions, size, seed }: OverlayEditorProps) {
  const editable = !profile.bundled;
  const [layer, setLayer] = useState<HTMLDivElement | null>(null);
  const [draft, setDraft] = useState<readonly Binding[]>(profile.bindings);
  const [base, setBase] = useState<readonly Binding[]>(profile.bindings);
  const [tool, setTool] = useState<Tool>(seed?.tool ?? 'tap');
  const [selectedId, setSelectedId] = useState<string | null>(seed?.selectedId ?? null);
  const [cardOpen, setCardOpen] = useState(seed?.cardOpen === true);
  const [pending, setPending] = useState<Pending | null>(null);
  const [notice, setNotice] = useState<Notice>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [sketch, setSketch] = useState<Sketch | null>(null);
  const [leave, setLeave] = useState<Leave>({ phase: 'closed' });

  // The saved profile changed (a save landed, or another window saved): an untouched draft follows it.
  if (base !== profile.bindings) {
    if (sameValue(draft, base)) setDraft(profile.bindings);
    setBase(profile.bindings);
  }
  const dirty = !sameValue(draft, profile.bindings);

  // `저장하고 끝`: end once the snapshot carries the draft; stay if the save came back with an issue.
  if (leave.phase === 'saving' && dirty && snapshot.issue !== null && snapshot.issue !== leave.issueBefore) {
    setLeave({ phase: 'closed' });
  }
  const saved = leave.phase === 'saving' && !dirty;
  useEffect(() => {
    if (saved) void actions.inputEditorToggle();
  }, [saved, actions]);

  const selected = editable ? draft.find((binding) => binding.id === selectedId) ?? null : null;

  const pointerAt = (event: { readonly clientX: number; readonly clientY: number }): PixelPoint => {
    const origin = clientOrigin(layer);
    return { x: event.clientX - origin.left, y: event.clientY - origin.top };
  };
  const focusLayer = () => { layer?.focus({ preventScroll: true }); };
  const clearTransient = () => {
    setPending(null);
    setNotice(null);
    setSketch(null);
    setDrag(null);
  };
  const updateBinding = (id: string, change: (binding: Binding) => Binding) => {
    setDraft((current) => current.map((binding) => (binding.id === id ? change(binding) : binding)));
  };
  const addBinding = (binding: Binding, openCard = false) => {
    setDraft((current) => [...current, binding]);
    setSelectedId(binding.id);
    setCardOpen(openCard);
    setPending(null);
    setNotice(null);
  };
  const openCard = (id: string) => {
    clearTransient();
    setSelectedId(id);
    setCardOpen(true);
  };

  /** A click on the empty screen with the current tool. */
  const begin = (point: LogicalPoint) => {
    setSelectedId(null);
    setCardOpen(false);
    setNotice(null);
    if (crowded(point, anchorsExcept(draft, null), size)) {
      setPending(null);
      return;
    }
    switch (tool) {
      case 'tap': setPending({ kind: 'tap', at: point }); break;
      case 'joystick': setPending({ kind: 'joystick', center: point, keys: [] }); break;
      case 'mouse': setPending({ kind: 'mouse', at: point, fixed: true }); break;
      case 'wheel': setPending({ kind: 'wheel', at: point }); break;
      case 'swipe': setPending(null); break;
    }
    if (tool === 'tap' || tool === 'joystick') focusLayer();
  };

  const capture = (code: string) => {
    if (pending === null) return;
    const hotkey = snapshot.input.suspendHotkey;
    switch (pending.kind) {
      case 'tap':
      case 'swipe':
      case 'rekey': {
        const verdict = judgeKey(code, draft, pending.kind === 'rekey' ? pending.id : null, hotkey);
        if (verdict !== 'ok') {
          setNotice(verdict);
          return;
        }
        const trigger: Trigger = { kind: 'key', code };
        if (pending.kind === 'tap') {
          addBinding({ id: newId('binding'), trigger, action: { kind: 'tap', at: pending.at, hold: false } });
        } else if (pending.kind === 'swipe') {
          addBinding({
            id: newId('binding'), trigger,
            action: { kind: 'swipe', from: pending.from, to: pending.to, durationMs: DEFAULT_SWIPE_MS },
          });
        } else {
          updateBinding(pending.id, (binding) => ({ ...binding, trigger }));
          setSelectedId(pending.id);
          setPending(null);
          setNotice(null);
        }
        return;
      }
      case 'joystick':
      case 'rekeySet': {
        const verdict = judgeKey(code, draft, pending.kind === 'rekeySet' ? pending.id : null, hotkey, pending.keys);
        if (verdict !== 'ok') {
          setNotice(verdict);
          return;
        }
        const keys = [...pending.keys, code];
        const trigger = keySet(keys);
        setNotice(null);
        if (trigger === null) {
          setPending({ ...pending, keys });
        } else if (pending.kind === 'joystick') {
          addBinding({
            id: newId('binding'), trigger,
            action: { kind: 'joystick', center: pending.center, radius: DEFAULT_JOYSTICK_RADIUS },
          });
        } else {
          updateBinding(pending.id, (binding) => ({ ...binding, trigger }));
          setSelectedId(pending.id);
          setPending(null);
        }
        return;
      }
      case 'mouse':
      case 'wheel':
        return;
    }
  };

  // Keys belong to the editor only while it waits for one; Esc cancels that wait or closes the card.
  // Registered in the capture phase so a focused control never also acts on the captured key.
  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (leave.phase === 'asking') return;
    if (event.key === 'Escape' || event.code === 'Escape') {
      if (pending !== null || sketch !== null || drag !== null) {
        event.preventDefault();
        clearTransient();
      } else if (cardOpen) {
        event.preventDefault();
        setCardOpen(false);
      }
      return;
    }
    if (!waitsForKey(pending) || event.code === '') return;
    event.preventDefault();
    event.stopPropagation();
    if (!event.repeat) capture(event.code);
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent) => { onKey(event); };
    document.addEventListener('keydown', listener, true);
    return () => { document.removeEventListener('keydown', listener, true); };
  }, []);

  // ---- Pointer: markers and handles are dragged, the empty screen is clicked or drawn on ----

  const press = (binding: Binding) => (part: DragPart, event: ReactPointerEvent<HTMLElement>) => {
    if (event.button !== 0) return;
    event.stopPropagation();
    setPending(null);
    setNotice(null);
    if (binding.id !== selectedId) setCardOpen(false);
    const pointer = pointerAt(event);
    const held = part === 'marker' ? markerAnchor(binding.action)
      : part === 'end' && binding.action.kind === 'swipe' ? binding.action.to : null;
    const heldAt = held === null ? pointer : toPixels(held, size);
    capturePointer(event.currentTarget, event.pointerId);
    setDrag({
      id: binding.id, part, pointerId: event.pointerId, startX: event.clientX, startY: event.clientY,
      grab: { x: pointer.x - heldAt.x, y: pointer.y - heldAt.y }, moved: false, point: null,
    });
  };

  const dragPoint = (event: ReactPointerEvent<HTMLElement>, current: Drag): LogicalPoint => {
    const pointer = pointerAt(event);
    return toLogical({ x: pointer.x - current.grab.x, y: pointer.y - current.grab.y }, size);
  };

  const finishDrag = (event: ReactPointerEvent<HTMLElement>, current: Drag) => {
    setDrag(null);
    const binding = draft.find((item) => item.id === current.id);
    if (binding === undefined) return;
    if (!current.moved) {
      if (current.part === 'marker') openCard(binding.id);
      return;
    }
    const next = applyDrag(binding.action, current.part, dragPoint(event, current), size);
    setSelectedId(binding.id);
    if (current.part === 'marker') {
      const anchor = markerAnchor(next);
      // Dropped onto another marker's hit area: it goes back where it was (DESIGN.md 8).
      if (anchor !== null && crowded(anchor, anchorsExcept(draft, binding.id), size)) return;
    }
    updateBinding(binding.id, (item) => ({ ...item, action: next }));
  };

  const finishSketch = (event: ReactPointerEvent<HTMLElement>, current: Sketch) => {
    setSketch(null);
    if (tool !== 'swipe') {
      begin(current.from);
      return;
    }
    const to = toLogical(pointerAt(event), size);
    if (pixelDistance(current.from, to, size) < MIN_SWIPE_PX) return;
    if (crowded(current.from, anchorsExcept(draft, null), size)) return;
    setPending({ kind: 'swipe', from: current.from, to });
    setNotice(null);
    focusLayer();
  };

  const onSurfaceDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return;
    // The first click beside an open card only closes it.
    if (cardOpen || pending?.kind === 'mouse' || pending?.kind === 'wheel') {
      setCardOpen(false);
      setPending(null);
      setNotice(null);
      return;
    }
    const point = toLogical(pointerAt(event), size);
    capturePointer(event.currentTarget, event.pointerId);
    setSelectedId(null);
    setSketch({ pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, from: point, to: point, moved: false });
  };

  /** Enter or Space on the empty screen places the marker in the middle. */
  const onSurfaceKey = (event: ReactMouseEvent<HTMLButtonElement>) => {
    if (event.detail !== 0) return;
    if (tool !== 'swipe') {
      begin({ x: 0.5, y: 0.5 });
      return;
    }
    const from = { x: 0.4, y: 0.5 };
    if (crowded(from, anchorsExcept(draft, null), size)) return;
    setSelectedId(null);
    setCardOpen(false);
    setPending({ kind: 'swipe', from, to: { x: 0.6, y: 0.5 } });
    setNotice(null);
    focusLayer();
  };

  const onMove = (event: ReactPointerEvent<HTMLElement>) => {
    if (drag !== null && event.pointerId === drag.pointerId) {
      const moved = drag.moved || Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) >= DRAG_THRESHOLD_PX;
      if (!moved) return;
      if (!drag.moved) setCardOpen(false);
      setDrag({ ...drag, moved: true, point: dragPoint(event, drag) });
      return;
    }
    if (sketch !== null && event.pointerId === sketch.pointerId) {
      const moved = sketch.moved || Math.hypot(event.clientX - sketch.startX, event.clientY - sketch.startY) >= DRAG_THRESHOLD_PX;
      setSketch({ ...sketch, moved, to: toLogical(pointerAt(event), size) });
    }
  };

  const onUp = (event: ReactPointerEvent<HTMLElement>) => {
    if (drag !== null && event.pointerId === drag.pointerId) {
      finishDrag(event, drag);
      return;
    }
    if (sketch !== null && event.pointerId === sketch.pointerId) finishSketch(event, sketch);
  };

  const onCancel = () => {
    setDrag(null);
    setSketch(null);
  };

  // ---- Tool bar ----

  const save = () => actions.inputProfileSave({ ...profile, bindings: draft });
  const revert = () => {
    clearTransient();
    setDraft(profile.bindings);
    setCardOpen(false);
  };
  const end = () => {
    if (dirty) {
      clearTransient();
      setLeave({ phase: 'asking' });
      return;
    }
    setLeave({ phase: 'closed' });
    void actions.inputEditorToggle();
  };

  // ---- Drawing ----

  const shown = (binding: Binding): BindingAction =>
    drag !== null && drag.id === binding.id && drag.moved && drag.point !== null
      ? applyDrag(binding.action, drag.part, drag.point, size)
      : binding.action;
  const atPointer = draft.filter((binding) => binding.action.kind === 'mouseTap' && binding.action.at === null);

  const cardStyle = (binding: Binding): CSSProperties => {
    const words = binding.trigger.kind === 'mouseButton' || binding.trigger.kind === 'wheel';
    const extent = markerExtent(binding.action, size, chipReach(triggerLabel(binding.trigger), words));
    return extent === null ? TRAY_CARD_STYLE : floatBeside(extent, CARD_WIDTH, size);
  };

  const promptAnchor = (current: Pending): LogicalPoint | null => {
    switch (current.kind) {
      case 'tap': return current.at;
      case 'swipe': return current.to;
      case 'joystick': return current.center;
      case 'rekey':
      case 'rekeySet': {
        const binding = draft.find((item) => item.id === current.id);
        return binding === undefined ? null : markerAnchor(binding.action);
      }
      case 'mouse':
      case 'wheel':
        return null;
    }
  };

  const liveSwipe = sketch !== null && sketch.moved && tool === 'swipe' ? { from: sketch.from, to: sketch.to }
    : pending?.kind === 'swipe' ? { from: pending.from, to: pending.to } : null;
  const prompt = pending !== null && waitsForKey(pending) ? promptAnchor(pending) : null;
  const placed = prompt === null ? null : promptBeside(toPixels(prompt, size), PROMPT_WIDTH, size);

  return (
    <div
      ref={setLayer}
      className="ome-overlay-layer"
      tabIndex={-1}
      onPointerMove={onMove}
      onPointerUp={onUp}
      onPointerCancel={onCancel}
    >
      {editable && (
        <button
          type="button"
          className="ome-overlay-surface"
          aria-label="새 표지 자리"
          onPointerDown={onSurfaceDown}
          onClick={onSurfaceKey}
        />
      )}

      {draft.map((binding) => {
        const action = shown(binding);
        if (markerAnchor(action) === null) return null;
        return (
          <div key={`${binding.id}/lines`} className="ome-overlay-binding">
            <BindingLines action={action} size={size} />
          </div>
        );
      })}
      {draft.map((binding) => {
        const action = shown(binding);
        if (markerAnchor(action) === null) return null;
        return (
          <div key={binding.id} className="ome-overlay-binding">
            <BindingMarker
              binding={binding}
              action={action}
              size={size}
              edit={editable ? {
                selected: binding.id === selectedId,
                onPress: press(binding),
                onOpen: () => { openCard(binding.id); },
              } : undefined}
            />
          </div>
        );
      })}

      {liveSwipe !== null && (
        <div className="ome-overlay-binding ome-overlay-draft">
          <svg className="ome-overlay-lines" viewBox={`0 0 ${String(size.width)} ${String(size.height)}`} aria-hidden="true">
            <line
              x1={toPixels(liveSwipe.from, size).x} y1={toPixels(liveSwipe.from, size).y}
              x2={toPixels(liveSwipe.to, size).x} y2={toPixels(liveSwipe.to, size).y}
            />
          </svg>
          <span className="ome-overlay-pending" style={placeStyle(liveSwipe.from)} />
        </div>
      )}
      {pending?.kind === 'joystick' && (
        <div className="ome-overlay-binding ome-overlay-draft">
          <span
            className="ome-overlay-ring ome-overlay-ring-pending"
            style={{
              ...placeStyle(pending.center),
              width: `${String(joystickRadiusPx(DEFAULT_JOYSTICK_RADIUS, size) * 2)}px`,
              height: `${String(joystickRadiusPx(DEFAULT_JOYSTICK_RADIUS, size) * 2)}px`,
            }}
          />
          <span className="ome-overlay-pin" style={placeStyle(pending.center)}>
            <span className="ome-overlay-cross">
              <span className="ome-overlay-cross-up">{pending.keys[0] === undefined ? '' : keyLabel(pending.keys[0])}</span>
              <span className="ome-overlay-cross-left">{pending.keys[2] === undefined ? '' : keyLabel(pending.keys[2])}</span>
              <span className="ome-overlay-cross-right">{pending.keys[3] === undefined ? '' : keyLabel(pending.keys[3])}</span>
              <span className="ome-overlay-cross-down">{pending.keys[1] === undefined ? '' : keyLabel(pending.keys[1])}</span>
            </span>
          </span>
        </div>
      )}
      {(pending?.kind === 'tap' || pending?.kind === 'mouse' || pending?.kind === 'wheel') && (
        <span className="ome-overlay-pending" style={placeStyle(pending.at)} aria-hidden="true" />
      )}
      {pending !== null && placed !== null && (
        <div className="ome-overlay-prompt" data-side={placed.side} style={placed.style} role="status">
          {notice !== null && <span className="ome-overlay-prompt-notice">{NOTICE_TEXT[notice]}</span>}
          <span>{promptText(pending)}</span>
        </div>
      )}

      {atPointer.length > 0 && (
        <div className="ome-overlay-tray" role="group" aria-label="누른 자리">
          <span className="ome-overlay-tray-label" aria-hidden="true">누른 자리</span>
          {atPointer.map((binding) => (editable ? (
            <button
              key={binding.id}
              type="button"
              className="ome-overlay-hit ome-overlay-tray-item"
              aria-label={markerName(binding)}
              data-selected={binding.id === selectedId ? 'true' : undefined}
              onClick={() => { openCard(binding.id); }}
            >
              <MarkerFace trigger={binding.trigger} action={binding.action} />
            </button>
          ) : (
            <span key={binding.id} className="ome-overlay-tray-item">
              <MarkerFace trigger={binding.trigger} action={binding.action} />
            </span>
          )))}
        </div>
      )}

      {cardOpen && selected !== null && drag === null && (
        <BindingCard
          key={selected.id}
          binding={selected}
          bindings={draft}
          style={cardStyle(selected)}
          onAction={(action) => { updateBinding(selected.id, (binding) => ({ ...binding, action })); }}
          onTrigger={(trigger) => { updateBinding(selected.id, (binding) => ({ ...binding, trigger })); }}
          onRekey={() => {
            setCardOpen(false);
            setNotice(null);
            setPending(selected.trigger.kind === 'keySet' ? { kind: 'rekeySet', id: selected.id, keys: [] } : { kind: 'rekey', id: selected.id });
            focusLayer();
          }}
          onDelete={() => {
            setDraft((current) => current.filter((binding) => binding.id !== selected.id));
            setSelectedId(null);
            setCardOpen(false);
          }}
          onClose={() => { setCardOpen(false); }}
        />
      )}
      {pending?.kind === 'mouse' && (
        <NewMouseCard
          bindings={draft}
          fixed={pending.fixed}
          style={floatBeside(pointExtent(pending.at, size), CARD_WIDTH, size)}
          onPlacement={(fixed) => { setPending({ ...pending, fixed }); }}
          onPick={(button: MouseButton) => {
            addBinding({
              id: newId('binding'),
              trigger: { kind: 'mouseButton', button },
              action: { kind: 'mouseTap', at: pending.fixed ? pending.at : null },
            }, true);
          }}
          onClose={() => { setPending(null); }}
        />
      )}
      {pending?.kind === 'wheel' && (
        <NewWheelCard
          bindings={draft}
          style={floatBeside(pointExtent(pending.at, size), CARD_WIDTH, size)}
          onPick={(direction: WheelDirection) => {
            addBinding({
              id: newId('binding'),
              trigger: { kind: 'wheel', direction },
              action: { kind: 'wheelSwipe', at: pending.at, distance: DEFAULT_WHEEL_DISTANCE },
            }, true);
          }}
          onClose={() => { setPending(null); }}
        />
      )}

      <div className="ome-overlay-top">
        <div className="ome-overlay-bar" role="group" aria-label="매핑 편집">
          <Segmented
            label="표지 종류"
            options={TOOLS}
            value={tool}
            disabled={!editable}
            onChange={(next) => {
              setTool(next);
              clearTransient();
              setCardOpen(false);
            }}
          />
          <span className="ome-overlay-bar-divider" aria-hidden="true" />
          <Button variant="primary" disabled={!editable || !dirty} onClick={save}>저장</Button>
          <Button disabled={!editable || !dirty} onClick={revert}>되돌리기</Button>
          <Button onClick={end}>편집 끝</Button>
        </div>
        {profile.bundled && (
          <span className="ome-overlay-chip">
            <Icon name="info" size={14} strokeWidth={2} />
            동봉 프리셋은 복제한 뒤 편집할 수 있습니다.
          </span>
        )}
        {snapshot.issue !== null && <div className="ome-overlay-issue"><IssueNotice issue={snapshot.issue} /></div>}
      </div>

      {leave.phase === 'asking' && (
        <LeaveDialog
          onSave={() => {
            setLeave({ phase: 'saving', issueBefore: snapshot.issue });
            void save();
          }}
          onDiscard={() => {
            setDraft(profile.bindings);
            setLeave({ phase: 'closed' });
            void actions.inputEditorToggle();
          }}
          onStay={() => { setLeave({ phase: 'closed' }); }}
        />
      )}
    </div>
  );
}
