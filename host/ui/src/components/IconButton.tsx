// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Icon } from './Icon';
import type { IconName } from './Icon';

export interface IconButtonProps {
  readonly icon: IconName;
  /** Accessible name and tooltip. Icon-only buttons always carry one. */
  readonly label: string;
  readonly onClick?: (() => void | Promise<void>) | undefined;
  /** When set, the button is a toggle and exposes `aria-pressed`. */
  readonly pressed?: boolean | undefined;
  readonly disabled?: boolean | undefined;
  readonly className?: string | undefined;
}

export function IconButton({ icon, label, onClick, pressed, disabled = false, className }: IconButtonProps) {
  return (
    <button
      type="button"
      className={className === undefined ? 'ome-icon-button' : `ome-icon-button ${className}`}
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick === undefined ? undefined : () => { void onClick(); }}
    >
      <Icon name={icon} size={20} />
    </button>
  );
}
