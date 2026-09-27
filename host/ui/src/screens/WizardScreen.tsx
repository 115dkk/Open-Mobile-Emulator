// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// First-run wizard (M2-SCREENS.md 1). Rust owns the step order and whether each button may act;
// this screen reads `wizard.step`, `wizard.canContinue` and `wizard.canSkip` and sends commands.
// `나중에 하기` on every step, and S1.2's way out, send `wizard_defer`: Rust keeps the step and opens
// the stage.
import { useState } from 'react';
import type { ReactNode } from 'react';
import type { ScreenActions, ScreenProps } from '../actions';
import type {
  AppIssue, AppSnapshot, GuestImageSummary, GuestView, TransferProgress, TransferStage, WizardStep,
} from '../contracts';
import {
  Button, Icon, IssueNotice, ProgressBar, Segmented, StageFrame, StatusDot, StepList,
} from '../components';
import type { StepItem } from '../components';
import {
  byteUnit, formatBytes, formatBytesIn, formatDuration, formatGib, formatPercent, formatRate, formatTimestamp,
} from '../format';
import {
  CAPABILITY_LABEL, FAILURE_NEXT_STEP, bootSteps, hostLines, userFacingCapabilities,
} from '../presentation';
import type { BootStep, StepState } from '../presentation';
import { ImageCards } from './ImageCards';
import { useEscapeKey } from './use-escape-key';
import { ESC_LINE, WhpxConsentBody } from './WhpxConsentBody';

const STEPS: readonly { readonly id: Exclude<WizardStep, 'done'>; readonly name: string }[] = [
  { id: 'hostCheck', name: '호스트 점검' },
  { id: 'whpxConsent', name: '하이퍼바이저' },
  { id: 'rebootPending', name: '다시 시작' },
  { id: 'artifactDownload', name: '운영체제 이미지' },
  { id: 'guestInstall', name: '운영체제 설치' },
  { id: 'firstBoot', name: '첫 부팅' },
  { id: 'appInstall', name: '앱 설치' },
];

function WizardHeader({ step }: { readonly step: WizardStep }) {
  const index = step === 'done' ? STEPS.length : STEPS.findIndex((item) => item.id === step);
  const name = STEPS[index]?.name ?? '앱 설치';
  const position = Math.min(index + 1, STEPS.length);
  return (
    <header className="ome-wizard-header">
      <div className="ome-wizard-dots" aria-hidden="true">
        {STEPS.map((item, dot) => (
          <span
            key={item.id}
            className={dot < index ? 'ome-wizard-dot ome-wizard-dot-done'
              : dot === index ? 'ome-wizard-dot ome-wizard-dot-current' : 'ome-wizard-dot'}
          />
        ))}
      </div>
      <p className="ome-wizard-position">
        <span className="ome-wizard-count">{position} / {STEPS.length}</span>
        <span>{name}</span>
      </p>
    </header>
  );
}

interface LayoutProps {
  readonly issue: AppIssue | null;
  readonly children: ReactNode;
  readonly footer?: ReactNode;
  /** `나중에 하기` at the footer's start: closes the wizard and keeps this step (every step but `done`). */
  readonly onDefer?: (() => Promise<void>) | undefined;
  /** Stage on the left, guidance on the right (S1.5 after the disk exists, S1.6). */
  readonly wide?: boolean | undefined;
}

function WizardLayout({ issue, children, footer, onDefer, wide = false }: LayoutProps) {
  return (
    <>
      <main className={wide ? 'ome-wizard-body ome-wizard-body-wide' : 'ome-wizard-body'}>
        <div className={wide ? 'ome-wizard-wide' : 'ome-wizard-column'}>
          {issue !== null && <IssueNotice issue={issue} />}
          {wide ? <div className="ome-wizard-wide-row">{children}</div> : children}
        </div>
      </main>
      {(footer !== undefined || onDefer !== undefined) && (
        <footer className="ome-wizard-footer">
          {onDefer !== undefined && <Button size="large" variant="ghost" onClick={onDefer}>나중에 하기</Button>}
          {footer !== undefined && <div className="ome-wizard-footer-actions">{footer}</div>}
        </footer>
      )}
    </>
  );
}

interface StepProps {
  readonly snapshot: AppSnapshot;
  readonly actions: ScreenActions;
}

function HostCheckStep({ snapshot, actions }: StepProps) {
  const { host, wizard } = snapshot;
  const lines = hostLines(host.rows);
  return (
    <WizardLayout
      issue={snapshot.issue}
      onDefer={actions.wizardDefer}
      footer={<>
        <Button size="large" icon="refresh" onClick={actions.hostCheckRefresh}>다시 확인</Button>
        <Button size="large" variant="primary" disabled={!wizard.canContinue} onClick={actions.wizardContinue}>계속</Button>
      </>}
    >
      <h1 className="ome-page-title">호스트 점검</h1>
      {lines.length > 0 && (
        <p className="ome-verdict">
          {host.ready
            ? '이 PC에서는 Open Mobile Emulator를 사용할 수 있습니다.'
            : '이 PC에서는 Open Mobile Emulator를 사용할 수 없습니다.'}
        </p>
      )}
      {lines.length > 0 && (
        <ul className="ome-host-list">
          {lines.map((line) => (
            <li key={line.label} className="ome-host-row">
              <span className="ome-host-dot"><StatusDot tone={line.tone} /></span>
              <span className="ome-host-label">{line.label}</span>
              <span className="ome-host-detail">{line.detail}</span>
            </li>
          ))}
        </ul>
      )}
      {host.inspectedAt !== null && <p className="ome-caption">{formatTimestamp(host.inspectedAt)} 확인</p>}
    </WizardLayout>
  );
}

function WhpxConsentStep({ snapshot, actions }: StepProps) {
  // Leaving consent is always possible: the wizard stays at this step and the stage reports the
  // hypervisor blocker, so the way out does not depend on `canSkip`.
  useEscapeKey(() => { void actions.wizardDefer(); });
  return (
    <WizardLayout
      issue={snapshot.issue}
      onDefer={actions.wizardDefer}
      footer={<>
        <p className="ome-footer-note">{ESC_LINE}</p>
        <Button size="large" onClick={actions.wizardDefer}>지금은 건너뛰기</Button>
        <Button size="large" variant="primary" onClick={actions.whpxEnable}>활성화</Button>
      </>}
    >
      <WhpxConsentBody />
    </WizardLayout>
  );
}

function RebootPendingStep({ snapshot, actions }: StepProps) {
  return (
    <WizardLayout
      issue={snapshot.issue}
      onDefer={actions.wizardDefer}
      footer={<Button size="large" variant="primary" onClick={actions.appQuit}>닫기</Button>}
    >
      <h1 className="ome-page-title">하이퍼바이저를 활성화했습니다.</h1>
      <p className="ome-lead">PC를 다시 시작한 뒤 앱을 다시 열면 이어서 진행합니다.</p>
    </WizardLayout>
  );
}

const ACTIVE_TRANSFER: readonly TransferStage[] = ['waiting', 'transferring', 'verifying'];

function transferSteps(stage: TransferStage): StepItem[] {
  const order: StepState[] = stage === 'verified' ? ['done', 'done', 'done']
    : stage === 'verifying' ? ['done', 'active', 'pending']
      : ['active', 'pending', 'pending'];
  return [
    { id: 'transfer', label: '다운로드 중', state: order[0] ?? 'pending' },
    { id: 'verify', label: '무결성 확인', state: order[1] ?? 'pending' },
    { id: 'complete', label: '완료', state: order[2] ?? 'pending' },
  ];
}

function sourceName(image: GuestImageSummary): string | null {
  return image.distribution === 'selfBuilt' ? null : 'SourceForge 공식 프로젝트';
}

function TransferPanel({ download }: { readonly download: TransferProgress }) {
  const total = download.totalBytes;
  const ratio = download.stage === 'verified' ? 1 : total !== null && total > 0 ? download.doneBytes / total : null;
  const remaining = total !== null ? Math.max(0, total - download.doneBytes) : null;
  const moving = download.stage === 'transferring';
  // Done, total and remaining share the total's unit so the numbers compare at a glance.
  const unit = byteUnit(total ?? download.doneBytes);
  return (
    <div className="ome-transfer">
      <div className="ome-transfer-head">
        <span className="ome-readout">
          {total !== null
            ? `${formatBytesIn(download.doneBytes, unit)} / ${formatBytesIn(total, unit)}`
            : formatBytesIn(download.doneBytes, unit)}
        </span>
        {ratio !== null && <span className="ome-readout ome-transfer-percent">{formatPercent(ratio)}</span>}
      </div>
      <ProgressBar value={ratio} label="다운로드 진행률" />
      {moving && (download.bytesPerSecond !== null || remaining !== null) && (
        <div className="ome-transfer-meta">
          {download.bytesPerSecond !== null && (
            <span>속도 <span className="ome-readout">{formatRate(download.bytesPerSecond)}</span></span>
          )}
          {remaining !== null && (
            <span>
              남은 양 <span className="ome-readout">{formatBytesIn(remaining, unit)}</span>
              {download.bytesPerSecond !== null && download.bytesPerSecond > 0 && (
                <>, 약 <span className="ome-readout">{formatDuration(remaining / download.bytesPerSecond)}</span></>
              )}
            </span>
          )}
        </div>
      )}
    </div>
  );
}

function ArtifactDownloadStep({ snapshot, actions }: StepProps) {
  const { wizard, images } = snapshot;
  const download = wizard.download;
  const active = download !== null && ACTIVE_TRANSFER.includes(download.stage);
  const showProgress = download !== null && (active || download.stage === 'verified');
  const selected = images.profiles.find((image) => image.id === wizard.imageId) ?? null;
  const source = selected === null ? null : sourceName(selected);

  let footer: ReactNode;
  if (active) {
    footer = (<>
      <Button size="large" onClick={actions.artifactDownloadCancel}>취소</Button>
      <Button size="large" variant="primary" disabled={!wizard.canContinue} onClick={actions.wizardContinue}>다음</Button>
    </>);
  } else if (wizard.canContinue || download?.stage === 'verified') {
    footer = <Button size="large" variant="primary" disabled={!wizard.canContinue} onClick={actions.wizardContinue}>다음</Button>;
  } else {
    footer = (
      <Button size="large" variant="primary" icon="download" disabled={wizard.imageId === null} onClick={actions.artifactDownloadStart}>
        다운로드
      </Button>
    );
  }

  return (
    <WizardLayout issue={snapshot.issue} onDefer={actions.wizardDefer} footer={footer}>
      <h1 className="ome-page-title">운영체제 이미지 다운로드</h1>
      <p className="ome-lead">운영체제 이미지를 공식 배포처에서 다운로드합니다. 연결이 끊겨도 다시 이어받을 수 있습니다.</p>
      {images.profiles.length > 0 && (
        <ImageCards
          images={images.profiles}
          selectedId={wizard.imageId}
          disabled={active}
          onSelect={(id) => { void actions.guestImageSelect(id); }}
        />
      )}
      {(download !== null || selected !== null) && (
        <dl className="ome-facts">
          {download !== null && download.label !== '' && (
            <div className="ome-fact"><dt>파일</dt><dd className="ome-mono">{download.label}</dd></div>
          )}
          {source !== null && <div className="ome-fact"><dt>출처</dt><dd>{source}</dd></div>}
          {selected !== null && selected.sizeBytes !== null && (
            <div className="ome-fact"><dt>크기</dt><dd className="ome-readout">{formatBytes(selected.sizeBytes)}</dd></div>
          )}
        </dl>
      )}
      {showProgress && <TransferPanel download={download} />}
      {showProgress && (
        <div className="ome-labelled">
          <span className="ome-group-label" aria-hidden="true">진행</span>
          <StepList orientation="horizontal" label="진행" items={transferSteps(download.stage)} />
        </div>
      )}
    </WizardLayout>
  );
}

const DISK_SIZES = ['32', '64', '128'] as const;
type DiskSize = (typeof DISK_SIZES)[number];

function toDiskSize(gib: number): DiskSize | null {
  return DISK_SIZES.find((size) => Number(size) === gib) ?? null;
}

function SeparateWindowNotice({ guest, actions }: { readonly guest: GuestView; readonly actions: ScreenActions }) {
  if (guest.hosting !== 'separateWindow') return null;
  return (
    <div className="ome-stage-notice">
      <p>운영체제 화면은 별도 창에 있습니다.</p>
      <Button icon="app-window" onClick={actions.guestWindowToFront}>창 앞으로</Button>
    </div>
  );
}

function GuestInstallStep({ snapshot, actions }: StepProps) {
  const { wizard, images, guest } = snapshot;
  const [size, setSize] = useState<DiskSize | null>(() => toDiskSize(wizard.diskSizeGib));
  const imageId = wizard.imageId;

  if (images.activeGuest === null) {
    return (
      <WizardLayout
        issue={snapshot.issue}
        onDefer={actions.wizardDefer}
        footer={
          <Button
            size="large"
            variant="primary"
            icon="hard-drive"
            disabled={imageId === null || size === null}
            onClick={() => {
              if (imageId !== null && size !== null) return actions.guestCreate(imageId, Number(size));
              return undefined;
            }}
          >
            디스크 만들기
          </Button>
        }
      >
        <h1 className="ome-page-title">운영체제 설치</h1>
        <div className="ome-labelled">
          <span className="ome-group-label">디스크 크기</span>
          <Segmented
            label="디스크 크기"
            options={DISK_SIZES.map((value) => ({ value, label: formatGib(Number(value)) }))}
            value={size}
            onChange={setSize}
          />
          {wizard.diskFreeBytes !== null && (
            <p className="ome-caption">여유 공간 {formatBytes(wizard.diskFreeBytes)}</p>
          )}
        </div>
      </WizardLayout>
    );
  }

  return (
    <WizardLayout
      wide
      issue={snapshot.issue}
      onDefer={actions.wizardDefer}
      footer={<Button size="large" variant="primary" disabled={!wizard.canContinue} onClick={actions.wizardContinue}>설치 완료</Button>}
    >
      <StageFrame onRect={actions.stageRectChanged}>
        <SeparateWindowNotice guest={guest} actions={actions} />
      </StageFrame>
      <aside className="ome-wizard-side">
        <h1 className="ome-page-title">운영체제 설치</h1>
        {wizard.installGuide.length > 0 && (
          <ol className="ome-guide">
            {wizard.installGuide.map((line, index) => (
              <li key={`${String(index)}-${line}`} className="ome-guide-item">
                <span className="ome-guide-number">{index + 1}</span>
                <span>{line}</span>
              </li>
            ))}
          </ol>
        )}
      </aside>
    </WizardLayout>
  );
}

function CapabilityList({ guest }: { readonly guest: GuestView }) {
  const items = userFacingCapabilities(guest.capabilities.items);
  if (items.length === 0) return null;
  return (
    <ul className="ome-capabilities">
      {items.map((item) => (
        <li key={item.id} className={`ome-capability ome-capability-${item.state}`}>
          <span className="ome-capability-mark" aria-hidden="true">
            {item.state === 'available' ? <Icon name="check" size={16} strokeWidth={2} /> : <StatusDot tone="muted" />}
          </span>
          <span>{CAPABILITY_LABEL[item.id]}</span>
          <span className="ome-visually-hidden">
            {item.state === 'available' ? '사용 가능' : item.state === 'unavailable' ? '사용 불가' : '확인 전'}
          </span>
        </li>
      ))}
    </ul>
  );
}

function FirstBootStep({ snapshot, actions }: StepProps) {
  const { guest, wizard } = snapshot;
  // The failed symbol and the noun phrase name the failure; the detail says what to do (DESIGN.md 9).
  const detail = (step: BootStep): ReactNode => {
    if (step.state === 'failed') return FAILURE_NEXT_STEP;
    if (step.id === 'probe') return <CapabilityList guest={guest} />;
    return undefined;
  };
  const steps: StepItem[] = bootSteps(guest, true).map((step) => ({
    id: step.id,
    label: step.label,
    state: step.state,
    children: detail(step),
  }));
  const failed = guest.state === 'failed';
  const running = guest.state !== 'stopped' && !failed;
  return (
    <WizardLayout
      wide
      issue={snapshot.issue}
      onDefer={actions.wizardDefer}
      footer={(running || failed || wizard.canContinue) ? <>
        {running && <Button size="large" onClick={actions.guestStop}>취소</Button>}
        {/* The virtual machine has ended; the supervisor starts again from Failed. */}
        {failed && <Button size="large" icon="download" onClick={actions.diagnosticsExport}>진단 묶음 내보내기</Button>}
        {failed && <Button size="large" variant="primary" icon="refresh" onClick={actions.guestStart}>다시 시작</Button>}
        {wizard.canContinue && <Button size="large" variant="primary" onClick={actions.wizardContinue}>다음</Button>}
      </> : undefined}
    >
      <StageFrame onRect={actions.stageRectChanged}>
        <SeparateWindowNotice guest={guest} actions={actions} />
      </StageFrame>
      <aside className="ome-wizard-side">
        <h1 className="ome-page-title">첫 부팅</h1>
        <StepList label="첫 부팅 진행" items={steps} />
        {guest.bootCompleted && <p className="ome-lead">미디어 볼륨과 기본 표시 프리셋을 적용했습니다.</p>}
      </aside>
    </WizardLayout>
  );
}

function AppInstallStep({ snapshot, actions }: StepProps) {
  const { apps, wizard } = snapshot;
  const install = apps.install;
  const installing = install !== null && ACTIVE_TRANSFER.includes(install.stage);
  const ratio = install !== null && install.totalBytes !== null && install.totalBytes > 0
    ? install.doneBytes / install.totalBytes : null;
  return (
    <WizardLayout
      issue={snapshot.issue}
      onDefer={actions.wizardDefer}
      footer={<>
        {wizard.canSkip && <Button size="large" onClick={actions.wizardSkip}>건너뛰기</Button>}
        <Button size="large" variant="primary" disabled={!wizard.canContinue} onClick={actions.wizardContinue}>완료</Button>
      </>}
    >
      <h1 className="ome-page-title">앱 설치</h1>
      <p className="ome-lead">앱 파일을 지금 설치할 수 있습니다.</p>
      <div className="ome-drop-zone">
        <span className="ome-drop-zone-icon"><Icon name="file-up" size={32} /></span>
        <p className="ome-drop-zone-text">APK, XAPK, APKS 파일을 여기에 놓습니다.</p>
        <p className="ome-caption">여러 파일로 나뉜 앱은 묶음 파일 하나를 놓으면 됩니다.</p>
        <Button onClick={actions.appInstallPick}>파일 고르기</Button>
      </div>
      {installing && (
        <div className="ome-install-line">
          <StatusDot tone="accent" />
          <span><span className="ome-mono">{install.label}</span> 설치 중</span>
          <div className="ome-install-bar"><ProgressBar value={ratio} label="설치 진행률" size="thin" /></div>
          {ratio !== null && <span className="ome-readout">{formatPercent(ratio)}</span>}
        </div>
      )}
      {apps.items.length > 0 && (
        <ul className="ome-install-results">
          {apps.items.map((app) => (
            <li key={app.package} className="ome-install-result">
              <span className="ome-install-result-mark"><Icon name="check" size={18} strokeWidth={2} /></span>
              <span className="ome-install-result-name">{app.label}</span>
              <span className="ome-mono ome-muted">{app.package}</span>
              <span className="ome-install-result-state">설치 완료</span>
            </li>
          ))}
        </ul>
      )}
      <p className="ome-caption">앱 화면에서도 추가로 설치할 수 있습니다.</p>
    </WizardLayout>
  );
}

function StepBody({ snapshot, actions }: StepProps) {
  switch (snapshot.wizard.step) {
    case 'hostCheck': return <HostCheckStep snapshot={snapshot} actions={actions} />;
    case 'whpxConsent': return <WhpxConsentStep snapshot={snapshot} actions={actions} />;
    case 'rebootPending': return <RebootPendingStep snapshot={snapshot} actions={actions} />;
    case 'artifactDownload': return <ArtifactDownloadStep snapshot={snapshot} actions={actions} />;
    case 'guestInstall': return <GuestInstallStep snapshot={snapshot} actions={actions} />;
    case 'firstBoot': return <FirstBootStep snapshot={snapshot} actions={actions} />;
    case 'appInstall': return <AppInstallStep snapshot={snapshot} actions={actions} />;
    case 'done': return <WizardLayout issue={snapshot.issue}>{null}</WizardLayout>;
  }
}

export function WizardScreen({ snapshot, actions }: ScreenProps) {
  const step = snapshot.wizard.step;
  return (
    <div className="ome-wizard">
      <WizardHeader step={step} />
      <div key={step} className="ome-wizard-step">
        <StepBody snapshot={snapshot} actions={actions} />
      </div>
    </div>
  );
}
