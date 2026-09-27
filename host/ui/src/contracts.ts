// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Read-only presentation contracts mirroring ome-runtime contract.rs field for field.

export const CONTRACT_VERSION = 5;

export type AppPhase = 'wizard' | 'main';
export type BlockerKind = 'virtualizationOff' | 'qemuMissing' | 'hypervisorPlatformOff';
export interface Blocker { readonly kind: BlockerKind }

export type HostCheckId =
  | 'cpuVirtualization' | 'hypervisorPlatform' | 'rebootPending' | 'whpxAvailable'
  | 'qemuPresent' | 'firmwarePresent' | 'adbPresent' | 'diskSpace';
export type HostStatus = 'ready' | 'attention' | 'blocked';
export interface HostRow { readonly id: HostCheckId; readonly status: HostStatus; readonly detail: string }
export interface HostReport { readonly rows: readonly HostRow[]; readonly ready: boolean; readonly inspectedAt: string | null }

export type WizardStep =
  | 'hostCheck' | 'whpxConsent' | 'rebootPending' | 'artifactDownload' | 'guestInstall'
  | 'firstBoot' | 'appInstall' | 'done';
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
  readonly imageId: string | null;
  readonly installGuide: readonly string[];
  readonly diskSizeGib: number;
  readonly diskFreeBytes: number | null;
}

export interface InstallProgress {
  readonly label: string;
  readonly stage: TransferStage;
  readonly doneItems: number;
  readonly totalItems: number;
  readonly ratio: number | null;
  readonly doneBytes: number;
  readonly totalBytes: number | null;
}

export type ImageDistribution = 'bliss' | 'androidX86' | 'selfBuilt';
export type ImageTranslator = 'houdini' | 'ndkTranslation' | 'digitalis' | 'none';
export type ImageStatus = 'verified' | 'candidate' | 'deprecated';
export type Capability = 'available' | 'unavailable' | 'unknown';
export type CapabilityId =
  | 'bootMarker' | 'appList' | 'displaySize' | 'mediaVolume' | 'deviceId' | 'screenshot'
  | 'foregroundApp' | 'multitouch' | 'nativeBridge' | 'root';
export interface CapabilityItem { readonly id: CapabilityId; readonly state: Capability }
export interface CapabilityReport { readonly probedAt: string | null; readonly items: readonly CapabilityItem[] }
export interface GuestImageSummary {
  readonly id: string;
  readonly displayName: string;
  readonly androidVersion: string;
  readonly apiLevel: number;
  readonly distribution: ImageDistribution;
  readonly translator: ImageTranslator;
  readonly sizeBytes: number | null;
  readonly status: ImageStatus;
  readonly releasedAt: string | null;
  readonly verifiedGames: number;
  readonly recommended: boolean;
}
export interface GuestSummary {
  readonly name: string;
  readonly imageId: string;
  readonly androidVersion: string;
  readonly diskSizeGib: number;
  readonly lastStartedAt: string | null;
  readonly capabilities: CapabilityReport;
}
export interface ImagesView {
  readonly profiles: readonly GuestImageSummary[];
  readonly guests: readonly GuestSummary[];
  readonly activeGuest: string | null;
}

export type GuestState = 'stopped' | 'starting' | 'running' | 'stopping' | 'restarting' | 'failed';
export type HostingMode = 'none' | 'embedded' | 'separateWindow';
export type ExitKind = 'userStop' | 'guestReset' | 'bootTimeout' | 'crash' | 'startFailed';
export interface Size { readonly width: number; readonly height: number }
export interface Rect { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
/** The stage rectangle in CSS pixels plus the device scale factor. Rust converts it. */
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
  readonly imageId: string | null;
  readonly androidVersion: string | null;
  readonly apiLevel: number | null;
  readonly capabilities: CapabilityReport;
  readonly deviceId: string | null;
  readonly deviceIdDecimal: string | null;
  readonly googleAccounts: number | null;
  readonly registrationOpenedAt: string | null;
  readonly addAccountSupported: boolean;
  readonly pid: number | null;
  readonly adbAddress: string | null;
  readonly rootEnabled: boolean | null;
  readonly mediaVolume: number | null;
}

export interface AppItem {
  readonly package: string;
  readonly label: string;
  readonly versionName: string | null;
  readonly versionCode: number | null;
  readonly installedAt: string | null;
}
export interface AppsView { readonly available: boolean; readonly items: readonly AppItem[]; readonly install: InstallProgress | null }

export interface LogicalPoint { readonly x: number; readonly y: number }
export type MouseButton = 'left' | 'right' | 'middle';
export type WheelDirection = 'up' | 'down';
export type Trigger =
  | { readonly kind: 'key'; readonly code: string }
  | { readonly kind: 'mouseButton'; readonly button: MouseButton }
  | { readonly kind: 'wheel'; readonly direction: WheelDirection }
  | { readonly kind: 'keySet'; readonly up: string; readonly down: string; readonly left: string; readonly right: string };
export type BindingAction =
  | { readonly kind: 'tap'; readonly at: LogicalPoint; readonly hold: boolean }
  | { readonly kind: 'swipe'; readonly from: LogicalPoint; readonly to: LogicalPoint; readonly durationMs: number }
  | { readonly kind: 'joystick'; readonly center: LogicalPoint; readonly radius: number }
  | { readonly kind: 'mouseTap'; readonly at: LogicalPoint | null }
  | { readonly kind: 'wheelSwipe'; readonly at: LogicalPoint; readonly distance: number }
  | { readonly kind: 'passThrough' };
export interface Binding { readonly id: string; readonly trigger: Trigger; readonly action: BindingAction }
export type Anchor = 'center' | 'edges';
export interface InputProfile {
  readonly id: string;
  readonly name: string;
  readonly bundled: boolean;
  readonly targetPackage: string | null;
  readonly referenceAspect: Size;
  readonly anchor: Anchor;
  readonly bindings: readonly Binding[];
}
export interface InputView {
  readonly profiles: readonly InputProfile[];
  readonly activeId: string | null;
  readonly suspended: boolean;
  readonly editing: boolean;
  readonly autoApply: boolean;
  readonly foregroundPackage: string | null;
  readonly multitouch: Capability;
  readonly suspendHotkey: string;
  readonly overlayVisible: boolean;
}

export type Orientation = 'landscape' | 'portrait';
export type StageFit = 'fitWindow' | 'oneToOne';
export type VsyncMode = 'off' | 'on' | 'adaptive';
export interface DisplayPreset { readonly id: string; readonly size: Size; readonly densityDpi: number; readonly orientation: Orientation; readonly needsReboot: boolean }
export interface CustomDisplay { readonly size: Size; readonly densityDpi: number }
export interface DisplayView {
  readonly presets: readonly DisplayPreset[];
  readonly activeId: string | null;
  readonly custom: CustomDisplay | null;
  readonly fit: StageFit;
  readonly refreshRateHz: number | null;
  readonly refreshRates: readonly number[];
  readonly refreshSupported: boolean;
  readonly vsync: VsyncMode;
  readonly vsyncSupported: boolean;
}

export type GpuMode = 'virgl' | 'software';
export type CloseAction = 'stopGuest' | 'minimizeToTray';
export type AdbAccess = 'localhost' | 'network';
export interface SettingsView {
  readonly memoryMib: number;
  readonly memoryMibMin: number;
  readonly memoryMibMax: number;
  readonly vcpus: number;
  readonly vcpusMax: number;
  readonly gpuMode: GpuMode;
  readonly closeAction: CloseAction;
  readonly showFps: boolean;
  readonly autoUpdateCheck: boolean;
  readonly homeDir: string;
  readonly diskUsageBytes: number | null;
  readonly adbAccess: AdbAccess;
  readonly bindingOverlayDefault: boolean;
}
export interface SettingsInput {
  readonly memoryMib: number;
  readonly vcpus: number;
  readonly gpuMode: GpuMode;
  readonly closeAction: CloseAction;
  readonly showFps: boolean;
  readonly autoUpdateCheck: boolean;
  readonly adbAccess: AdbAccess;
  readonly bindingOverlayDefault: boolean;
}

export type UpdateState =
  | { readonly kind: 'idle' }
  | { readonly kind: 'checking' }
  | { readonly kind: 'upToDate'; readonly checkedAt: string }
  | { readonly kind: 'available'; readonly version: string; readonly notesUrl: string | null; readonly asset: UpdateAsset }
  | { readonly kind: 'downloading'; readonly progress: TransferProgress }
  | { readonly kind: 'readyToInstall'; readonly version: string }
  | { readonly kind: 'failed'; readonly issue: AppIssue };
export interface UpdateAsset { readonly name: string; readonly sizeBytes: number }
export interface UpdateView { readonly currentVersion: string; readonly state: UpdateState }

export type NoticeLevel = 'info' | 'warning' | 'error';
export interface Notice { readonly at: string; readonly level: NoticeLevel; readonly message: string }
export interface AppIssue { readonly code: string; readonly message: string; readonly nextAction: string | null }

export interface AppSnapshot {
  readonly contractVersion: number;
  readonly productVersion: string;
  readonly phase: AppPhase;
  readonly blocker: Blocker | null;
  readonly host: HostReport;
  readonly wizard: WizardView;
  readonly images: ImagesView;
  readonly guest: GuestView;
  readonly apps: AppsView;
  readonly input: InputView;
  readonly display: DisplayView;
  readonly settings: SettingsView;
  readonly update: UpdateView;
  readonly notices: readonly Notice[];
  readonly issue: AppIssue | null;
}

/** Help destinations opened by trusted native code. */
export type HelpTopic =
  | 'virtualizationBios' | 'hypervisorPlatform' | 'googleAccount' | 'adbSecurity'
  | 'qemuSource' | 'thirdPartyNotices' | 'releaseNotes';

/** Snapshot-owned values that native code may copy. */
export type ClipboardItem = 'deviceId' | 'adbAddress';

/** Event names emitted by the shell. */
export const EVENT_SNAPSHOT = 'snapshot';
export const EVENT_PROGRESS = 'progress';

/** The typed IPC surface available to screens. */
export interface ControllerBridge {
  snapshot(): Promise<AppSnapshot>;
  hostCheckRefresh(): Promise<AppSnapshot>;
  wizardContinue(): Promise<AppSnapshot>;
  wizardSkip(): Promise<AppSnapshot>;
  wizardDefer(): Promise<AppSnapshot>;
  openHelp(topic: HelpTopic): Promise<AppSnapshot>;
  appQuit(): Promise<AppSnapshot>;
  whpxEnable(): Promise<AppSnapshot>;
  artifactDownloadStart(): Promise<AppSnapshot>;
  artifactDownloadCancel(): Promise<AppSnapshot>;
  guestImageSelect(id: string): Promise<AppSnapshot>;
  guestCreate(imageId: string, sizeGib: number): Promise<AppSnapshot>;
  guestSelect(id: string): Promise<AppSnapshot>;
  guestDelete(id: string): Promise<AppSnapshot>;
  guestReinstall(name: string): Promise<AppSnapshot>;
  guestStart(): Promise<AppSnapshot>;
  guestStop(): Promise<AppSnapshot>;
  guestRestart(): Promise<AppSnapshot>;
  guestRootSet(enabled: boolean): Promise<AppSnapshot>;
  guestVolumeSet(index: number): Promise<AppSnapshot>;
  stageRectChanged(rect: StageRect): Promise<AppSnapshot>;
  screenshotSave(): Promise<AppSnapshot>;
  appInstallPick(): Promise<AppSnapshot>;
  appInstallCancel(): Promise<AppSnapshot>;
  appUninstall(pkg: string): Promise<AppSnapshot>;
  appLaunch(pkg: string): Promise<AppSnapshot>;
  inputProfileSelect(id: string | null): Promise<AppSnapshot>;
  inputSuspendToggle(): Promise<AppSnapshot>;
  inputOverlayToggle(): Promise<AppSnapshot>;
  inputProfileDelete(id: string): Promise<AppSnapshot>;
  inputProfileSave(profile: InputProfile): Promise<AppSnapshot>;
  inputBindingUpsert(profileId: string, binding: Binding): Promise<AppSnapshot>;
  inputBindingRemove(profileId: string, id: string): Promise<AppSnapshot>;
  inputEditorToggle(): Promise<AppSnapshot>;
  inputAutoApplySet(enabled: boolean): Promise<AppSnapshot>;
  inputSuspendHotkeySet(code: string): Promise<AppSnapshot>;
  displayPresetApply(id: string): Promise<AppSnapshot>;
  displayCustomApply(size: Size, densityDpi: number): Promise<AppSnapshot>;
  displayRefreshSet(hz: number | null): Promise<AppSnapshot>;
  displayVsyncSet(mode: VsyncMode): Promise<AppSnapshot>;
  stageFitSet(fit: StageFit): Promise<AppSnapshot>;
  settingsSave(settings: SettingsInput): Promise<AppSnapshot>;
  updateCheck(): Promise<AppSnapshot>;
  updateInstall(): Promise<AppSnapshot>;
  diagnosticsExport(): Promise<AppSnapshot>;
  openLogsFolder(): Promise<AppSnapshot>;
  openScreenshotsFolder(): Promise<AppSnapshot>;
  openHomeFolder(): Promise<AppSnapshot>;
  copyToClipboard(item: ClipboardItem): Promise<AppSnapshot>;
  openRegistrationPage(): Promise<AppSnapshot>;
  googleAccountAddOpen(): Promise<AppSnapshot>;
  guestWindowToFront(): Promise<AppSnapshot>;
  /** Subscribe to snapshot pushes. */
  watchSnapshot(notify: (snapshot: AppSnapshot) => void): Promise<() => Promise<void>>;
  /** Subscribe to frequent transfer progress. */
  watchProgress(notify: (progress: TransferProgress) => void): Promise<() => Promise<void>>;
}
