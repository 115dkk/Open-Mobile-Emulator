# M2 stack versions and facts checked on 2026-09-26

Research by an ASTRA worker (WebFetch only; nothing was built or run). Registry pages and
release APIs were opened for every row; a cell says "unverified" where no page answered. The
pins chosen for the skeleton are in the last column: the set that already builds on this host in
the user's UAC 원격 승인기 project, with the Tauri security patch taken.

## Rust crates

| Crate | Latest stable | Released | Pin in host/ | Why |
|---|---|---|---|---|
| tauri | 2.11.6 | 2026-09-19 | 2.11.6 | Fixes GHSA-w28w-mhc8-qvjv (channel data bound to its originating webview) |
| tauri-build | 2.6.3 | 2026-06-17 | 2.6.3 | |
| tauri-plugin-single-instance | 2.4.5 | 2026-09-20 | 2.4.5 | |
| tauri-plugin-notification | 2.4.0 | 2026-08-31 | not yet | Works only for installed apps; dev shows the PowerShell identity |
| tauri-plugin-updater | 2.12.0 | 2026-09-20 | not yet | Signature verification cannot be disabled (docs) |
| tauri-plugin-dialog | 2.7.3 | 2026-08-31 | not used | File dialogs open from Rust (`rfd`), never from the webview |
| windows | 0.62.2 | 2025-10-06 | 0.62.2 | tauri itself depends on windows 0.61: bridge HWND as an integer |
| windows-sys | 0.61.2 | 2025-10-06 | not used | |
| tokio | 1.53.1 | 2026-07-20 | transitive only | Core crates stay synchronous (ADR-0001) |
| serde / serde_json | 1.0.229 / 1.0.151 | 2026-07 | 1.0.228 / 1.0.145 | Known-building set |
| thiserror | 2.0.21 | 2026-09-23 | 2.0.17 | Known-building set |
| ureq | 3.4.2 | 2026-09-13 | 3.4.2 | rustls; streaming download with Range |
| reqwest | 0.13.5 | 2026-09-08 | not used | |
| sha2 | 0.11.0 | 2026-03-25 | 0.10.9 | Known-building set |
| zip | 8.6.0 | 2026-04-25 | 8.6.0 | XAPK/APKS and the diagnostic bundle |
| rfd | 0.17.2 | 2026-01-12 | later | File picker from Rust |
| tauri-cli | 2.11.5 | 2026-09-19 | host has 2.11.4 | Installed as `cargo install tauri-cli` |

MSRV declared by tauri 2.11.6 is 1.77.2; the workspace pins 1.97.0.

## npm packages

| Package | Latest | Released | Pin in host/ |
|---|---|---|---|
| @tauri-apps/api | 2.11.1 | n/a | 2.11.1 |
| @tauri-apps/cli | 2.11.5 | n/a | 2.11.5 |
| react, react-dom | 19.3.0 | 2026-09-09 | 19.2.8 |
| typescript | 7.0.2 | 2026-08-20 | 6.0.3 |
| vite | 8.3.1 | 2026-09-24 | 8.2.2 |
| @vitejs/plugin-react | 6.1.1 | 2026-08-28 | 6.1.1 |
| eslint | 10.11.0 | 2026-09-18 | 10.10.0 |
| typescript-eslint | 8.70.1 | 2026-09-21 | 8.70.0 |
| eslint-plugin-react-hooks | 7.1.1 | n/a | 7.1.1 |
| eslint-plugin-react-refresh | 0.5.7 | 2026-09-14 | 0.5.6 |
| vitest | 5.0.2 | 2026-09-25 | 4.1.11 |
| @testing-library/react | 16.3.3 | 2026-08-27 | 16.3.3 |
| @testing-library/jest-dom | 7.0.1 | 2026-08-09 | 7.0.1 |
| @testing-library/user-event | 14.6.7 | 2026-09-02 | 14.6.7 |
| jsdom | 30.1.1 | 2026-09-22 | 30.0.1 |
| @playwright/test | 1.63.0 | 2026-09-04 | 1.63.0 |
| @types/react, @types/react-dom | 19.3.0 | n/a | 19.2.18 / 19.2.7 |
| @types/node | 26.6.3 | n/a | 24.13.3 (host Node 24.1.0) |

Upgrades to TypeScript 7, Vitest 5 and React 19.3 are deliberate, tested changes later, not part
of the skeleton.

## Tauri 2 facts used by the architecture

- `app.windows[].create: false` plus `WebviewWindowBuilder::from_config` and `on_navigation`
  (returning false cancels navigation). `withGlobalTauri` defaults to false; set explicitly anyway.
- Capability permissions for events are `core:event:allow-listen` and `core:event:allow-unlisten`.
  App commands do not need `core:default`; `AppManifest::commands()` in build.rs enables the
  per-command ACL that `capabilities/main.json` lists.
- CSP `connect-src ipc: http://ipc.localhost`; `https://ipc.localhost` applies when
  `useHttpsScheme` is on. Both are local custom-protocol origins, not network endpoints.
- `WebviewWindow::hwnd()` returns `windows::Win32::Foundation::HWND` from windows 0.61.
- `tray-icon` feature and `TrayIconBuilder`; `Emitter::emit` for snapshot events.
- Notifications need an installed app (AppUserModelID shortcut); `tauri dev` shows PowerShell.
- Updater: `plugins.updater.pubkey` holds the key contents, `endpoints` array, `.sig` per bundle,
  verification cannot be disabled; Windows `installMode` passive (default), basicUi, quiet.
- NSIS `installMode` currentUser (default), perMachine, both; `webviewInstallMode` skip,
  downloadBootstrapper (default, `go.microsoft.com` then redirects), embedBootstrapper,
  offlineInstaller, fixedRuntime. The product uses `skip` (docs/NETWORK.md).
- Drag and drop: with `dragDropEnabled: true` Tauri delivers native drop events with paths;
  HTML5 drop is not delivered on Windows in that mode. The shell handles `WindowEvent::DragDrop`
  in Rust (ADR-0003).
- Tauri 2.11.6 default features: wry, compression, common-controls-v6, dynamic-acl, x11, dbus.
  The shell turns defaults off and enables wry, compression, common-controls-v6, tray-icon.

## Win32 facts for window hosting (windows 0.62.2 feature names)

| API | Feature | Note |
|---|---|---|
| SetParent, SetWindowLongPtrW, SetWindowPos, EnumWindows, GetWindowThreadProcessId, SetWindowsHookExW | `Win32_UI_WindowsAndMessaging` | GetWindowThreadProcessId returns the thread id; the PID is the out parameter |
| GetDpiForWindow, SetThreadDpiHostingBehavior | `Win32_UI_HiDpi` | Mixed hosting applies to windows created on that thread afterwards |
| AttachThreadInput | `Win32_System_Threading` | Not a fix for focus; avoid in the skeleton |
| ShellExecuteExW | `Win32_UI_Shell` | runas verb for the setup helper |
| WHvGetCapability | `Win32_System_Hypervisor` | Truth for "is WHPX usable" |

SDL2 registers its window class as `SDL_app` by default, but a caller can change it: find the
guest window by QEMU's PID and use the class only as a secondary check. Cross-process `SetParent`
may reset the child process's DPI awareness; the M2 spike measures it.

## Sources (opened 2026-09-26)

docs.rs crate pages for every crate above; registry.npmjs.org `latest` documents and GitHub
release APIs for every npm package; https://v2.tauri.app/security/capabilities/ ,
https://v2.tauri.app/reference/acl/core-permissions/ , https://v2.tauri.app/security/csp/ ,
https://v2.tauri.app/plugin/notification/ , https://v2.tauri.app/plugin/updater/ ,
https://v2.tauri.app/plugin/single-instance/ , https://v2.tauri.app/reference/config/ ,
https://v2.tauri.app/distribute/windows-installer/ , https://schema.tauri.app/config/2 ,
https://docs.rs/crate/tauri/2.11.6/source/Cargo.toml ,
https://raw.githubusercontent.com/tauri-apps/tauri/tauri-bundler-v2.8.1/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi ,
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent ,
https://devblogs.microsoft.com/oldnewthing/20130412-00/?p=4683 ,
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw ,
https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc ,
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddpihostingbehavior ,
https://raw.githubusercontent.com/libsdl-org/SDL/SDL2/src/video/windows/SDL_windowsevents.c ,
https://docs.rs/crate/windows/0.62.2/features .
