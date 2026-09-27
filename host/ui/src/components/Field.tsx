// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useId } from 'react';
import { Icon } from './Icon';
import type { IconName } from './Icon';

export interface FieldProps {
  readonly label: string;
  /** Hide the label visually and keep it as the accessible name (search boxes). */
  readonly labelHidden?: boolean | undefined;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly type?: 'text' | 'number' | 'search' | undefined;
  readonly placeholder?: string | undefined;
  readonly icon?: IconName | undefined;
  /** Unit after the input, for example `px` or `DPI`. */
  readonly suffix?: string | undefined;
  readonly min?: number | undefined;
  readonly max?: number | undefined;
  readonly step?: number | undefined;
  /** One short sentence under the input, for example the allowed range. */
  readonly hint?: string | undefined;
  readonly invalid?: boolean | undefined;
  readonly disabled?: boolean | undefined;
  readonly onEnter?: (() => void) | undefined;
}

export function Field({
  label, labelHidden = false, value, onChange, type = 'text', placeholder, icon, suffix,
  min, max, step, hint, invalid = false, disabled = false, onEnter,
}: FieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  return (
    <div className="ome-field">
      <label className={labelHidden ? 'ome-visually-hidden' : 'ome-field-label'} htmlFor={id}>{label}</label>
      <div className={invalid ? 'ome-field-box ome-field-box-invalid' : 'ome-field-box'}>
        {icon !== undefined && <Icon name={icon} size={18} />}
        <input
          id={id}
          className="ome-field-input"
          type={type}
          inputMode={type === 'number' ? 'numeric' : undefined}
          value={value}
          placeholder={placeholder}
          min={min}
          max={max}
          step={step}
          disabled={disabled}
          aria-invalid={invalid || undefined}
          aria-describedby={hint === undefined ? undefined : hintId}
          onChange={(event) => { onChange(event.currentTarget.value); }}
          onKeyDown={(event) => { if (event.key === 'Enter' && onEnter !== undefined) onEnter(); }}
        />
        {suffix !== undefined && <span className="ome-field-suffix">{suffix}</span>}
      </div>
      {hint !== undefined && <p id={hintId} className="ome-field-hint">{hint}</p>}
    </div>
  );
}
