// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// `편집 끝` with unsaved changes asks one question with three answers. The shared Dialog has two
// buttons, so this one reuses its classes and its behaviour: focus starts on the safe choice
// (`계속 편집`), stays inside, and Esc or the backdrop also mean `계속 편집`.
import { useEffect, useEffectEvent, useId, useRef } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '../components';

const FOCUSABLE = 'button:not([disabled])';

export interface LeaveDialogProps {
  readonly onSave: () => void;
  readonly onDiscard: () => void;
  readonly onStay: () => void;
}

export function LeaveDialog({ onSave, onDiscard, onStay }: LeaveDialogProps) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const stay = useEffectEvent(() => { onStay(); });

  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const root = panel.current;
    root?.querySelector<HTMLElement>('[data-dialog-stay] button')?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        stay();
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
  }, []);

  return createPortal(
    <div className="ome-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onStay(); }}>
      <div ref={panel} className="ome-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <h2 id={titleId} className="ome-dialog-title">저장하지 않은 변경이 있습니다.</h2>
        <div className="ome-dialog-actions">
          <Button variant="danger" onClick={onDiscard}>버리고 끝</Button>
          <span data-dialog-stay="" className="ome-dialog-slot">
            <Button variant="secondary" onClick={onStay}>계속 편집</Button>
          </span>
          <Button variant="primary" onClick={onSave}>저장하고 끝</Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
