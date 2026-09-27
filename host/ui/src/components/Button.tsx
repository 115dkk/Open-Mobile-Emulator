// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { ReactNode } from 'react';
import { Icon } from './Icon';
import type { IconName } from './Icon';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'danger-solid';

export interface ButtonProps {
  readonly children: ReactNode;
  /**
   * `primary` is the one main action of a view. `danger` is a quiet red text button (row actions such
   * as 제거); `danger-solid` confirms something that cannot be undone inside a dialog.
   */
  readonly variant?: ButtonVariant | undefined;
  /** `large` is the 44px wizard main button; the default is 40px. */
  readonly size?: 'default' | 'large' | undefined;
  readonly icon?: IconName | undefined;
  readonly onClick?: (() => void | Promise<void>) | undefined;
  readonly disabled?: boolean | undefined;
  readonly type?: 'button' | 'submit' | undefined;
  readonly className?: string | undefined;
  readonly autoFocus?: boolean | undefined;
}

export function Button({
  children, variant = 'secondary', size = 'default', icon, onClick, disabled = false,
  type = 'button', className, autoFocus,
}: ButtonProps) {
  const classes = ['ome-button', `ome-button-${variant}`];
  if (size === 'large') classes.push('ome-button-large');
  if (className !== undefined) classes.push(className);
  return (
    <button
      type={type}
      className={classes.join(' ')}
      disabled={disabled}
      autoFocus={autoFocus}
      onClick={onClick === undefined ? undefined : () => { void onClick(); }}
    >
      {icon !== undefined && <Icon name={icon} size={18} strokeWidth={2} />}
      <span>{children}</span>
    </button>
  );
}
