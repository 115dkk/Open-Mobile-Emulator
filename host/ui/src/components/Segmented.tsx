// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useRef } from 'react';
import type { KeyboardEvent } from 'react';

export interface SegmentedOption<T extends string> {
  readonly value: T;
  readonly label: string;
  readonly disabled?: boolean | undefined;
}

export interface SegmentedProps<T extends string> {
  /** Accessible name of the group. */
  readonly label: string;
  readonly options: readonly SegmentedOption<T>[];
  /** Selected value, or null when none is selected yet. */
  readonly value: T | null;
  readonly onChange: (value: T) => void | Promise<void>;
  readonly disabled?: boolean | undefined;
}

/** A radio group drawn as joined buttons. Arrow keys move and select, like native radios. */
export function Segmented<T extends string>({ label, options, value, onChange, disabled = false }: SegmentedProps<T>) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const enabled = options.filter((option) => !disabled && option.disabled !== true);
  const selectedIndex = options.findIndex((option) => option.value === value);
  const focusIndex = selectedIndex >= 0 ? selectedIndex : options.findIndex((option) => enabled.includes(option));

  const move = (event: KeyboardEvent<HTMLButtonElement>, from: number) => {
    const step = event.key === 'ArrowRight' || event.key === 'ArrowDown' ? 1
      : event.key === 'ArrowLeft' || event.key === 'ArrowUp' ? -1 : 0;
    if (step === 0 || enabled.length === 0) return;
    event.preventDefault();
    for (let offset = 1; offset <= options.length; offset += 1) {
      const index = (from + step * offset + options.length * offset) % options.length;
      const option = options[index];
      if (option !== undefined && enabled.includes(option)) {
        refs.current[index]?.focus();
        void onChange(option.value);
        return;
      }
    }
  };

  return (
    <div className="ome-segmented" role="radiogroup" aria-label={label} aria-disabled={disabled || undefined}>
      {options.map((option, index) => {
        const checked = option.value === value;
        const optionDisabled = disabled || option.disabled === true;
        return (
          <button
            key={option.value}
            ref={(element) => { refs.current[index] = element; }}
            type="button"
            role="radio"
            className="ome-segmented-option"
            aria-checked={checked}
            tabIndex={index === focusIndex ? 0 : -1}
            disabled={optionDisabled}
            onClick={() => { if (!checked) void onChange(option.value); }}
            onKeyDown={(event) => { move(event, index); }}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
