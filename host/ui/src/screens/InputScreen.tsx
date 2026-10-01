// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Input screen (M2-SCREENS.md 4, mockup S4): the profile list on the left and the applied profile's
// bindings on the right. The list selection is Rust's `input.activeId`. Every edit goes to Rust as a
// whole profile or a single binding and Rust validates it; bundled presets only offer 복제 because
// Rust refuses to save them. The overlay editor lives on the stage (`화면에서 편집`).
import { useEffect, useEffectEvent, useState } from 'react';
import type { CSSProperties, MouseEvent } from 'react';
import type { ScreenActions, ScreenProps } from '../actions';
import type { AppSnapshot, Binding, InputProfile, LogicalPoint, Size } from '../contracts';
import {
  Button, Chip, Dialog, EmptyState, Field, Icon, IconButton, IssueNotice, Select, Toggle,
} from '../components';
import type { SelectOption } from '../components';
import { actionWords, newId, nextProfileName, positionLabel, triggerLabel } from '../input-profile';

type DialogKind = 'rename' | 'target' | 'delete';

/** The name Rust read for a package, or the package itself when the app is not in the list. */
function appName(snapshot: AppSnapshot, pkg: string): string {
  return snapshot.apps.items.find((item) => item.package === pkg)?.label ?? pkg;
}

function ProfileRow({ profile, active, autoApply, target, onSelect }: {
  readonly profile: InputProfile;
  readonly active: boolean;
  readonly autoApply: boolean;
  readonly target: string | null;
  readonly onSelect: () => void;
}) {
  const count = `표지 ${String(profile.bindings.length)}개`;
  return (
    <li>
      <button
        type="button"
        className="ome-input-profile"
        aria-current={active ? 'true' : undefined}
        onClick={() => { if (!active) onSelect(); }}
      >
        <span className="ome-input-profile-name">{profile.name}</span>
        <span className="ome-input-profile-meta">
          <span className="ome-caption">{target === null ? count : `${target} · ${count}`}</span>
          {autoApply && profile.targetPackage !== null && <Chip>자동 적용</Chip>}
        </span>
      </button>
    </li>
  );
}

function ProfileGroup({ title, profiles, snapshot, actions }: {
  readonly title: string;
  readonly profiles: readonly InputProfile[];
  readonly snapshot: AppSnapshot;
  readonly actions: ScreenActions;
}) {
  if (profiles.length === 0) return null;
  const { input } = snapshot;
  return (
    <section className="ome-input-group" aria-label={title}>
      <h2 className="ome-group-label">{title}</h2>
      <ul className="ome-input-profiles">
        {profiles.map((profile) => (
          <ProfileRow
            key={profile.id}
            profile={profile}
            active={profile.id === input.activeId}
            autoApply={input.autoApply}
            target={profile.targetPackage === null ? null : appName(snapshot, profile.targetPackage)}
            onSelect={() => { void actions.inputProfileSelect(profile.id); }}
          />
        ))}
      </ul>
    </section>
  );
}

// ---- Preview: the profile's markers at their logical places ----

function place(point: LogicalPoint): CSSProperties {
  return { left: `${String(point.x * 100)}%`, top: `${String(point.y * 100)}%` };
}

const VIEW_WIDTH = 1000;
const PREVIEW_HEIGHT = 320;

function Marker({ at, label, hold = false, mouse = false }: {
  readonly at: LogicalPoint;
  readonly label: string;
  readonly hold?: boolean;
  readonly mouse?: boolean;
}) {
  const classes = ['ome-input-marker'];
  if (hold) classes.push('ome-input-marker-hold');
  return (
    <span className={classes.join(' ')} style={place(at)}>
      {mouse && <Icon name="mouse" size={14} />}
      <span>{label}</span>
    </span>
  );
}

function BindingMarks({ binding, aspect }: { readonly binding: Binding; readonly aspect: Size }) {
  const { action } = binding;
  const label = triggerLabel(binding.trigger);
  switch (action.kind) {
    case 'tap':
      return <Marker at={action.at} label={label} hold={action.hold} />;
    case 'mouseTap':
      return action.at === null ? null : <Marker at={action.at} label={label} mouse />;
    case 'wheelSwipe':
      return <Marker at={action.at} label={label} mouse />;
    case 'swipe':
      return <Marker at={action.from} label={label} />;
    case 'joystick': {
      // The radius is a share of the short axis (ARCHITECTURE 8.3); the ring's width is a share of the frame's width.
      const shortOverWidth = aspect.width >= aspect.height ? aspect.height / aspect.width : 1;
      const diameter = action.radius * 2 * shortOverWidth * 100;
      return (
        <>
          <span className="ome-input-joystick" style={{ ...place(action.center), width: `${String(diameter)}%` }} />
          <Marker at={action.center} label={label} />
        </>
      );
    }
    case 'passThrough':
      return null;
  }
}

function SwipeLine({ from, to, viewHeight }: { readonly from: LogicalPoint; readonly to: LogicalPoint; readonly viewHeight: number }) {
  const x1 = from.x * VIEW_WIDTH;
  const y1 = from.y * viewHeight;
  const x2 = to.x * VIEW_WIDTH;
  const y2 = to.y * viewHeight;
  const angle = Math.atan2(y2 - y1, x2 - x1);
  const head = 16;
  const wing = (turn: number) => `${String(x2 - head * Math.cos(angle + turn))},${String(y2 - head * Math.sin(angle + turn))}`;
  return (
    <g>
      <line x1={x1} y1={y1} x2={x2} y2={y2} />
      <polyline points={`${wing(0.5)} ${String(x2)},${String(y2)} ${wing(-0.5)}`} />
    </g>
  );
}

/** At most 320px tall, never wider than the pane, always the profile's aspect. */
function surfaceStyle(aspect: Size): CSSProperties {
  const widest = Math.round((PREVIEW_HEIGHT * aspect.width) / aspect.height);
  return { aspectRatio: `${String(aspect.width)} / ${String(aspect.height)}`, width: `min(100%, ${String(widest)}px)` };
}

function Preview({ profile, label, editable, pending, onPick }: {
  readonly profile: InputProfile;
  readonly label: string;
  readonly editable: boolean;
  readonly pending: LogicalPoint | null;
  readonly onPick: (point: LogicalPoint) => void;
}) {
  const aspect = profile.referenceAspect;
  const viewHeight = (VIEW_WIDTH * aspect.height) / aspect.width;
  const pick = (event: MouseEvent<HTMLButtonElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    // Keyboard activation (detail 0) and an unmeasured frame pick the middle.
    const ratio = (offset: number, size: number) => (event.detail === 0 || size <= 0 ? 0.5 : offset / size);
    const clamp = (value: number) => Math.round(Math.min(1, Math.max(0, value)) * 1000) / 1000;
    onPick({ x: clamp(ratio(event.clientX - box.left, box.width)), y: clamp(ratio(event.clientY - box.top, box.height)) });
  };
  return (
    <div className="ome-input-preview">
      <div className="ome-input-surface" style={surfaceStyle(aspect)}>
        {editable && <button type="button" className="ome-input-surface-hit" aria-label="새 표지 자리" onClick={pick} />}
        <span className="ome-input-surface-label">{label}</span>
        <svg className="ome-input-lines" viewBox={`0 0 ${String(VIEW_WIDTH)} ${String(viewHeight)}`} aria-hidden="true">
          {profile.bindings.map((binding) => binding.action.kind === 'swipe'
            ? <SwipeLine key={binding.id} from={binding.action.from} to={binding.action.to} viewHeight={viewHeight} />
            : null)}
        </svg>
        {profile.bindings.map((binding) => (
          <BindingMarks key={binding.id} binding={binding} aspect={aspect} />
        ))}
        {pending !== null && <span className="ome-input-pending" style={place(pending)} aria-hidden="true" />}
      </div>
    </div>
  );
}

function BindingTable({ profile, editable, actions }: {
  readonly profile: InputProfile;
  readonly editable: boolean;
  readonly actions: ScreenActions;
}) {
  if (profile.bindings.length === 0) return null;
  return (
    <table className="ome-input-table">
      <thead>
        <tr>
          <th scope="col">입력</th>
          <th scope="col">동작 종류</th>
          <th scope="col">자리</th>
          <th scope="col">설명</th>
          {editable && <th scope="col"><span className="ome-visually-hidden">삭제</span></th>}
        </tr>
      </thead>
      <tbody>
        {profile.bindings.map((binding) => {
          const words = actionWords(binding.action);
          const trigger = triggerLabel(binding.trigger);
          return (
            <tr key={binding.id}>
              <td><span className="ome-input-key">{trigger}</span></td>
              <td>{words.kind}</td>
              <td className="ome-input-position">{positionLabel(binding.action) ?? ''}</td>
              <td className="ome-muted">{words.description}</td>
              {editable && (
                <td className="ome-input-remove">
                  <IconButton icon="trash" label={`${trigger} 표지 삭제`} onClick={() => actions.inputBindingRemove(profile.id, binding.id)} />
                </td>
              )}
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

// ---- Dialogs: each mounts fresh, so its draft starts from the profile ----

function RenameDialog({ profile, onSave, onCancel }: {
  readonly profile: InputProfile;
  readonly onSave: (name: string) => Promise<void>;
  readonly onCancel: () => void;
}) {
  const [name, setName] = useState(profile.name);
  const trimmed = name.trim();
  return (
    <Dialog
      open
      title="프로필 이름을 바꿉니다."
      confirmLabel="저장"
      confirmDisabled={trimmed === ''}
      onConfirm={() => onSave(trimmed)}
      onCancel={onCancel}
    >
      <Field label="이름" value={name} onChange={setName} onEnter={() => { if (trimmed !== '') void onSave(trimmed); }} />
    </Dialog>
  );
}

function TargetDialog({ profile, snapshot, onSave, onCancel }: {
  readonly profile: InputProfile;
  readonly snapshot: AppSnapshot;
  readonly onSave: (pkg: string | null) => Promise<void>;
  readonly onCancel: () => void;
}) {
  const [pkg, setPkg] = useState(profile.targetPackage ?? '');
  const options: SelectOption<string>[] = [{ value: '', label: '지정 안 함' }];
  for (const app of snapshot.apps.items) options.push({ value: app.package, label: `${app.label} (${app.package})` });
  const current = profile.targetPackage;
  if (current !== null && !options.some((option) => option.value === current)) options.push({ value: current, label: current });
  return (
    <Dialog
      open
      title="대상 앱을 지정합니다."
      confirmLabel="지정"
      onConfirm={() => onSave(pkg === '' ? null : pkg)}
      onCancel={onCancel}
    >
      <div className="ome-labelled">
        <span className="ome-group-label" aria-hidden="true">대상 앱</span>
        <Select label="대상 앱" options={options} value={pkg} onChange={setPkg} />
      </div>
    </Dialog>
  );
}

// ---- The applied profile ----

function ProfileDetail({ profile, snapshot, actions, navigate }: ScreenProps & { readonly profile: InputProfile }) {
  const { guest, input } = snapshot;
  const editable = !profile.bundled;
  const [dialog, setDialog] = useState<DialogKind | null>(null);
  const [pending, setPending] = useState<LogicalPoint | null>(null);
  const close = () => { setDialog(null); };

  // A picked place waits for one key: Esc cancels, any other key becomes a tap there.
  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (pending === null || event.code === '' || event.repeat) return;
    event.preventDefault();
    if (event.code === 'Escape') {
      setPending(null);
      return;
    }
    const binding: Binding = {
      id: newId('binding'),
      trigger: { kind: 'key', code: event.code },
      action: { kind: 'tap', at: pending, hold: false },
    };
    setPending(null);
    void actions.inputBindingUpsert(profile.id, binding);
  });
  const waiting = pending !== null;
  useEffect(() => {
    if (!waiting) return undefined;
    const listener = (event: KeyboardEvent) => { onKey(event); };
    document.addEventListener('keydown', listener);
    return () => { document.removeEventListener('keydown', listener); };
  }, [waiting]);

  const save = async (changes: Partial<InputProfile>) => {
    setDialog(null);
    await actions.inputProfileSave({ ...profile, ...changes });
  };
  const duplicate = () => actions.inputProfileSave({
    ...profile, id: newId('profile'), name: `${profile.name} 사본`, bundled: false,
  });
  const editOnStage = navigate === undefined || !editable || guest.state !== 'running' ? undefined : async () => {
    if (!input.editing) await actions.inputEditorToggle();
    navigate('stage');
  };
  const target = profile.targetPackage === null ? null : appName(snapshot, profile.targetPackage);
  const off = guest.state === 'stopped' || guest.state === 'failed';

  return (
    <>
      <div className="ome-input-head">
        <div className="ome-input-title">
          <h2 className="ome-section-title">{profile.name}</h2>
          {target !== null && <p className="ome-caption">대상 앱 {target}</p>}
        </div>
        <div className="ome-input-actions">
          <Button variant="ghost" onClick={duplicate}>복제</Button>
          {editable && (<>
            <Button variant="ghost" onClick={() => { setDialog('rename'); }}>이름 바꾸기</Button>
            <Button variant="ghost" onClick={() => { setDialog('target'); }}>대상 앱 지정</Button>
            <Button variant="danger" onClick={() => { setDialog('delete'); }}>삭제</Button>
          </>)}
          {editOnStage !== undefined && <Button variant="primary" icon="pencil" onClick={editOnStage}>화면에서 편집</Button>}
        </div>
      </div>
      {editable && <p className="ome-muted">화면 위의 자리를 클릭하고 설정할 키를 누르십시오.</p>}
      <Preview
        profile={profile}
        label={off ? '화면 스냅샷' : '현재 화면'}
        editable={editable}
        pending={pending}
        onPick={setPending}
      />
      <BindingTable profile={profile} editable={editable} actions={actions} />
      {dialog === 'rename' && <RenameDialog profile={profile} onSave={(name) => save({ name })} onCancel={close} />}
      {dialog === 'target' && (
        <TargetDialog profile={profile} snapshot={snapshot} onSave={(pkg) => save({ targetPackage: pkg })} onCancel={close} />
      )}
      <Dialog
        open={dialog === 'delete'}
        title={`${profile.name} 프로필을 삭제합니다.`}
        tone="danger"
        confirmLabel="삭제"
        onConfirm={() => {
          setDialog(null);
          return actions.inputProfileDelete(profile.id);
        }}
        onCancel={close}
      />
    </>
  );
}

export function InputScreen({ snapshot, actions, navigate }: ScreenProps) {
  const { input } = snapshot;
  const bundled = input.profiles.filter((profile) => profile.bundled);
  const own = input.profiles.filter((profile) => !profile.bundled);
  const active = input.profiles.find((profile) => profile.id === input.activeId) ?? null;

  const create = () => actions.inputProfileSave({
    id: newId('profile'),
    name: nextProfileName(input.profiles.map((profile) => profile.name)),
    bundled: false,
    targetPackage: null,
    referenceAspect: snapshot.guest.resolution ?? { width: 16, height: 9 },
    anchor: 'center',
    bindings: [],
  });

  return (
    <div className="ome-input">
      {snapshot.issue !== null && <div className="ome-input-issue"><IssueNotice issue={snapshot.issue} /></div>}
      <div className="ome-input-panes">
        <div className="ome-input-list">
          <h1 className="ome-page-title">입력</h1>
          <ProfileGroup title="동봉 프리셋" profiles={bundled} snapshot={snapshot} actions={actions} />
          <ProfileGroup title="내 프로필" profiles={own} snapshot={snapshot} actions={actions} />
          <div className="ome-input-list-foot">
            <div className="ome-input-auto">
              <span>자동 적용</span>
              <Toggle label="자동 적용" checked={input.autoApply} onChange={(next) => actions.inputAutoApplySet(next)} />
            </div>
            <Button icon="plus" className="ome-input-new" onClick={create}>새 프로필</Button>
          </div>
        </div>
        <section className="ome-input-detail" aria-label={active?.name ?? '입력 프로필'}>
          {active !== null ? (
            <ProfileDetail key={active.id} profile={active} snapshot={snapshot} actions={actions} navigate={navigate} />
          ) : (
            <EmptyState
              icon="input"
              title={input.profiles.length === 0 ? '입력 프로필이 없습니다.' : '왼쪽 목록에서 프로필을 고르십시오.'}
            />
          )}
        </section>
      </div>
    </div>
  );
}
