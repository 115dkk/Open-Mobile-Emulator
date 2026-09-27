// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { useEffect, useEffectEvent, useId, useRef } from 'react';
import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Button } from './Button';

export interface DialogProps {
  readonly open: boolean;
  /** The fact in one line, for example `샘플 앱 A를 제거합니다.` */
  readonly title: string;
  /** Optional second sentence (what is lost). */
  readonly message?: ReactNode;
  /** Extra content between the text and the buttons (image cards, a size picker). */
  readonly children?: ReactNode;
  /** Verb noun for the confirming button: `제거`, `삭제`, `다시 설치`. */
  readonly confirmLabel: string;
  readonly cancelLabel?: string | undefined;
  /** `danger` for what cannot be undone. */
  readonly tone?: 'default' | 'danger' | undefined;
  readonly confirmDisabled?: boolean | undefined;
  readonly onConfirm: () => void | Promise<void>;
  readonly onCancel: () => void;
}

const FOCUSABLE = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Modal confirmation. Esc, the cancel button and the backdrop all cancel; focus stays inside. */
export function Dialog({
  open, title, message, children, confirmLabel, cancelLabel = '취소', tone = 'default',
  confirmDisabled = false, onConfirm, onCancel,
}: DialogProps) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const cancel = useEffectEvent(() => { onCancel(); });

  useEffect(() => {
    if (!open) return undefined;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const root = panel.current;
    // Focus starts on the safe choice.
    root?.querySelector<HTMLElement>('[data-dialog-cancel] button')?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        cancel();
        return;
      }
      if (event.key !== 'Tab' || root === null) return;
      const items = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE));
      const first = items[0];
      const last = items[items.length - 1];
      if (first === undefined || last === undefined) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      previous?.focus();
    };
  }, [open]);

  if (!open) return null;
  return createPortal(
    <div className="ome-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onCancel(); }}>
      <div ref={panel} className="ome-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <h2 id={titleId} className="ome-dialog-title">{title}</h2>
        {message !== undefined && <p className="ome-dialog-message">{message}</p>}
        {children !== undefined && <div className="ome-dialog-body">{children}</div>}
        <div className="ome-dialog-actions">
          <span data-dialog-cancel="" className="ome-dialog-slot">
            <Button variant="secondary" onClick={onCancel}>{cancelLabel}</Button>
          </span>
          <Button variant={tone === 'danger' ? 'danger-solid' : 'primary'} disabled={confirmDisabled} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
