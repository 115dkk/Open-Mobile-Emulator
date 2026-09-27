// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// The small card beside a marker (M2-SCREENS.md 4, S4-오버레이): a tap's 홀드, a swipe's duration, a
// mouse button's button and place, the wheel's direction and distance, and for every keyboard marker
// `키 바꾸기` and `삭제`. A card for a new mouse or wheel marker picks the trigger first; picking it
// creates the binding. Changes go into the editor's draft; Rust judges the profile on 저장.
import { useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import type { Binding, BindingAction, LogicalPoint, MouseButton, Trigger, WheelDirection } from '../contracts';
import { Button, Field, Icon, IconButton, Segmented, Toggle } from '../components';
import type { SegmentedOption } from '../components';
import { actionWords, keyLabel, triggerLabel } from '../input-profile';
import { DIRECTION_WORD, JOYSTICK_ORDER, markerName, mouseButtonTaken, wheelTaken } from './model';

const BUTTONS: readonly { readonly value: MouseButton; readonly label: string }[] = [
  { value: 'left', label: '왼쪽' },
  { value: 'right', label: '오른쪽' },
  { value: 'middle', label: '가운데' },
];

const DIRECTIONS: readonly { readonly value: WheelDirection; readonly label: string }[] = [
  { value: 'up', label: '휠 위' },
  { value: 'down', label: '휠 아래' },
];

type Placement = 'pointer' | 'fixed';
const PLACEMENTS: readonly SegmentedOption<Placement>[] = [
  { value: 'pointer', label: '누른 자리' },
  { value: 'fixed', label: '고정 자리' },
];

/** The largest `duration_ms` Rust can hold (u32). */
const MAX_DURATION_MS = 4_294_967_295;

function CardFrame({ label, title, style, onClose, children, footer }: {
  readonly label: string;
  readonly title: ReactNode;
  readonly style: CSSProperties;
  readonly onClose: () => void;
  readonly children?: ReactNode;
  readonly footer?: ReactNode;
}) {
  return (
    <div className="ome-overlay-card" role="dialog" aria-label={label} style={style}>
      <div className="ome-overlay-card-head">
        <div className="ome-overlay-card-title">{title}</div>
        <IconButton icon="x" label="닫기" onClick={onClose} />
      </div>
      {children !== undefined && <div className="ome-overlay-card-body">{children}</div>}
      {footer !== undefined && <div className="ome-overlay-card-foot">{footer}</div>}
    </div>
  );
}

function Labelled({ label, children }: { readonly label: string; readonly children: ReactNode }) {
  return (
    <div className="ome-labelled">
      <span className="ome-group-label" aria-hidden="true">{label}</span>
      {children}
    </div>
  );
}

function KeyTitle({ binding }: { readonly binding: Binding }) {
  const words = actionWords(binding.action);
  if (binding.trigger.kind === 'mouseButton' || binding.trigger.kind === 'wheel') {
    return (
      <>
        <Icon name="mouse" size={16} />
        <span>{words.kind}</span>
      </>
    );
  }
  return (
    <>
      <span className="ome-input-key">{triggerLabel(binding.trigger)}</span>
      <span>{words.kind}</span>
    </>
  );
}

function buttonOptions(bindings: readonly Binding[], exceptId: string | null): SegmentedOption<MouseButton>[] {
  return BUTTONS.map((item) => ({ ...item, disabled: mouseButtonTaken(bindings, item.value, exceptId) }));
}

function directionOptions(bindings: readonly Binding[], exceptId: string | null): SegmentedOption<WheelDirection>[] {
  return DIRECTIONS.map((item) => ({ ...item, disabled: wheelTaken(bindings, item.value, exceptId) }));
}

/** Whole numbers only; anything else keeps the last valid value in the draft. */
function wholeNumber(text: string, low: number, high: number): number | null {
  if (!/^\d+$/u.test(text.trim())) return null;
  const value = Number(text.trim());
  return Number.isSafeInteger(value) && value >= low && value <= high ? value : null;
}

function DurationField({ durationMs, onChange }: { readonly durationMs: number; readonly onChange: (ms: number) => void }) {
  const [text, setText] = useState(String(durationMs));
  const valid = wholeNumber(text, 1, MAX_DURATION_MS) !== null;
  return (
    <Field
      label="지속 시간"
      type="number"
      value={text}
      suffix="ms"
      min={1}
      step={10}
      invalid={!valid}
      hint={valid ? undefined : '1 이상의 정수를 입력하십시오.'}
      onChange={(next) => {
        setText(next);
        const value = wholeNumber(next, 1, MAX_DURATION_MS);
        if (value !== null) onChange(value);
      }}
    />
  );
}

function DistanceField({ distance, onChange }: { readonly distance: number; readonly onChange: (distance: number) => void }) {
  const [text, setText] = useState(String(Math.round(distance * 100)));
  const valid = wholeNumber(text, 1, 100) !== null;
  return (
    <Field
      label="거리"
      type="number"
      value={text}
      suffix="%"
      min={1}
      max={100}
      invalid={!valid}
      hint={valid ? undefined : '1에서 100 사이의 정수를 입력하십시오.'}
      onChange={(next) => {
        setText(next);
        const value = wholeNumber(next, 1, 100);
        if (value !== null) onChange(value / 100);
      }}
    />
  );
}

export interface BindingCardProps {
  readonly binding: Binding;
  /** Every binding of the draft, for the buttons and directions another binding already uses. */
  readonly bindings: readonly Binding[];
  readonly style: CSSProperties;
  readonly onAction: (action: BindingAction) => void;
  readonly onTrigger: (trigger: Trigger) => void;
  readonly onRekey: () => void;
  readonly onDelete: () => void;
  readonly onClose: () => void;
}

/** The card of an existing binding. Mount it with `key={binding.id}` so its fields start fresh. */
export function BindingCard({ binding, bindings, style, onAction, onTrigger, onRekey, onDelete, onClose }: BindingCardProps) {
  const { action, trigger } = binding;
  // A mouse button switched to 누른 자리 and back returns to where it was fixed.
  const [lastFixed, setLastFixed] = useState<LogicalPoint>(
    action.kind === 'mouseTap' && action.at !== null ? action.at : { x: 0.5, y: 0.5 },
  );
  const keyboard = trigger.kind === 'key' || trigger.kind === 'keySet';
  const footer = (
    <>
      {keyboard ? <Button onClick={onRekey}>키 바꾸기</Button> : <span />}
      <Button variant="danger" icon="trash" onClick={onDelete}>삭제</Button>
    </>
  );

  let body: ReactNode;
  switch (action.kind) {
    case 'tap':
      body = (
        <div className="ome-overlay-card-row">
          <div className="ome-overlay-card-row-text">
            <span>홀드</span>
            <span className="ome-caption ome-muted">누르는 동안 터치 유지</span>
          </div>
          <Toggle label="홀드" checked={action.hold} onChange={(hold) => { onAction({ ...action, hold }); }} />
        </div>
      );
      break;
    case 'swipe':
      body = <DurationField durationMs={action.durationMs} onChange={(durationMs) => { onAction({ ...action, durationMs }); }} />;
      break;
    case 'joystick':
      body = trigger.kind === 'keySet' ? (
        <dl className="ome-overlay-card-keys">
          {JOYSTICK_ORDER.map((direction) => (
            <div key={direction}>
              <dt>{DIRECTION_WORD[direction]}</dt>
              <dd><span className="ome-input-key">{keyLabel(trigger[direction])}</span></dd>
            </div>
          ))}
        </dl>
      ) : undefined;
      break;
    case 'mouseTap':
      body = (
        <>
          {trigger.kind === 'mouseButton' && (
            <Labelled label="버튼">
              <Segmented
                label="버튼"
                options={buttonOptions(bindings, binding.id)}
                value={trigger.button}
                onChange={(button) => { onTrigger({ kind: 'mouseButton', button }); }}
              />
            </Labelled>
          )}
          <Labelled label="자리">
            <Segmented
              label="자리"
              options={PLACEMENTS}
              value={action.at === null ? 'pointer' : 'fixed'}
              onChange={(placement) => {
                if (placement === 'fixed') {
                  onAction({ kind: 'mouseTap', at: lastFixed });
                } else {
                  if (action.at !== null) setLastFixed(action.at);
                  onAction({ kind: 'mouseTap', at: null });
                }
              }}
            />
          </Labelled>
        </>
      );
      break;
    case 'wheelSwipe':
      body = (
        <>
          {trigger.kind === 'wheel' && (
            <Labelled label="방향">
              <Segmented
                label="방향"
                options={directionOptions(bindings, binding.id)}
                value={trigger.direction}
                onChange={(direction) => { onTrigger({ kind: 'wheel', direction }); }}
              />
            </Labelled>
          )}
          <DistanceField distance={action.distance} onChange={(distance) => { onAction({ ...action, distance }); }} />
        </>
      );
      break;
    case 'passThrough':
      body = undefined;
      break;
  }

  return (
    <CardFrame label={markerName(binding)} title={<KeyTitle binding={binding} />} style={style} onClose={onClose} footer={footer}>
      {body}
    </CardFrame>
  );
}

/** A new mouse button marker: picking the button creates it, fixed at the clicked point or not. */
export function NewMouseCard({ bindings, fixed, style, onPlacement, onPick, onClose }: {
  readonly bindings: readonly Binding[];
  readonly fixed: boolean;
  readonly style: CSSProperties;
  readonly onPlacement: (fixed: boolean) => void;
  readonly onPick: (button: MouseButton) => void;
  readonly onClose: () => void;
}) {
  return (
    <CardFrame label="마우스 버튼" title={<><Icon name="mouse" size={16} /><span>마우스 버튼</span></>} style={style} onClose={onClose}>
      <Labelled label="버튼">
        <Segmented label="버튼" options={buttonOptions(bindings, null)} value={null} onChange={onPick} />
      </Labelled>
      <Labelled label="자리">
        <Segmented
          label="자리"
          options={PLACEMENTS}
          value={fixed ? 'fixed' : 'pointer'}
          onChange={(placement) => { onPlacement(placement === 'fixed'); }}
        />
      </Labelled>
    </CardFrame>
  );
}

/** A new wheel marker: picking the direction creates it. */
export function NewWheelCard({ bindings, style, onPick, onClose }: {
  readonly bindings: readonly Binding[];
  readonly style: CSSProperties;
  readonly onPick: (direction: WheelDirection) => void;
  readonly onClose: () => void;
}) {
  return (
    <CardFrame label="휠" title={<><Icon name="mouse" size={16} /><span>휠</span></>} style={style} onClose={onClose}>
      <Labelled label="방향">
        <Segmented label="방향" options={directionOptions(bindings, null)} value={null} onChange={onPick} />
      </Labelled>
    </CardFrame>
  );
}
