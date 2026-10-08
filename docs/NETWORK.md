# Network endpoints (R10)

Every host the product connects to is listed here. Code that reaches an address
not in this table is rejected in review. There is no telemetry; crash logs stay
on disk and are attached by the user by hand.

| Purpose | Host | Used by | Protocol | Notes |
|---|---|---|---|---|
| Guest ISO download | `downloads.sourceforge.net` and the SourceForge mirror pool under `dl.sourceforge.net` | installer, `launcher/Get-Artifacts.ps1` | HTTPS | URL and SHA-256 come from `manifests/artifacts.json`. The first host redirects to a mirror. |
| Google emulator system image for blob-carrying image profiles | `dl.google.com` | installer, only after the user accepts the image's license (R3, ADR-0006); development image builder (`vendor_google_emu-x86`, never shipped) | HTTPS | URL and SHA-256 come from `manifests/artifacts.json` (`fetched_by: installer`). Extraction happens on the user's PC; nothing from the image is uploaded or redistributed. The installer part waits for CLAUDE.md section 8 item 9 |
| Self-built guest image download (ADR-0012) | `github.com`, `release-assets.githubusercontent.com` | installer (wizard S1.4 and Settings > 새 운영체제 설치), `fetched_by: installer` entries of `manifests/artifacts.json` whose URL is an OME GitHub release | HTTPS | URL, size and SHA-256 of every part come from the manifest (`parts`); `github.com/.../releases/download/...` redirects to `release-assets.githubusercontent.com` (seen 2026-10-02; the suffix `githubusercontent.com` is in the manifest's allowed hosts). The image is this project's own blob-free build, not a redistribution of an external artifact (R3) |
| Self-update | `api.github.com`, `github.com`, `objects.githubusercontent.com` | product (M2 item 7) | HTTPS | Release lookup once at start while the `auto_update_check` setting is on (default on) and on the settings button; asset download and install only when the user clicks |

Not used on purpose: the Tauri installer's WebView2 bootstrapper download
(`go.microsoft.com` and its redirects). `host/app/tauri.conf.json` sets
`webviewInstallMode` to `skip`; the product requires the WebView2 runtime that
Windows 11 ships with, and the installer says so instead of downloading it.

Opened in the user's default browser by the product (the product itself sends no
request to these hosts; they are listed so reviewers can match `HelpTopic` and the
help commands against this file):

| Purpose | Host | Opened by |
|---|---|---|
| Help pages, third-party notices, release notes, QEMU source offer | `github.com` | `open_help` |

Local-only sockets (not network endpoints):

| Purpose | Address |
|---|---|
| QMP control of QEMU | `127.0.0.1:4444` (TCP, default; configurable) |
| adb to the guest | `127.0.0.1:5555` via QEMU user-mode `hostfwd` |

Development machine tooling (winget, MSYS2 pacman, git) is not part of the
product and is not governed by this table.
