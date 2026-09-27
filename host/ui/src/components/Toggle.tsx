// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

export interface ToggleProps {
  readonly checked: boolean;
  readonly onChange: (next: boolean) => void | Promise<void>;
  /** Accessible name; usually the setting row label. */
  readonly label: string;
  readonly disabled?: boolean | undefined;
}

/** On/off switch: 44x24 track inside a 44x40 hit area. */
export function Toggle({ checked, onChange, label, disabled = false }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      className="ome-toggle"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => { void onChange(!checked); }}
    >
      <span className="ome-toggle-track"><span className="ome-toggle-thumb" /></span>
    </button>
  );
}
