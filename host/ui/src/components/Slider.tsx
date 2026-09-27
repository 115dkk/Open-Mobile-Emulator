// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

export interface SliderProps {
  readonly label: string;
  readonly min: number;
  readonly max: number;
  readonly step?: number | undefined;
  readonly value: number;
  /** Every movement. Keep the value in local state and save on commit. */
  readonly onChange: (value: number) => void;
  /** Pointer release or key release: the moment to send a command. */
  readonly onCommit?: ((value: number) => void | Promise<void>) | undefined;
  /** Readout next to the track, for example `8 GiB`. */
  readonly format: (value: number) => string;
  readonly disabled?: boolean | undefined;
}

export function Slider({ label, min, max, step = 1, value, onChange, onCommit, format, disabled = false }: SliderProps) {
  const commit = (target: HTMLInputElement) => {
    if (onCommit !== undefined) void onCommit(Number(target.value));
  };
  return (
    <div className="ome-slider">
      <input
        type="range"
        className="ome-slider-input"
        aria-label={label}
        aria-valuetext={format(value)}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(event) => { onChange(Number(event.currentTarget.value)); }}
        onPointerUp={(event) => { commit(event.currentTarget); }}
        onKeyUp={(event) => { commit(event.currentTarget); }}
      />
      <span className="ome-slider-value">{format(value)}</span>
    </div>
  );
}
