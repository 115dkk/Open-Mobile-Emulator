// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Stage (M2-SCREENS.md 2): a toolbar, the stage where Rust places the operating system's window, and a
// status bar. Which buttons exist follows the guest state and the capability probe in the snapshot;
// whether a command succeeds is Rust's call. Marker visibility is a view setting and stays in every
// state; volume needs a running system whose probe found the media volume and reported its value.
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import type { ScreenActions, ScreenProps } from '../actions';
import type {
  AppSnapshot, Capability, CapabilityId, DisplayPreset, DisplayView, ExitKind, GuestView, LastExit, Size,
} from '../contracts';
import { Button, Icon, IconButton, IssueNotice, Slider, StageFrame, StatusDot, StepList } from '../components';
import { formatTimestamp } from '../format';
import { keyLabel } from '../input-profile';
import { FAILURE_NEXT_STEP, GUEST_STATE_LABEL, GUEST_STATE_TONE, bootSteps } from '../presentation';
import { useGuestKeyboard } from './use-guest-keyboard';

function sizeText(size: Size): string {
  return `${String(size.width)}x${String(size.height)}`;
}

function presetText(preset: DisplayPreset): string {
  return preset.orientation === 'portrait' ? `세로 ${sizeText(preset.size)}` : sizeText(preset.size);
}

function activePreset(display: DisplayView): DisplayPreset | undefined {
  return display.presets.find((preset) => preset.id === display.activeId);
}

function capability(guest: GuestView, id: CapabilityId): Capability {
  return guest.capabilities.items.find((item) => item.id === id)?.state ?? 'unknown';
}

/** Exits that did not end the way the user or the operating system asked. */
const ABNORMAL_EXITS: ReadonlySet<ExitKind> = new Set<ExitKind>(['bootTimeout', 'crash', 'startFailed']);

/** The failure title names the cause (M2-SCREENS.md 2, DESIGN.md 9). */
function failureTitle(lastExit: LastExit | null): string {
  switch (lastExit?.kind) {
    case 'bootTimeout': return '운영체제 부팅에 실패했습니다.';
    case 'crash': return '크래시가 일어났습니다.';
    default: return '가상 머신이 시작하지 못했습니다.';
  }
}

interface PresetSelectProps {
  readonly display: DisplayView;
  readonly guest: GuestView;
  readonly onApply: ScreenActions['displayPresetApply'];
}

/**
 * A native select, because the operating system's window is a native child window over the stage:
 * a list drawn by the webview under the toolbar would disappear behind it, the system list does not.
 */
function PresetSelect({ display, guest, onApply }: PresetSelectProps) {
  if (display.presets.length === 0) return null;
  const active = activePreset(display);
  const current = display.custom?.size ?? guest.resolution;
  return (
    <span className="ome-stage-preset">
      <span className="ome-stage-preset-icon"><Icon name="display" size={20} /></span>
      <select
        className="ome-stage-preset-input"
        aria-label="표시 프리셋"
        value={active?.id ?? ''}
        onChange={(event) => {
          const id = event.currentTarget.value;
          if (id !== '') void onApply(id);
        }}
      >
        {active === undefined && (
          <option value="" disabled hidden>{current === null ? '표시 프리셋' : sizeText(current)}</option>
        )}
        {display.presets.map((preset) => <option key={preset.id} value={preset.id}>{presetText(preset)}</option>)}
      </select>
      <span className="ome-stage-preset-chevron"><Icon name="chevron-down" size={16} /></span>
    </span>
  );
}

/** The media volume index range of `guest_volume_set` (ARCHITECTURE.md 8.7). */
const MEDIA_VOLUME_MAX = 15;

/**
 * `볼륨`: a disclosure button whose slider opens inline in the toolbar row, because a panel under the
 * toolbar would sit behind the operating system's native window. Every step is sent at once so the
 * change is audible while dragging; the draft keeps the thumb in place until the snapshot follows.
 */
function VolumeControl({ volume, onSet }: { readonly volume: number; readonly onSet: ScreenActions['guestVolumeSet'] }) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<number | null>(null);
  const sliderId = useId();
  return (
    <>
      <button
        type="button"
        className="ome-icon-button ome-stage-volume-toggle"
        aria-label="볼륨"
        title="볼륨"
        aria-expanded={open}
        aria-controls={open ? sliderId : undefined}
        onClick={() => { setOpen(!open); }}
      >
        <Icon name="volume" size={20} />
      </button>
      {open && (
        <div id={sliderId} className="ome-stage-volume">
          <Slider
            label="미디어 볼륨"
            min={0}
            max={MEDIA_VOLUME_MAX}
            value={draft ?? volume}
            format={String}
            onChange={(next) => {
              setDraft(next);
              void onSet(next);
            }}
            onCommit={() => { setDraft(null); }}
          />
        </div>
      )}
    </>
  );
}

function StageToolbar({ snapshot, actions }: ScreenProps) {
  const { guest, display, input } = snapshot;
  const running = guest.state === 'running';
  const screenshot = running && capability(guest, 'screenshot') !== 'unavailable';
  const volume = running && capability(guest, 'mediaVolume') === 'available' ? guest.mediaVolume : null;
  return (
    <div className="ome-stage-toolbar" role="group" aria-label="무대 도구">
      {screenshot && <IconButton icon="camera" label="스크린샷" onClick={actions.screenshotSave} />}
      {volume !== null && <VolumeControl volume={volume} onSet={actions.guestVolumeSet} />}
      {(screenshot || volume !== null) && <span className="ome-stage-toolbar-divider" aria-hidden="true" />}
      <PresetSelect display={display} guest={guest} onApply={actions.displayPresetApply} />
      <IconButton icon="eye" label="매핑 표시" pressed={input.overlayVisible} onClick={actions.inputOverlayToggle} />
      {running && (
        <IconButton icon="pencil" label="매핑 편집" pressed={input.editing} onClick={actions.inputEditorToggle} />
      )}
      {running && (
        <div className="ome-stage-toolbar-end">
          <IconButton icon="refresh" label="다시 시작" onClick={actions.guestRestart} />
          <IconButton icon="power" label="끄기" onClick={actions.guestStop} />
        </div>
      )}
    </div>
  );
}

function StoppedStage({ lastExit, actions }: { readonly lastExit: LastExit | null; readonly actions: ScreenActions }) {
  const abnormal = lastExit !== null && ABNORMAL_EXITS.has(lastExit.kind);
  return (
    <div className="ome-stage-start">
      <Button variant="primary" size="large" icon="play" className="ome-stage-start-button" onClick={actions.guestStart}>
        시작
      </Button>
      {lastExit !== null && (
        <p className="ome-stage-caption">
          마지막 실행 {formatTimestamp(lastExit.at)}, {abnormal ? '비정상 종료' : '정상 종료'}
        </p>
      )}
      {abnormal && <Button icon="folder-open" onClick={actions.openLogsFolder}>로그 보기</Button>}
    </div>
  );
}

function BootStage({ guest, onCancel }: { readonly guest: GuestView; readonly onCancel?: (() => Promise<void>) | undefined }) {
  return (
    <div className="ome-stage-boot">
      <StepList label="부팅 진행" items={bootSteps(guest, false)} />
      {onCancel !== undefined && <div><Button onClick={onCancel}>취소</Button></div>}
    </div>
  );
}

function FailedStage({ lastExit, actions }: { readonly lastExit: LastExit | null; readonly actions: ScreenActions }) {
  const logPath = lastExit?.logPath ?? null;
  return (
    <div className="ome-stage-failure">
      <div className="ome-stage-failure-head">
        <span className="ome-stage-failure-icon"><Icon name="alert-circle" size={24} strokeWidth={2} /></span>
        <h2 className="ome-stage-failure-title">{failureTitle(lastExit)}</h2>
      </div>
      <p className="ome-stage-failure-body">{FAILURE_NEXT_STEP}</p>
      <div className="ome-stage-failure-actions">
        {/* The virtual machine has ended; the supervisor starts again from Failed. */}
        <Button variant="primary" icon="refresh" onClick={actions.guestStart}>다시 시작</Button>
        <Button icon="download" onClick={actions.diagnosticsExport}>진단 묶음 내보내기</Button>
      </div>
      {logPath !== null && (
        <details className="ome-stage-details">
          <summary>
            <span className="ome-stage-details-chevron"><Icon name="chevron-right" size={16} /></span>
            세부 정보
          </summary>
          <div className="ome-stage-log">
            <span className="ome-stage-log-label">로그 파일</span>
            <p className="ome-mono ome-stage-log-path">{logPath}</p>
          </div>
        </details>
      )}
    </div>
  );
}

function StageContent({ snapshot, actions }: ScreenProps): ReactElement | null {
  const { guest } = snapshot;
  switch (guest.state) {
    case 'stopped':
      return <StoppedStage lastExit={guest.lastExit} actions={actions} />;
    case 'starting':
      return <BootStage guest={guest} onCancel={actions.guestStop} />;
    case 'running':
      if (guest.hosting === 'separateWindow') {
        return (
          <div className="ome-stage-notice">
            <p>운영체제 화면은 별도 창에 있습니다.</p>
            <Button icon="app-window" onClick={actions.guestWindowToFront}>창 앞으로</Button>
          </div>
        );
      }
      // Embedded: Rust places the operating system's window over this frame. Until it does, the
      // frame shows how far the boot has come.
      return guest.hosting === 'none' ? <BootStage guest={guest} /> : null;
    case 'failed':
      return <FailedStage lastExit={guest.lastExit} actions={actions} />;
    case 'stopping':
    case 'restarting':
      return null;
  }
}

function StageStatus({ snapshot }: { readonly snapshot: AppSnapshot }) {
  const { guest, display, input, settings, apps } = snapshot;
  const size = guest.resolution ?? activePreset(display)?.size ?? display.custom?.size ?? null;
  const fit = display.fit === 'fitWindow' ? '창에 맞춤' : '1:1';
  const profile = input.profiles.find((item) => item.id === input.activeId);
  const target = profile?.targetPackage ?? null;
  // Applied automatically: the profile belongs to the app in the foreground.
  const foreground = input.autoApply && target !== null && target === input.foregroundPackage
    ? apps.items.find((app) => app.package === target)?.label ?? target
    : null;
  const fps = settings.showFps && guest.state === 'running' ? guest.fps : null;
  return (
    <footer className="ome-stage-status">
      <span className="ome-stage-status-state">
        <StatusDot tone={GUEST_STATE_TONE[guest.state]} pulse={guest.state === 'starting'} />
        {GUEST_STATE_LABEL[guest.state]}
      </span>
      {guest.androidVersion !== null && <span className="ome-stage-status-item">안드로이드 {guest.androidVersion}</span>}
      <span className="ome-stage-status-item">{size === null ? fit : `${sizeText(size)} · ${fit}`}</span>
      {guest.adbConnected && <span className="ome-stage-status-item">앱 관리 연결됨</span>}
      {profile !== undefined && (
        <span className="ome-stage-status-item">
          {foreground === null ? `입력 프로필: ${profile.name}` : `입력 프로필: ${profile.name} (${foreground})`}
        </span>
      )}
      {input.suspended && (
        <span className="ome-stage-status-paused">
          <Icon name="pause" size={14} strokeWidth={2} />
          {`매핑 일시 중지 (${keyLabel(input.suspendHotkey)})`}
        </span>
      )}
      {fps !== null && <span className="ome-stage-status-fps">{`${String(Math.round(fps))} fps`}</span>}
    </footer>
  );
}

export function StageScreen({ snapshot, actions }: ScreenProps) {
  useGuestKeyboard(snapshot.guest.state === 'running' && !snapshot.input.editing, actions.inputHostKey);
  return (
    <div className="ome-stage">
      <h1 className="ome-visually-hidden">무대</h1>
      <StageToolbar snapshot={snapshot} actions={actions} />
      {snapshot.issue !== null && (
        <div className="ome-stage-issue"><IssueNotice issue={snapshot.issue} /></div>
      )}
      <div className="ome-stage-area">
        <StageFrame onRect={actions.stageRectChanged}>
          <StageContent snapshot={snapshot} actions={actions} />
        </StageFrame>
      </div>
      <StageStatus snapshot={snapshot} />
    </div>
  );
}
