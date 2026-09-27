## How to build with Open Mobile Emulator (OME)

OME is a Windows desktop app (Tauri + React) that runs an Android operating system in a virtual machine. Its UI is a 64px icon rail on the left and one screen at a time on the right: 무대 (the stage, where the operating system window sits), 앱, 입력, 표시, 설정, plus a seven-step first-run wizard. Build screens out of the 19 components on `window.OME`; do not draw look-alikes.

### Setup

No provider is needed. Load `styles.css` once; every token, both font families and every component rule come from its `@import` closure (`_ds_bundle.css` starts with the token block). The theme is dark by default (`color-scheme: dark` on `:root`). Light is chosen by the operating system (`prefers-color-scheme: light`) or by `<html data-theme="light">`; `data-theme="dark"` forces dark. Never hard-code a color: every value below flips with the theme.

### Styling idiom

Components style themselves with private `ome-*` classes. Do not write `ome-*` classes yourself and do not restyle a component; pass its props. Your own layout glue (rows, columns, sections, spacing) is inline style or your own CSS built only from these tokens:

- Color: `--bg` (window), `--surface` (panels, rows), `--surface-raised` (inputs, pressed), `--stage-frame` (the stage), `--text`, `--muted` (help sentences, captions), `--line`, `--line-strong`, `--accent`, `--accent-hover`, `--on-accent` (text on accent), `--accent-soft` (selected background), `--success`, `--warning`, `--danger`, `--shadow-surface`, `--image-outline`.
- Type, as `font:` shorthands: `--type-title` (screen title, 22px), `--type-section` (16px), `--type-body` (14px), `--type-label` (13px semibold), `--type-caption` (12px), `--type-mono` (13px Sarasa Mono K for paths, ids, addresses), `--type-readout` (slider readouts). Families: `--font-ui` (Pretendard), `--font-mono`.
- Space: `--space-1` 4, `--space-2` 8, `--space-3` 12, `--space-4` 16, `--space-5` 20, `--space-6` 24, `--space-8` 32, `--space-10` 40, `--space-12` 48 (px).
- Shape and size: `--radius-control` 6, `--radius-stage` 8, `--radius-card` 12, `--rail-width` 64, `--toolbar-height` 44, `--statusbar-height` 28, `--target-min` 40, `--target-primary` 44, `--disabled-opacity`.
- Motion: `--motion-fast` 120ms, `--motion-normal` 180ms, `--ease-standard`. Nothing else animates.

Screens have no cards: a settings screen is a title (`ScreenHeader`) followed by section headings (`font: var(--type-section)`) and `SettingRow`s separated by 1px `--line`. Status is a `StatusDot` beside words or a `Chip`; the dot never carries the meaning alone. Icons are `Icon` with one of the 37 bundled names (see `components/general/Icon/Icon.d.ts`); `IconButton` always has a `label`.

### Copy (Korean, DESIGN.md section 9)

UI text is Korean in the `-십시오` register. Say what happened, then what to do (`IssueNotice` has exactly those two lines). Name things as the user sees them: `운영체제` (the Android system), `가상 머신` (QEMU), `앱`, `기기 ID`; never `게스트`, `VM`, `QEMU`, `adb` in a sentence a non-developer reads (the adb address is the one exception, in `--type-mono`). Buttons are verb nouns (`계속`, `다시 확인`, `설치`, `제거`, `삭제`); `Button variant="primary"` is the one main action of a view, `danger` is a quiet red row action, `danger-solid` confirms something that cannot be undone inside a `Dialog`. Read `guidelines/DESIGN.md` before composing a new screen.

### Where the truth lives

`styles.css` and `_ds_bundle.css` (tokens at the top, then every component rule); `components/general/<Name>/<Name>.prompt.md` for props and examples; `guidelines/DESIGN.md` for direction, color, type, spacing, motion, icons, state grammar, overlay and copy rules.

### An idiomatic section

```jsx
const { ScreenHeader, SettingRow, Toggle, Select, Button } = window.OME;

<div style={{ padding: 'var(--space-6) var(--space-8)', background: 'var(--bg)', color: 'var(--text)', font: 'var(--type-body)' }}>
  <ScreenHeader title="설정" />
  <h2 style={{ font: 'var(--type-section)', margin: 'var(--space-6) 0 var(--space-2)' }}>고급</h2>
  <SettingRow label="다른 PC에서 연결 허용" help="다시 시작해야 적용됩니다.">
    <Toggle checked={false} onChange={() => {}} label="다른 PC에서 연결 허용" />
  </SettingRow>
  <SettingRow label="창을 닫을 때" help="운영체제가 실행 중일 때 적용됩니다.">
    <Select label="창을 닫을 때" value="stop" onChange={() => {}}
      options={[{ value: 'stop', label: '운영체제 끄기' }, { value: 'tray', label: '트레이로 내리기' }]} />
  </SettingRow>
  <div style={{ display: 'flex', gap: 'var(--space-3)', justifyContent: 'flex-end', marginTop: 'var(--space-6)' }}>
    <Button>취소</Button>
    <Button variant="primary">저장</Button>
  </div>
</div>
```
