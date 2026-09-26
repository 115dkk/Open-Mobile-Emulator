# Network endpoints (R10)

Every host the product connects to is listed here. Code that reaches an address
not in this table is rejected in review. There is no telemetry; crash logs stay
on disk and are attached by the user by hand.

| Purpose | Host | Used by | Protocol | Notes |
|---|---|---|---|---|
| Guest ISO download | `downloads.sourceforge.net` and the SourceForge mirror pool under `dl.sourceforge.net` | installer, `launcher/Get-Artifacts.ps1` | HTTPS | URL and SHA-256 come from `manifests/artifacts.json`. The first host redirects to a mirror. |
| Google emulator image for blob extraction | `dl.google.com` | image builder only (P3), never the installed product | HTTPS | `vendor_google_emu-x86` flow (R3) |
| Self-update | `api.github.com`, `github.com`, `objects.githubusercontent.com` | product (M2 item 7) | HTTPS | Release lookup and signed asset download, user-initiated |

Not used on purpose: the Tauri installer's WebView2 bootstrapper download
(`go.microsoft.com` and its redirects). `host/app/tauri.conf.json` sets
`webviewInstallMode` to `skip`; the product requires the WebView2 runtime that
Windows 11 ships with, and the installer says so instead of downloading it.

Local-only sockets (not network endpoints):

| Purpose | Address |
|---|---|
| QMP control of QEMU | `127.0.0.1:4444` (TCP, default; configurable) |
| adb to the guest | `127.0.0.1:5555` via QEMU user-mode `hostfwd` |

Development machine tooling (winget, MSYS2 pacman, git) is not part of the
product and is not governed by this table.
