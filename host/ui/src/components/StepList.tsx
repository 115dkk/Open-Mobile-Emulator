// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import { Fragment } from 'react';
import type { ReactNode } from 'react';
import type { StepState } from '../presentation';
import { Icon } from './Icon';
import { StatusDot } from './StatusDot';

export interface StepItem {
  readonly id: string;
  /** Noun phrase. The symbol says done or in progress, so the words never do. */
  readonly label: string;
  readonly state: StepState;
  /** Optional detail under the line (vertical lists only). */
  readonly children?: ReactNode;
}

export interface StepListProps {
  readonly items: readonly StepItem[];
  /** Accessible name of the list. */
  readonly label: string;
  /** `horizontal` joins the steps with arrows (download: 다운로드 중 → 무결성 확인 → 완료). */
  readonly orientation?: 'vertical' | 'horizontal' | undefined;
}

const HIDDEN_STATE: Readonly<Record<StepState, string>> = {
  done: '완료',
  active: '진행 중',
  pending: '대기',
  failed: '실패',
};

function StepSymbol({ state }: { readonly state: StepState }) {
  switch (state) {
    case 'done':
      return <span className="ome-step-symbol ome-step-symbol-done"><Icon name="check" size={18} strokeWidth={2} /></span>;
    case 'failed':
      return <span className="ome-step-symbol ome-step-symbol-failed"><Icon name="x" size={18} strokeWidth={2} /></span>;
    case 'active':
      return <span className="ome-step-symbol"><StatusDot tone="accent" /></span>;
    case 'pending':
      return <span className="ome-step-symbol"><StatusDot tone="muted" /></span>;
  }
}

export function StepList({ items, label, orientation = 'vertical' }: StepListProps) {
  return (
    <ol className={`ome-steps ome-steps-${orientation}`} aria-label={label}>
      {items.map((item, index) => (
        <Fragment key={item.id}>
          {orientation === 'horizontal' && index > 0 && (
            <li className="ome-steps-arrow" aria-hidden="true"><Icon name="arrow-right" size={14} /></li>
          )}
          <li className={`ome-step ome-step-${item.state}`} aria-current={item.state === 'active' ? 'step' : undefined}>
            <div className="ome-step-line">
              <StepSymbol state={item.state} />
              <span className="ome-step-label">{item.label}</span>
              <span className="ome-visually-hidden">{HIDDEN_STATE[item.state]}</span>
            </div>
            {orientation === 'vertical' && item.children !== undefined && (
              <div className="ome-step-detail">{item.children}</div>
            )}
          </li>
        </Fragment>
      ))}
    </ol>
  );
}
