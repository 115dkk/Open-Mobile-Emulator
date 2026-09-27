// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Settings screen (M2-SCREENS.md 6, mockup S6): section titles over setting rows, no cards. Limits
// come from Rust (`memoryMibMin/Max`, `vcpusMax`); every change is sent as a whole `SettingsInput` or
// as its own command. The webview knows no URL and copies no text itself: help pages, notices and
// release notes go through `open_help`, and `복사` names a snapshot value that Rust copies.
import { useEffect, useEffectEvent, useId, useState } from 'react';
import type { ReactNode } from 'react';
import type { ScreenActions, ScreenProps } from '../actions';
import type {
  AppSnapshot, CloseAction, GuestSummary, GuestView, NoticeLevel, SettingsInput, SettingsView, UpdateView,
} from '../contracts';
import {
  Button, Chip, Dialog, Icon, IconButton, IssueNotice, ProgressBar, ScreenHeader, Segmented, Select, SettingRow, Slider,
  StatusDot, Toggle,
} from '../components';
import type { IconName } from '../components';
import { formatBytes, formatGib, formatPercent, formatTimestamp } from '../format';
import { keyLabel } from '../input-profile';
import { clockTime, googleAccountState } from '../presentation';
import type { StatusTone } from '../presentation';
import { ImageCards } from './ImageCards';

const RESTART = '다시 시작해야 적용됩니다.';

function toInput(settings: SettingsView): SettingsInput {
  return {
    memoryMib: settings.memoryMib,
    vcpus: settings.vcpus,
    gpuMode: settings.gpuMode,
    closeAction: settings.closeAction,
    showFps: settings.showFps,
    autoUpdateCheck: settings.autoUpdateCheck,
    adbAccess: settings.adbAccess,
    bindingOverlayDefault: settings.bindingOverlayDefault,
  };
}

interface SectionProps {
  readonly snapshot: AppSnapshot;
  readonly actions: ScreenActions;
  /** Sends every setting with one field changed. */
  readonly save: (changes: Partial<SettingsInput>) => Promise<void>;
}

function Section({ title, children }: { readonly title: string; readonly children: ReactNode }) {
  const id = useId();
  return (
    <section className="ome-settings-section" aria-labelledby={id}>
      <h2 id={id} className="ome-section-title">{title}</h2>
      <div className="ome-settings-rows">{children}</div>
    </section>
  );
}

/** A slider that follows the pointer locally and sends its value on release. */
function DraftSlider({ label, min, max, step, value, format, onCommit }: {
  readonly label: string;
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly value: number;
  readonly format: (value: number) => string;
  readonly onCommit: (value: number) => Promise<void>;
}) {
  const [draft, setDraft] = useState<number | null>(null);
  return (
    <Slider
      label={label}
      min={min}
      max={max}
      step={step}
      value={draft ?? value}
      format={format}
      onChange={setDraft}
      onCommit={(next) => {
        setDraft(null);
        return next === value ? undefined : onCommit(next);
      }}
    />
  );
}

// ---- 운영체제 ----

const CLOSE_OPTIONS: readonly { readonly value: CloseAction; readonly label: string }[] = [
  { value: 'stopGuest', label: '운영체제 끄기' },
  { value: 'minimizeToTray', label: '트레이로 내리기' },
];

function SystemSection({ snapshot, save }: SectionProps) {
  const { settings } = snapshot;
  return (
    <Section title="운영체제">
      <SettingRow label="메모리" help={RESTART}>
        <DraftSlider
          label="메모리"
          min={settings.memoryMibMin}
          max={settings.memoryMibMax}
          step={1024}
          value={settings.memoryMib}
          format={(mib) => `${String(mib / 1024)} GiB`}
          onCommit={(memoryMib) => save({ memoryMib })}
        />
      </SettingRow>
      <SettingRow label="프로세서 코어" help={RESTART}>
        <DraftSlider
          label="프로세서 코어"
          min={2}
          max={settings.vcpusMax}
          step={1}
          value={settings.vcpus}
          format={(count) => `${String(count)}개`}
          onCommit={(vcpus) => save({ vcpus })}
        />
      </SettingRow>
      <SettingRow
        label="그래픽"
        help="하드웨어 가속을 기본적으로 사용합니다. 화면이 검게 나오거나 깨질 때만 소프트웨어 렌더링을 사용하십시오. 소프트웨어 렌더링은 성능을 저하시킵니다."
      >
        <span className="ome-muted" aria-hidden="true">소프트웨어 렌더링</span>
        <Toggle
          label="소프트웨어 렌더링"
          checked={settings.gpuMode === 'software'}
          onChange={(on) => save({ gpuMode: on ? 'software' : 'virgl' })}
        />
      </SettingRow>
      <SettingRow label="창을 닫을 때" help="운영체제가 실행 중일 때 적용됩니다.">
        <Select label="창을 닫을 때" options={CLOSE_OPTIONS} value={settings.closeAction} onChange={(closeAction) => save({ closeAction })} />
      </SettingRow>
    </Section>
  );
}

// ---- 설치된 운영체제 ----

const DISK_SIZES = ['32', '64', '128'] as const;
type DiskSize = (typeof DISK_SIZES)[number];

function guestMeta(guest: GuestSummary): string {
  const parts = [`안드로이드 ${guest.androidVersion}`, formatGib(guest.diskSizeGib)];
  if (guest.lastStartedAt !== null) parts.push(`마지막 실행 ${formatTimestamp(guest.lastStartedAt)}`);
  if (guest.capabilities.probedAt !== null) {
    const confirmed = guest.capabilities.items.filter((item) => item.state === 'available').length;
    parts.push(`확인된 기능 ${String(confirmed)}개`);
  }
  return parts.join(' · ');
}

function NewSystemDialog({ snapshot, onCreate, onCancel }: {
  readonly snapshot: AppSnapshot;
  readonly onCreate: (imageId: string, sizeGib: number) => Promise<void>;
  readonly onCancel: () => void;
}) {
  const { profiles } = snapshot.images;
  const [imageId, setImageId] = useState<string | null>(
    () => profiles.find((image) => image.recommended)?.id ?? profiles[0]?.id ?? null,
  );
  const [size, setSize] = useState<DiskSize | null>(
    () => DISK_SIZES.find((value) => Number(value) === snapshot.wizard.diskSizeGib) ?? null,
  );
  const free = snapshot.wizard.diskFreeBytes;
  return (
    <Dialog
      open
      title="새 운영체제를 설치합니다."
      confirmLabel="설치"
      confirmDisabled={imageId === null || size === null}
      onConfirm={() => (imageId === null || size === null ? undefined : onCreate(imageId, Number(size)))}
      onCancel={onCancel}
    >
      <ImageCards images={profiles} selectedId={imageId} disabled={false} onSelect={setImageId} />
      <div className="ome-labelled">
        <span className="ome-group-label">디스크 크기</span>
        <Segmented
          label="디스크 크기"
          options={DISK_SIZES.map((value) => ({ value, label: formatGib(Number(value)) }))}
          value={size}
          onChange={setSize}
        />
        {free !== null && <p className="ome-caption">여유 공간 {formatBytes(free)}</p>}
      </div>
    </Dialog>
  );
}

type GuestDialog = { readonly kind: 'reinstall' | 'delete'; readonly name: string } | { readonly kind: 'create' };

function SystemsSection({ snapshot, actions }: SectionProps) {
  const { images, guest } = snapshot;
  const [dialog, setDialog] = useState<GuestDialog | null>(null);
  const close = () => { setDialog(null); };
  // The system that is on (or on its way) keeps its disk; Rust refuses the rest anyway.
  const busy = guest.state !== 'stopped' && guest.state !== 'failed' ? images.activeGuest : null;
  const startOptions = images.guests.map((item) => ({ value: item.name, label: item.name }));
  if (images.activeGuest === null) startOptions.unshift({ value: '', label: '없음' });

  return (
    <Section title="설치된 운영체제">
      {images.guests.map((item) => (
        <SettingRow key={item.name} label={item.name} help={guestMeta(item)}>
          {item.name !== busy && (<>
            <Button variant="ghost" onClick={() => { setDialog({ kind: 'reinstall', name: item.name }); }}>다시 설치</Button>
            <Button variant="danger" onClick={() => { setDialog({ kind: 'delete', name: item.name }); }}>삭제</Button>
          </>)}
        </SettingRow>
      ))}
      {images.guests.length > 0 && (
        <SettingRow label="시작할 운영체제">
          <Select
            label="시작할 운영체제"
            options={startOptions}
            value={images.activeGuest ?? ''}
            onChange={(name) => (name === '' ? undefined : actions.guestSelect(name))}
          />
        </SettingRow>
      )}
      <SettingRow label="운영체제 이미지" help="새 운영체제 이미지는 앱 업데이트로 추가됩니다.">
        <Button icon="plus" disabled={images.profiles.length === 0} onClick={() => { setDialog({ kind: 'create' }); }}>
          새 운영체제 설치
        </Button>
      </SettingRow>
      {dialog?.kind === 'create' && (
        <NewSystemDialog
          snapshot={snapshot}
          onCreate={(imageId, sizeGib) => {
            setDialog(null);
            return actions.guestCreate(imageId, sizeGib);
          }}
          onCancel={close}
        />
      )}
      <Dialog
        open={dialog?.kind === 'reinstall'}
        title={dialog?.kind === 'reinstall' ? `${dialog.name} 운영체제의 디스크를 지우고 같은 이미지로 처음부터 설치합니다.` : ''}
        tone="danger"
        confirmLabel="다시 설치"
        onConfirm={() => {
          if (dialog?.kind !== 'reinstall') return undefined;
          setDialog(null);
          return actions.guestReinstall(dialog.name);
        }}
        onCancel={close}
      />
      <Dialog
        open={dialog?.kind === 'delete'}
        title={dialog?.kind === 'delete' ? `${dialog.name} 운영체제를 디스크 파일까지 삭제합니다.` : ''}
        tone="danger"
        confirmLabel="삭제"
        onConfirm={() => {
          if (dialog?.kind !== 'delete') return undefined;
          setDialog(null);
          return actions.guestDelete(dialog.name);
        }}
        onCancel={close}
      />
    </Section>
  );
}

// ---- Google 계정: while running, in one of four states (M2-SCREENS.md 6) ----

/** The setting row without a label: a lamp beside a sentence on the left, controls on the right. */
function LampRow({ tone, sentence, help, children }: {
  readonly tone: StatusTone;
  readonly sentence: string;
  readonly help?: ReactNode;
  readonly children?: ReactNode;
}) {
  return (
    <div className="ome-setting">
      <div className="ome-setting-row">
        <div className="ome-settings-status">
          <span className="ome-settings-lamp"><StatusDot tone={tone} /></span>
          <div className="ome-setting-text">
            <p>{sentence}</p>
            {help !== undefined && <p className="ome-setting-help">{help}</p>}
          </div>
        </div>
        {children !== undefined && <div className="ome-setting-control">{children}</div>}
      </div>
    </div>
  );
}

const ADD_ACCOUNT_FALLBACK = '운영체제의 설정 → 계정에서 추가하십시오.';

/** `계정 추가 화면 열기` where the system can open it; otherwise the fallback line takes its place. */
function addAccount(guest: GuestView, actions: ScreenActions): { readonly help?: string; readonly control?: ReactNode } {
  if (!guest.addAccountSupported) return { help: ADD_ACCOUNT_FALLBACK };
  return { control: <Button icon="plus" onClick={actions.googleAccountAddOpen}>계정 추가 화면 열기</Button> };
}

function GoogleState({ guest, actions }: { readonly guest: GuestView; readonly actions: ScreenActions }) {
  switch (googleAccountState(guest)) {
    case 'reading':
      return <LampRow tone="muted" sentence="기기 ID를 읽는 중입니다. 운영체제가 Google 서버와 첫 교신을 마치면 나타납니다." />;
    case 'id':
      return (
        <SettingRow
          label="기기 ID"
          help={<Chip><span className="ome-mono ome-settings-value">{guest.deviceId}</span></Chip>}
        >
          <IconButton icon="copy" label="복사" onClick={() => actions.copyToClipboard('deviceId')} />
          <Button icon="external-link" onClick={actions.openRegistrationPage}>등록 페이지 열기</Button>
          <Button variant="ghost" icon="external-link" onClick={() => actions.openHelp('googleAccount')}>도움말</Button>
        </SettingRow>
      );
    case 'registered': {
      // Two buttons do not fit one control column: each goes beside the line it answers.
      const add = addAccount(guest, actions);
      return (<>
        <LampRow tone="warning" sentence="등록한 뒤 약 10분이 지나면 계정을 추가할 수 있습니다." help={add.help}>
          {add.control}
        </LampRow>
        <div className="ome-setting">
          <div className="ome-setting-row">
            <div className="ome-setting-text">
              <p>{`${clockTime(guest.registrationOpenedAt ?? '')}에 열었습니다`}</p>
            </div>
            <div className="ome-setting-control">
              <Button variant="ghost" icon="external-link" onClick={actions.openRegistrationPage}>등록 페이지 다시 열기</Button>
            </div>
          </div>
        </div>
      </>);
    }
    case 'account': {
      const add = addAccount(guest, actions);
      return (
        <LampRow
          tone="success"
          sentence={`Google 계정 ${String(guest.googleAccounts ?? 0)}개가 로그인되어 있습니다.`}
          help={add.help}
        >
          {add.control}
        </LampRow>
      );
    }
  }
}

function GoogleSection({ snapshot, actions }: SectionProps) {
  const { guest } = snapshot;
  if (guest.state !== 'running') return null;
  return (
    <Section title="Google 계정">
      <p className="ome-settings-lead">
        이 운영체제는 Google 인증 기기가 아닙니다. Google 계정으로 로그인하려면 기기 ID를 Google에 한 번 등록해야 합니다.
      </p>
      <GoogleState guest={guest} actions={actions} />
      <p className="ome-settings-note">등록해도 Play 스토어와 인앱 결제는 보장되지 않습니다.</p>
    </Section>
  );
}

// ---- 입력 ----

/** Shows the hotkey; after a click the next key press (other than Esc) becomes the hotkey. */
function HotkeyButton({ code, onSet }: { readonly code: string; readonly onSet: (code: string) => Promise<void> }) {
  const [capturing, setCapturing] = useState(false);
  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (event.code === '' || event.repeat) return;
    event.preventDefault();
    setCapturing(false);
    if (event.code !== 'Escape' && event.code !== code) void onSet(event.code);
  });
  useEffect(() => {
    if (!capturing) return undefined;
    const listener = (event: KeyboardEvent) => { onKey(event); };
    document.addEventListener('keydown', listener);
    return () => { document.removeEventListener('keydown', listener); };
  }, [capturing]);
  return (
    <Button className="ome-settings-hotkey" onClick={() => { setCapturing(!capturing); }}>
      {capturing ? '키를 누르십시오' : keyLabel(code)}
    </Button>
  );
}

function InputSection({ snapshot, actions, save }: SectionProps) {
  const { input, settings } = snapshot;
  return (
    <Section title="입력">
      <SettingRow label="매핑 일시 중지 단축키">
        <HotkeyButton code={input.suspendHotkey} onSet={actions.inputSuspendHotkeySet} />
      </SettingRow>
      <SettingRow label="매핑 표지 기본 표시">
        <Toggle
          label="매핑 표지 기본 표시"
          checked={settings.bindingOverlayDefault}
          onChange={(bindingOverlayDefault) => save({ bindingOverlayDefault })}
        />
      </SettingRow>
      <SettingRow label="게임별 자동 적용">
        <Toggle label="게임별 자동 적용" checked={input.autoApply} onChange={actions.inputAutoApplySet} />
      </SettingRow>
    </Section>
  );
}

// ---- 고급 ----

function AdvancedSection({ snapshot, actions, save }: SectionProps) {
  const { guest, settings } = snapshot;
  const network = settings.adbAccess === 'network';
  return (
    <Section title="고급">
      <SettingRow
        label="adb 연결"
        help="adb로 연결한 프로그램은 운영체제 안의 모든 앱과 데이터를 다룰 수 있습니다. 신뢰하는 프로그램만 연결하십시오."
      >
        {guest.adbAddress !== null && <span className="ome-mono ome-settings-value">{guest.adbAddress}</span>}
        {guest.adbAddress !== null && (
          <IconButton icon="copy" label="복사" onClick={() => actions.copyToClipboard('adbAddress')} />
        )}
        <Button variant="ghost" icon="external-link" onClick={() => actions.openHelp('adbSecurity')}>도움말</Button>
      </SettingRow>
      <SettingRow
        label="다른 PC에서 연결 허용"
        help={RESTART}
        below={network ? (
          <div className="ome-caution" role="note">
            <span className="ome-caution-icon"><Icon name="alert-triangle" size={18} strokeWidth={2} /></span>
            <p>같은 네트워크의 누구나 이 운영체제에 접근할 수 있습니다.</p>
          </div>
        ) : undefined}
      >
        <Toggle
          label="다른 PC에서 연결 허용"
          checked={network}
          onChange={(on) => save({ adbAccess: on ? 'network' : 'localhost' })}
        />
      </SettingRow>
      {guest.rootEnabled !== null && (
        <SettingRow
          label="루트 권한"
          help="앱이 루트 권한을 요청할 수 있게 합니다. 일부 게임과 결제 기능은 루트가 켜진 기기에서 동작하지 않습니다."
        >
          <Toggle
            label="루트 권한"
            checked={guest.rootEnabled}
            disabled={guest.state !== 'running'}
            onChange={actions.guestRootSet}
          />
        </SettingRow>
      )}
      <SettingRow label="fps 표시">
        <Toggle label="fps 표시" checked={settings.showFps} onChange={(showFps) => save({ showFps })} />
      </SettingRow>
    </Section>
  );
}

// ---- 저장 위치 ----

function StorageSection({ snapshot, actions }: SectionProps) {
  const { settings } = snapshot;
  return (
    <Section title="저장 위치">
      <SettingRow
        label="경로"
        help="운영체제 디스크와 로그, 스크린샷이 있는 폴더입니다."
        below={settings.homeDir === '' ? undefined : <span className="ome-mono ome-settings-value">{settings.homeDir}</span>}
      >
        {settings.diskUsageBytes !== null && (
          <span className="ome-muted">사용 중 <span className="ome-readout">{formatBytes(settings.diskUsageBytes)}</span></span>
        )}
        {settings.homeDir !== '' && <Button icon="folder-open" onClick={actions.openHomeFolder}>폴더 열기</Button>}
      </SettingRow>
    </Section>
  );
}

// ---- 업데이트 ----

function updateHelp(update: UpdateView): ReactNode {
  const { state } = update;
  switch (state.kind) {
    case 'checking': return '새 버전을 확인하는 중입니다.';
    case 'upToDate': return `마지막 확인 ${formatTimestamp(state.checkedAt)}. 새 버전이 없습니다.`;
    case 'failed': return (
      <span className="ome-settings-failed">
        <span>{state.issue.message}</span>
        {state.issue.nextAction !== null && <span className="ome-settings-failed-next">{state.issue.nextAction}</span>}
      </span>
    );
    case 'idle':
    case 'available':
    case 'downloading':
    case 'readyToInstall':
      return undefined;
  }
}

function NewVersionRow({ update, actions }: { readonly update: UpdateView; readonly actions: ScreenActions }) {
  const { state } = update;
  switch (state.kind) {
    case 'available':
      // The version sits on the help line so the fixed control column holds both buttons. Rust opens
      // `notesUrl`; without one there is nothing to open and no button.
      return (
        <SettingRow label="새 버전" help={<span className="ome-readout">{state.version}</span>}>
          {state.notesUrl !== null && (
            <Button variant="ghost" icon="external-link" onClick={() => actions.openHelp('releaseNotes')}>릴리스 노트</Button>
          )}
          <Button variant="primary" icon="download" onClick={actions.updateInstall}>다운로드 후 설치</Button>
        </SettingRow>
      );
    case 'downloading': {
      const { doneBytes, totalBytes } = state.progress;
      const ratio = totalBytes !== null && totalBytes > 0 ? doneBytes / totalBytes : null;
      return (
        <SettingRow label="새 버전" help="다운로드 중">
          <span className="ome-settings-progress"><ProgressBar value={ratio} label="업데이트 다운로드 진행률" /></span>
          {ratio !== null && <span className="ome-readout">{formatPercent(ratio)}</span>}
        </SettingRow>
      );
    }
    case 'readyToInstall':
      return (
        <SettingRow label="새 버전" help={<span className="ome-readout">{state.version}</span>}>
          <Button variant="primary" onClick={actions.updateInstall}>설치</Button>
        </SettingRow>
      );
    case 'idle':
    case 'checking':
    case 'upToDate':
    case 'failed':
      return null;
  }
}

function UpdateSection({ snapshot, actions, save }: SectionProps) {
  const { update, settings } = snapshot;
  const help = updateHelp(update);
  return (
    <Section title="업데이트">
      <SettingRow label="현재 버전" help={help}>
        <span className="ome-readout">{update.currentVersion}</span>
        <Button icon="refresh" disabled={update.state.kind === 'checking'} onClick={actions.updateCheck}>업데이트 확인</Button>
      </SettingRow>
      <NewVersionRow update={update} actions={actions} />
      <SettingRow label="자동 확인" help="앱을 열 때 새 버전이 있는지 확인합니다.">
        <Toggle label="자동 확인" checked={settings.autoUpdateCheck} onChange={(autoUpdateCheck) => save({ autoUpdateCheck })} />
      </SettingRow>
    </Section>
  );
}

// ---- 진단 ----

const NOTICE_ICON: Readonly<Record<NoticeLevel, IconName | null>> = {
  info: null,
  warning: 'alert-triangle',
  error: 'alert-circle',
};

function DiagnosticsSection({ snapshot, actions }: SectionProps) {
  const recent = snapshot.notices.slice(-5);
  return (
    <Section title="진단">
      <SettingRow label="진단 묶음" help="로그와 환경 정보를 파일 하나로 묶습니다.">
        <Button variant="ghost" icon="folder-open" onClick={actions.openLogsFolder}>로그 폴더 열기</Button>
        <Button icon="download" onClick={actions.diagnosticsExport}>진단 묶음 내보내기</Button>
      </SettingRow>
      {recent.length > 0 && (
        <div className="ome-settings-events">
          <h3 className="ome-group-label">최근 이벤트</h3>
          <ul className="ome-settings-event-list">
            {recent.map((notice, index) => {
              const icon = NOTICE_ICON[notice.level];
              return (
                <li key={`${notice.at}-${String(index)}`} className={`ome-settings-event ome-settings-event-${notice.level}`}>
                  <span className="ome-mono ome-settings-event-time">{formatTimestamp(notice.at)}</span>
                  <span className="ome-settings-event-mark" aria-hidden="true">
                    {icon !== null && <Icon name={icon} size={16} strokeWidth={2} />}
                  </span>
                  <span className="ome-settings-event-text">{notice.message}</span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </Section>
  );
}

// ---- 정보 ----

function AboutSection({ snapshot, actions }: SectionProps) {
  return (
    <Section title="정보">
      <SettingRow label="Open Mobile Emulator" help={`버전 ${snapshot.productVersion} · GPL-2.0-or-later`}>
        <Button variant="ghost" icon="external-link" onClick={() => actions.openHelp('thirdPartyNotices')}>제3자 고지</Button>
      </SettingRow>
      <SettingRow label="가상 머신 소스 코드" help="이 앱에 포함된 가상 머신 프로그램의 소스 코드를 받을 수 있습니다.">
        <Button variant="ghost" icon="external-link" onClick={() => actions.openHelp('qemuSource')}>소스 코드 받기</Button>
      </SettingRow>
    </Section>
  );
}

export function SettingsScreen({ snapshot, actions }: ScreenProps) {
  const save = (changes: Partial<SettingsInput>) => actions.settingsSave({ ...toInput(snapshot.settings), ...changes });
  const props: SectionProps = { snapshot, actions, save };
  return (
    <>
      <ScreenHeader title="설정" />
      <div className="ome-settings">
        {snapshot.issue !== null && <IssueNotice issue={snapshot.issue} />}
        <SystemSection {...props} />
        <SystemsSection {...props} />
        <GoogleSection {...props} />
        <InputSection {...props} />
        <AdvancedSection {...props} />
        <StorageSection {...props} />
        <UpdateSection {...props} />
        <DiagnosticsSection {...props} />
        <AboutSection {...props} />
      </div>
    </>
  );
}
