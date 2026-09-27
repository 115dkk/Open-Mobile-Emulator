// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Icon } from './Icon';

export interface SelectOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

export interface SelectProps<T extends string> {
  /** Accessible name; usually the setting row label. */
  readonly label: string;
  readonly options: readonly SelectOption<T>[];
  readonly value: T;
  readonly onChange: (value: T) => void | Promise<void>;
  readonly disabled?: boolean | undefined;
}

/** Native select with the product's control frame. */
export function Select<T extends string>({ label, options, value, onChange, disabled = false }: SelectProps<T>) {
  return (
    <span className="ome-select">
      <select
        className="ome-select-input"
        aria-label={label}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          const next = options.find((option) => option.value === event.currentTarget.value);
          if (next !== undefined) void onChange(next.value);
        }}
      >
        {options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
      </select>
      <span className="ome-select-chevron"><Icon name="chevron-down" size={16} /></span>
    </span>
  );
}
