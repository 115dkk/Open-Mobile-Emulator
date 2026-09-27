// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
import type { AppIssue } from '../contracts';
import { Icon } from './Icon';

/** `AppIssue` at the top of a screen: what happened, then what to do. The raw error is never shown. */
export function IssueNotice({ issue }: { readonly issue: AppIssue }) {
  return (
    <div className="ome-issue" role="alert">
      <span className="ome-issue-icon"><Icon name="alert-circle" size={18} strokeWidth={2} /></span>
      <div className="ome-issue-text">
        <p className="ome-issue-message">{issue.message}</p>
        {issue.nextAction !== null && <p className="ome-issue-next">{issue.nextAction}</p>}
      </div>
    </div>
  );
}
