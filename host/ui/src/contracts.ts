// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Read-only presentation contracts. Mirrors host/crates/ome-runtime/src/contract.rs field for
// field; both change in one commit and the fixtures under tests/fixtures/contract/ keep them
// equal. The Rust side decides everything; these types carry data.

export const CONTRACT_VERSION = 1;

export type AppPhase = 'wizard' | 'main';

export type HostCheckId =
  | 'cpuVirtualization' | 'hypervisorPlatform' | 'rebootPending' | 'whpxAvailable'
  | 'qemuPresent' | 'firmwarePresent' | 'adbPresent' | 'diskSpace';
export type HostStatus = 'ready' | 'attention' | 'blocked';
export interface HostRow { readonly id: HostCheckId; readonly status: HostStatus; readonly detail: string }
export interface HostReport { readonly rows: readonly HostRow[]; readonly ready: boolean; readonly inspectedAt: string | null }

export type WizardStep =
  | 'hostCheck' | 'whpxConsent' | 'rebootPending' | 'artifactDownload' | 'guestInstall'
  | 'firstBoot' | 'googleRegistration' | 'appInstall' | 'done';
export type TransferStage = 'waiting' | 'transferring' | 'verifying' | 'verified' | 'failed' | 'cancelled';
export interface TransferProgress {
  readonly stage: TransferStage;
  readonly doneBytes: number;
  readonly totalBytes: number | null;
  readonly bytesPerSecond: number | null;
  readonly label: string;
}
export interface WizardView {
  readonly step: WizardStep;
  readonly canContinue: boolean;
  readonly canSkip: boolean;
  readonly download: TransferProgress | null;
  readonly gsfId: string | null;
  readonly diskSizeGib: number;
  readonly diskFreeBytes: number | null;
}

export type GuestState = 'stopped' | 'starting' | 'running' | 'stopping' | 'restarting' | 'failed';
export type HostingMode = 'none' | 'embedded' | 'separateWindow';
export type ExitKind = 'userStop' | 'guestReset' | 'unexpected';
export interface Size { readonly width: number; readonly height: number }
export interface Rect { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
/** The stage rectangle in CSS pixels plus the device scale factor. Rust converts; the webview never does. */
export interface StageRect { readonly x: number; readonly y: number; readonly width: number; readonly height: number; readonly scaleFactor: number }
export interface LastExit { readonly kind: ExitKind; readonly at: string; readonly logPath: string | null }
export interface GuestView {
  readonly state: GuestState;
  readonly bootCompleted: boolean;
  readonly adbConnected: boolean;
  readonly hosting: HostingMode;
  readonly resolution: Size | null;
  readonly lastExit: LastExit | null;
  readonly fps: number | null;
  readonly startedAt: string | null;
}

export interface AppItem { readonly package: string; readonly label: string; readonly versionName: string | null; readonly installedAt: string | null }
export interface AppsView { readonly available: boolean; readonly items: readonly AppItem[]; readonly install: TransferProgress | null }

export interface KeymapProfileSummary { readonly id: string; readonly name: string; readonly bundled: boolean; readonly bindingCount: number }
export interface KeymapView { readonly profiles: readonly KeymapProfileSummary[]; readonly activeId: string | null; readonly enabled: boolean }

export type Orientation = 'landscape' | 'portrait';
export type StageFit = 'fitWindow' | 'oneToOne';
export interface DisplayPreset { readonly id: string; readonly size: Size; readonly densityDpi: number; readonly orientation: Orientation; readonly needsReboot: boolean }
export interface DisplayView { readonly presets: readonly DisplayPreset[]; readonly activeId: string | null; readonly fit: StageFit }

export type GpuMode = 'virgl' | 'software';
export type CloseAction = 'minimizeToTray' | 'stopGuest';
export interface SettingsView {
  readonly memoryMib: number;
  readonly vcpus: number;
  readonly gpuMode: GpuMode;
  readonly closeAction: CloseAction;
  readonly showFps: boolean;
  readonly autoUpdateCheck: boolean;
  readonly homeDir: string;
  readonly diskUsageBytes: number | null;
}
export interface SettingsInput {
  readonly memoryMib: number;
  readonly vcpus: number;
  readonly gpuMode: GpuMode;
  readonly closeAction: CloseAction;
  readonly showFps: boolean;
  readonly autoUpdateCheck: boolean;
}

export type UpdateState =
  | { readonly kind: 'idle' }
  | { readonly kind: 'checking' }
  | { readonly kind: 'upToDate'; readonly checkedAt: string }
  | { readonly kind: 'available'; readonly version: string; readonly notesUrl: string | null }
  | { readonly kind: 'downloading'; readonly progress: TransferProgress }
  | { readonly kind: 'readyToInstall'; readonly version: string }
  | { readonly kind: 'failed'; readonly issue: AppIssue };
export interface UpdateView { readonly currentVersion: string; readonly state: UpdateState }

export type NoticeLevel = 'info' | 'warning' | 'error';
export interface Notice { readonly at: string; readonly level: NoticeLevel; readonly message: string }

/** What a failed command returns. `message` and `nextAction` are the words the screen shows. */
export interface AppIssue { readonly code: string; readonly message: string; readonly nextAction: string | null }

export interface AppSnapshot {
  readonly contractVersion: number;
  readonly productVersion: string;
  readonly phase: AppPhase;
  readonly host: HostReport;
  readonly wizard: WizardView;
  readonly guest: GuestView;
  readonly apps: AppsView;
  readonly keymap: KeymapView;
  readonly display: DisplayView;
  readonly settings: SettingsView;
  readonly update: UpdateView;
  readonly notices: readonly Notice[];
  readonly issue: AppIssue | null;
}

/** Event names the shell emits. */
export const EVENT_SNAPSHOT = 'snapshot';
export const EVENT_PROGRESS = 'progress';

/**
 * The only IPC surface the screens use. `bridge.ts` implements it over Tauri `invoke`; tests
 * implement it over fixtures. Every method maps to one Tauri command of the same name in
 * snake_case (host/crates/ome-runtime/src/contract.rs, TAURI_COMMANDS). Every mutation returns
 * the new snapshot or rejects with an AppIssue.
 */
export interface ControllerBridge {
  snapshot(): Promise<AppSnapshot>;
  hostCheckRefresh(): Promise<AppSnapshot>;
  wizardContinue(): Promise<AppSnapshot>;
  wizardSkip(): Promise<AppSnapshot>;
  wizardRestart(): Promise<AppSnapshot>;
  whpxEnable(): Promise<AppSnapshot>;
  artifactDownloadStart(): Promise<AppSnapshot>;
  artifactDownloadCancel(): Promise<AppSnapshot>;
  guestDiskCreate(sizeGib: number): Promise<AppSnapshot>;
  guestStart(): Promise<AppSnapshot>;
  guestStop(): Promise<AppSnapshot>;
  guestRestart(): Promise<AppSnapshot>;
  stageRectChanged(rect: StageRect): Promise<AppSnapshot>;
  screenshotSave(): Promise<AppSnapshot>;
  appInstallPick(): Promise<AppSnapshot>;
  appUninstall(pkg: string): Promise<AppSnapshot>;
  appLaunch(pkg: string): Promise<AppSnapshot>;
  keymapSetActive(id: string | null): Promise<AppSnapshot>;
  keymapSetEnabled(enabled: boolean): Promise<AppSnapshot>;
  keymapDelete(id: string): Promise<AppSnapshot>;
  displayPresetApply(id: string): Promise<AppSnapshot>;
  stageFitSet(fit: StageFit): Promise<AppSnapshot>;
  settingsSave(settings: SettingsInput): Promise<AppSnapshot>;
  updateCheck(): Promise<AppSnapshot>;
  updateInstall(): Promise<AppSnapshot>;
  diagnosticsExport(): Promise<AppSnapshot>;
  openLogsFolder(): Promise<AppSnapshot>;
  openScreenshotsFolder(): Promise<AppSnapshot>;
  openRegistrationPage(): Promise<AppSnapshot>;
  guestWindowToFront(): Promise<AppSnapshot>;
  /** Subscribe to snapshot pushes. Returns an unsubscribe function. */
  watchSnapshot(notify: (snapshot: AppSnapshot) => void): Promise<() => Promise<void>>;
  /** Subscribe to frequent transfer progress. Returns an unsubscribe function. */
  watchProgress(notify: (progress: TransferProgress) => void): Promise<() => Promise<void>>;
}
