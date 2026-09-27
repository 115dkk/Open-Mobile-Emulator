// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
// Apps (M2-SCREENS.md 3): the apps installed in the operating system, with 실행 and 제거 per row and
// 설치 above. While the operating system is off the list keeps its last values, dimmed, and the
// commands that need it have no button. The search box filters what is drawn and nothing else.
import { useState } from 'react';
import type { ScreenProps } from '../actions';
import type { AppItem, TransferStage } from '../contracts';
import {
  Button, Dialog, EmptyState, Field, IssueNotice, ProgressBar, ScreenHeader, StatusDot,
} from '../components';
import { formatPercent, formatTimestamp } from '../format';

const ACTIVE_INSTALL: readonly TransferStage[] = ['waiting', 'transferring', 'verifying'];

/** Installation day: `2026-09-24`. */
function formatDay(value: string): string {
  const stamp = formatTimestamp(value);
  const space = stamp.indexOf(' ');
  return space > 0 ? stamp.slice(0, space) : stamp;
}

// Letters and digits read with a final consonant: L(엘), M(엠), N(엔), R(알); 0(영, 십, 백),
// 1(일), 3(삼), 6(육), 7(칠), 8(팔).
const LETTERS_WITH_FINAL = new Set(['L', 'M', 'N', 'R']);
const DIGITS_WITH_FINAL = new Set(['0', '1', '3', '6', '7', '8']);

/**
 * The object particle after an app name, when it can be known from the last character: a Hangul
 * syllable, a digit, or a lone Latin letter (`샘플 앱 A`). Anything else returns null.
 */
function objectParticle(name: string): '을' | '를' | null {
  const trimmed = name.trim();
  const last = trimmed.at(-1);
  if (last === undefined) return null;
  const code = last.charCodeAt(0);
  if (code >= 0xac00 && code <= 0xd7a3) return (code - 0xac00) % 28 === 0 ? '를' : '을';
  if (/^[0-9]$/u.test(last)) return DIGITS_WITH_FINAL.has(last) ? '을' : '를';
  const before = trimmed.at(-2);
  if (/^[A-Za-z]$/u.test(last) && (before === undefined || !/^[A-Za-z]$/u.test(before))) {
    return LETTERS_WITH_FINAL.has(last.toUpperCase()) ? '을' : '를';
  }
  return null;
}

function removalTitle(app: AppItem): string {
  const particle = objectParticle(app.label);
  return particle === null ? `${app.label} 앱을 제거합니다.` : `${app.label}${particle} 제거합니다.`;
}

function matches(app: AppItem, query: string): boolean {
  const needle = query.trim().toLocaleLowerCase();
  if (needle === '') return true;
  return app.label.toLocaleLowerCase().includes(needle) || app.package.toLocaleLowerCase().includes(needle);
}

export function AppsScreen({ snapshot, actions }: ScreenProps) {
  const { apps } = snapshot;
  const [query, setQuery] = useState('');
  const [removing, setRemoving] = useState<AppItem | null>(null);
  const install = apps.install;
  const installing = install !== null && ACTIVE_INSTALL.includes(install.stage);
  const ratio = install !== null && install.totalBytes !== null && install.totalBytes > 0
    ? install.doneBytes / install.totalBytes : null;
  const shown = apps.items.filter((app) => matches(app, query));

  const confirmRemoval = async () => {
    const target = removing;
    setRemoving(null);
    if (target !== null) await actions.appUninstall(target.package);
  };

  return (
    <div className="ome-apps">
      <ScreenHeader title="앱">
        <div className="ome-apps-search">
          <Field
            label="앱 검색"
            labelHidden
            type="search"
            icon="search"
            placeholder="이름이나 패키지로 찾기"
            value={query}
            onChange={setQuery}
          />
        </div>
        <Button icon="folder-open" onClick={actions.openScreenshotsFolder}>스크린샷 폴더 열기</Button>
        {apps.available && <Button variant="primary" icon="plus" onClick={actions.appInstallPick}>설치</Button>}
      </ScreenHeader>
      <div className="ome-apps-body">
        {snapshot.issue !== null && <IssueNotice issue={snapshot.issue} />}
        {installing && (
          <div className="ome-install-line">
            <StatusDot tone="accent" />
            <span><span className="ome-mono">{install.label}</span> 설치 중</span>
            <div className="ome-install-bar"><ProgressBar value={ratio} label="설치 진행률" size="thin" /></div>
            {ratio !== null && <span className="ome-readout">{formatPercent(ratio)}</span>}
            <Button onClick={actions.appInstallCancel}>취소</Button>
          </div>
        )}
        <div className={apps.available ? 'ome-apps-list' : 'ome-apps-list ome-apps-list-stale'}>
          {apps.items.length === 0 ? <EmptyState icon="apps" title="설치된 앱이 없습니다." />
            : shown.length === 0 ? <EmptyState icon="search" title="검색 결과가 없습니다." />
              : (
                <table className="ome-apps-table" aria-label="설치된 앱">
                  <colgroup>
                    <col className="ome-apps-col-name" />
                    <col />
                    <col className="ome-apps-col-version" />
                    <col className="ome-apps-col-date" />
                    {apps.available && <col className="ome-apps-col-actions" />}
                  </colgroup>
                  <thead>
                    <tr>
                      <th scope="col">이름</th>
                      <th scope="col">패키지</th>
                      <th scope="col">버전</th>
                      <th scope="col">설치일</th>
                      {apps.available && <th scope="col" className="ome-apps-actions-head">동작</th>}
                    </tr>
                  </thead>
                  <tbody>
                    {shown.map((app) => (
                      <tr key={app.package}>
                        <td className="ome-apps-name">{app.label}</td>
                        <td><span className="ome-mono ome-muted">{app.package}</span></td>
                        <td className="ome-apps-version">{app.versionName ?? ''}</td>
                        <td className="ome-apps-date">{app.installedAt === null ? '' : formatDay(app.installedAt)}</td>
                        {apps.available && (
                          <td className="ome-apps-actions">
                            <Button icon="play" onClick={() => actions.appLaunch(app.package)}>실행</Button>
                            <Button variant="danger" onClick={() => { setRemoving(app); }}>제거</Button>
                          </td>
                        )}
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
        </div>
        <p className="ome-caption">앱 {apps.items.length}개.</p>
      </div>
      <Dialog
        open={removing !== null}
        title={removing === null ? '' : removalTitle(removing)}
        confirmLabel="제거"
        tone="danger"
        onConfirm={confirmRemoval}
        onCancel={() => { setRemoving(null); }}
      />
    </div>
  );
}
