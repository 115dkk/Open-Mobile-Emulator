// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Display screen (M2-SCREENS.md 5, mockup S5): resolution, refresh rate, vertical sync and scaling.
// Rust says which rows exist (`refreshSupported`, `vsyncSupported`), whether a preset needs a restart
// (`needsReboot`) and whether a custom size is valid; this screen draws those values and sends commands.
import { useState } from 'react';
import type { ScreenProps } from '../actions';
import type { DisplayView, Orientation, Size, StageFit, VsyncMode } from '../contracts';
import { Button, Field, Icon, IssueNotice, ScreenHeader, Segmented } from '../components';
import type { SegmentedOption } from '../components';

const ORIENTATION_LABEL: Readonly<Record<Orientation, string>> = { landscape: '가로', portrait: '세로' };

function sizeLabel(size: Size): string {
  return `${String(size.width)}x${String(size.height)}`;
}

function orientationOf(size: Size): Orientation {
  return size.width >= size.height ? 'landscape' : 'portrait';
}

/** The line under 해상도: `현재 1920x1080 · 240 DPI · 가로`. */
function currentLine(display: DisplayView): string | null {
  const preset = display.presets.find((item) => item.id === display.activeId);
  if (preset !== undefined) {
    return `${sizeLabel(preset.size)} · ${String(preset.densityDpi)} DPI · ${ORIENTATION_LABEL[preset.orientation]}`;
  }
  if (display.custom !== null) {
    const { size, densityDpi } = display.custom;
    return `${sizeLabel(size)} · ${String(densityDpi)} DPI · ${ORIENTATION_LABEL[orientationOf(size)]}`;
  }
  return null;
}

/** A frame of the preset's shape inside a 192x120 box. */
function Shape({ size, active }: { readonly size: Size; readonly active: boolean }) {
  const scale = Math.min(192 / size.width, 120 / size.height);
  return (
    <div className="ome-display-shape" aria-hidden="true">
      <span
        className={active ? 'ome-display-shape-frame ome-display-shape-active' : 'ome-display-shape-frame'}
        style={{ width: `${String(Math.round(size.width * scale))}px`, height: `${String(Math.round(size.height * scale))}px` }}
      />
    </div>
  );
}

function Applied() {
  return (
    <span className="ome-display-applied">
      <Icon name="check" size={18} strokeWidth={2} />
      적용됨
    </span>
  );
}

/** The bounds Rust checks (`validate_display`): sides 640~7680 in steps of 8, density 120~640. */
const SIDE_MIN = 640;
const SIDE_MAX = 7680;
const DPI_MIN = 120;
const DPI_MAX = 640;
const FALLBACK_SIZE: Size = { width: 1920, height: 1080 };
const FALLBACK_DPI = 240;

interface CustomValues { readonly width: string; readonly height: string; readonly dpi: string }
interface Baseline { readonly size: Size; readonly densityDpi: number }

/**
 * What the guest shows now: the applied custom size, else the guest's resolution with the active
 * preset's density, else the active preset, else 1920x1080 at 240 DPI.
 */
function baselineOf(display: DisplayView, resolution: Size | null): Baseline {
  if (display.activeId === null && display.custom !== null) return display.custom;
  const preset = display.presets.find((item) => item.id === display.activeId);
  return {
    size: resolution ?? preset?.size ?? FALLBACK_SIZE,
    densityDpi: preset?.densityDpi ?? FALLBACK_DPI,
  };
}

function valuesOf(baseline: Baseline): CustomValues {
  return { width: String(baseline.size.width), height: String(baseline.size.height), dpi: String(baseline.densityDpi) };
}

function validSide(value: number): boolean {
  return Number.isInteger(value) && value >= SIDE_MIN && value <= SIDE_MAX && value % 8 === 0;
}

function validDpi(value: number): boolean {
  return Number.isInteger(value) && value >= DPI_MIN && value <= DPI_MAX;
}

/** Number('') is 0, which every check rejects, so an empty box reads as out of range. */
function numberOf(text: string): number {
  return text.trim() === '' ? Number.NaN : Number(text);
}

/** Density scaled with the side the user changed, kept within 120~640. Null while the side is out of range. */
function suggestedDpi(baseline: Baseline, side: 'width' | 'height', text: string): number | null {
  const value = numberOf(text);
  if (!Number.isFinite(value) || value < SIDE_MIN || value > SIDE_MAX) return null;
  const scaled = Math.round(baseline.densityDpi * value / baseline.size[side]);
  return Math.min(DPI_MAX, Math.max(DPI_MIN, scaled));
}

function CustomCard({ display, resolution, onApply }: {
  readonly display: DisplayView;
  readonly resolution: Size | null;
  readonly onApply: (size: Size, densityDpi: number) => Promise<void>;
}) {
  const baseline = baselineOf(display, resolution);
  // Null until the user types: the boxes then follow whatever the guest shows now.
  const [draft, setDraft] = useState<CustomValues | null>(null);
  // Once the user types a density, a new size no longer proposes one.
  const [dpiTouched, setDpiTouched] = useState(false);
  const values = draft ?? valuesOf(baseline);
  const width = numberOf(values.width);
  const height = numberOf(values.height);
  const dpi = numberOf(values.dpi);
  const widthOk = validSide(width);
  const heightOk = validSide(height);
  const dpiOk = validDpi(dpi);
  const valid = widthOk && heightOk && dpiOk;
  const same = width === baseline.size.width && height === baseline.size.height && dpi === baseline.densityDpi;
  const active = display.activeId === null && display.custom !== null;
  const canApply = valid && !same;

  const setSide = (side: 'width' | 'height', text: string) => {
    const proposal = dpiTouched ? null : suggestedDpi(baseline, side, text);
    setDraft({ ...values, [side]: text, dpi: proposal === null ? values.dpi : String(proposal) });
  };
  const setDensity = (text: string) => {
    setDpiTouched(true);
    setDraft({ ...values, dpi: text });
  };
  const apply = () => (canApply ? onApply({ width, height }, dpi) : Promise.resolve());
  const enter = () => { void apply(); };

  const problem = !widthOk ? '폭은 640~7680 사이의 8의 배수여야 합니다.'
    : !heightOk ? '높이는 640~7680 사이의 8의 배수여야 합니다.'
      : !dpiOk ? 'DPI는 120~640 사이의 정수여야 합니다.'
        : null;

  return (
    <div
      className={active ? 'ome-display-card ome-display-card-custom ome-display-card-active' : 'ome-display-card ome-display-card-custom'}
      role="group"
      aria-label="사용자 지정"
    >
      <span className="ome-display-card-size" aria-hidden="true">사용자 지정</span>
      <div className="ome-display-custom">
        <Field label="폭" type="number" value={values.width} onChange={(text) => { setSide('width', text); }}
          min={SIDE_MIN} max={SIDE_MAX} step={8} suffix="px" invalid={!widthOk} onEnter={enter} />
        <Field label="높이" type="number" value={values.height} onChange={(text) => { setSide('height', text); }}
          min={SIDE_MIN} max={SIDE_MAX} step={8} suffix="px" invalid={!heightOk} onEnter={enter} />
        <Field label="DPI" type="number" value={values.dpi} onChange={setDensity}
          min={DPI_MIN} max={DPI_MAX} invalid={!dpiOk} onEnter={enter} />
      </div>
      <p
        className={problem === null ? 'ome-display-custom-preview' : 'ome-display-custom-preview ome-display-custom-problem'}
        aria-live="polite"
      >
        {problem ?? `${String(width)}x${String(height)} · ${String(dpi)} DPI · ${ORIENTATION_LABEL[orientationOf({ width, height })]}`}
      </p>
      <div className="ome-display-card-action">
        {active && same ? <Applied /> : <Button disabled={!canApply} onClick={apply}>적용</Button>}
      </div>
    </div>
  );
}

function Resolution({ display, resolution, actions }: Pick<ScreenProps, 'actions'> & {
  readonly display: DisplayView;
  readonly resolution: Size | null;
}) {
  const current = currentLine(display);
  return (
    <section className="ome-display-section" aria-labelledby="ome-display-resolution">
      <div className="ome-display-head">
        <h2 id="ome-display-resolution" className="ome-section-title">해상도</h2>
        {current !== null && <p className="ome-muted">현재 <span className="ome-readout">{current}</span></p>}
      </div>
      <div className="ome-display-cards">
        {display.presets.map((preset) => {
          const active = preset.id === display.activeId;
          return (
            <div
              key={preset.id}
              className={active ? 'ome-display-card ome-display-card-active' : 'ome-display-card'}
              role="group"
              aria-label={sizeLabel(preset.size)}
            >
              <Shape size={preset.size} active={active} />
              <div className="ome-display-card-text">
                <span className="ome-display-card-size">{sizeLabel(preset.size)}</span>
                <span className="ome-muted ome-display-card-meta">
                  {String(preset.densityDpi)} DPI · {ORIENTATION_LABEL[preset.orientation]}
                </span>
              </div>
              <p className="ome-muted">
                {preset.needsReboot ? '방향을 바꾸려면 운영체제를 다시 시작해야 합니다.' : '즉시 적용할 수 있습니다.'}
              </p>
              <div className="ome-display-card-action">
                {active ? <Applied /> : <Button onClick={() => actions.displayPresetApply(preset.id)}>적용</Button>}
              </div>
            </div>
          );
        })}
        <CustomCard display={display} resolution={resolution} onApply={actions.displayCustomApply} />
      </div>
    </section>
  );
}

const CUSTOM = 'custom';

function RefreshRate({ display, actions }: Pick<ScreenProps, 'actions'> & { readonly display: DisplayView }) {
  const [customOpen, setCustomOpen] = useState(false);
  const [custom, setCustom] = useState('');
  // No value is the operating system's default, 60 Hz (ARCHITECTURE 8.4).
  const current = String(display.refreshRateHz ?? 60);
  const options: SegmentedOption<string>[] = display.refreshRates.map((hz) => ({
    value: String(hz), label: hz === 60 ? '60 Hz (기본)' : `${String(hz)} Hz`,
  }));
  options.push({ value: CUSTOM, label: '사용자 지정' });
  const choose = (value: string) => {
    if (value === CUSTOM) {
      setCustomOpen(true);
      return undefined;
    }
    setCustomOpen(false);
    const hz = Number(value);
    return actions.displayRefreshSet(hz === 60 ? null : hz);
  };
  const apply = async () => {
    setCustomOpen(false);
    setCustom('');
    await actions.displayRefreshSet(Number(custom));
  };
  return (
    <section className="ome-display-section" aria-labelledby="ome-display-refresh">
      <div className="ome-display-head">
        <h2 id="ome-display-refresh" className="ome-section-title">주사율</h2>
        <p className="ome-muted">다시 시작해야 적용됩니다.</p>
      </div>
      <Segmented label="주사율" options={options} value={customOpen ? CUSTOM : current} onChange={choose} />
      {customOpen && (
        <div className="ome-display-refresh-custom">
          <Field
            label="사용자 지정 주사율"
            labelHidden
            type="number"
            value={custom}
            onChange={setCustom}
            min={30}
            max={240}
            suffix="Hz"
            hint="30~240"
            onEnter={() => { if (custom.trim() !== '') void apply(); }}
          />
          <Button disabled={custom.trim() === ''} onClick={apply}>적용</Button>
        </div>
      )}
    </section>
  );
}

function RowSection<T extends string>({ id, title, help, options, value, onChange }: {
  readonly id: string;
  readonly title: string;
  readonly help: string;
  readonly options: readonly SegmentedOption<T>[];
  readonly value: T;
  readonly onChange: (value: T) => Promise<void>;
}) {
  return (
    <section className="ome-display-section ome-display-row" aria-labelledby={id}>
      <div className="ome-display-head">
        <h2 id={id} className="ome-section-title">{title}</h2>
        <p className="ome-muted">{help}</p>
      </div>
      <Segmented label={title} options={options} value={value} onChange={onChange} />
    </section>
  );
}

const VSYNC_OPTIONS: readonly SegmentedOption<VsyncMode>[] = [
  { value: 'off', label: '끔' },
  { value: 'on', label: '켬' },
  { value: 'adaptive', label: '적응형' },
];

const FIT_OPTIONS: readonly SegmentedOption<StageFit>[] = [
  { value: 'fitWindow', label: '창에 맞춤' },
  { value: 'oneToOne', label: '1:1' },
];

export function DisplayScreen({ snapshot, actions }: ScreenProps) {
  const { display } = snapshot;
  return (
    <>
      <ScreenHeader title="표시" />
      <div className="ome-display">
        {snapshot.issue !== null && <IssueNotice issue={snapshot.issue} />}
        <Resolution display={display} resolution={snapshot.guest.resolution} actions={actions} />
        {display.refreshSupported && <RefreshRate display={display} actions={actions} />}
        {display.vsyncSupported && (
          <RowSection
            id="ome-display-vsync"
            title="수직 동기화"
            help="다시 시작해야 적용됩니다."
            options={VSYNC_OPTIONS}
            value={display.vsync}
            onChange={actions.displayVsyncSet}
          />
        )}
        <RowSection
          id="ome-display-fit"
          title="배율"
          help="창에 맞춤은 창 크기에 따라 화면을 늘리거나 줄입니다. 1:1은 픽셀을 그대로 보입니다."
          options={FIT_OPTIONS}
          value={display.fit}
          onChange={actions.stageFitSet}
        />
      </div>
    </>
  );
}
