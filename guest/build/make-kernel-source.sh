#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Archives the exact kernel source and configuration used by a guest-image build.
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <tree> <out>" >&2
    exit 2
fi

TREE=$(realpath "$1")
mkdir -p "$2"
OUT=$(realpath "$2")
MANIFEST="$TREE/.repo/manifests/ome.xml"
KERNEL="$TREE/kernel/x86/common"
CONFIG="$TREE/out/target/product/x86_64/obj/kernel/.config"
RELEASE_FILE="$TREE/out/target/product/x86_64/obj/kernel/include/config/kernel.release"

if [ ! -f "$MANIFEST" ]; then
    echo "manifest snapshot not found: $MANIFEST" >&2
    exit 1
fi
if [ ! -d "$KERNEL/.git" ] && [ ! -f "$KERNEL/.git" ]; then
    echo "kernel repository not found: $KERNEL" >&2
    exit 1
fi
if [ ! -f "$CONFIG" ]; then
    echo "kernel configuration not found: $CONFIG" >&2
    exit 1
fi

KERNEL_INFO=$(python3 - "$MANIFEST" "$TREE" <<'PY'
from pathlib import Path
import subprocess
import sys
from urllib.parse import urljoin, urlparse
import xml.etree.ElementTree as ET

manifest = Path(sys.argv[1])
tree = Path(sys.argv[2])
root = ET.parse(manifest).getroot()
projects = [p for p in root.findall("project") if p.get("path") == "kernel/x86/common"]
if len(projects) != 1:
    raise SystemExit(f"expected one kernel/x86/common project; found {len(projects)}")
project = projects[0]
default = root.find("default")
remote_name = project.get("remote") or (default.get("remote", "") if default is not None else "")
remotes = {r.get("name", ""): r.get("fetch", "") for r in root.findall("remote")}
fetch = remotes.get(remote_name)
if fetch is None:
    repository = f"{remote_name}/{project.get('name', '')}"
else:
    parsed = urlparse(fetch)
    if parsed.scheme or fetch.startswith("git@"):
        base = fetch.rstrip("/")
    else:
        try:
            origin = subprocess.run(
                ["git", "-C", str(tree / ".repo" / "manifests"), "remote", "get-url", "origin"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            origin = ""
        base = urljoin(origin.rstrip("/") + "/", fetch).rstrip("/") if origin else ""
    repository = f"{base}/{project.get('name', '').lstrip('/')}" if base else f"{remote_name}/{project.get('name', '')}"
print(project.get("revision", ""))
print(repository)
PY
)
mapfile -t KERNEL_FIELDS <<< "$KERNEL_INFO"
if [ "${#KERNEL_FIELDS[@]}" -ne 2 ] || [ -z "${KERNEL_FIELDS[0]}" ]; then
    echo "could not read the kernel revision and repository from $MANIFEST" >&2
    exit 1
fi
REVISION=${KERNEL_FIELDS[0]}
REPOSITORY=${KERNEL_FIELDS[1]}
HEAD=$(git -C "$KERNEL" rev-parse HEAD)
if [ "$HEAD" != "$REVISION" ]; then
    echo "kernel revision mismatch: manifest=$REVISION tree=$HEAD" >&2
    exit 1
fi

if [ -s "$RELEASE_FILE" ]; then
    KERNEL_VERSION=$(tr -d '\r\n' < "$RELEASE_FILE")
else
    VERSION=$(sed -n 's/^VERSION = //p' "$KERNEL/Makefile" | head -n 1)
    PATCHLEVEL=$(sed -n 's/^PATCHLEVEL = //p' "$KERNEL/Makefile" | head -n 1)
    SUBLEVEL=$(sed -n 's/^SUBLEVEL = //p' "$KERNEL/Makefile" | head -n 1)
    LOCALVERSION=$(sed -n 's/^CONFIG_LOCALVERSION=//p' "$CONFIG" | head -n 1 | sed 's/^"//;s/"$//')
    if [ -z "$VERSION" ] || [ -z "$PATCHLEVEL" ] || [ -z "$SUBLEVEL" ]; then
        echo "could not derive the kernel version" >&2
        exit 1
    fi
    KERNEL_VERSION="$VERSION.$PATCHLEVEL.$SUBLEVEL$LOCALVERSION"
fi

SHORT_REVISION=${REVISION:0:12}
ARCHIVE_NAME="kernel-source-$KERNEL_VERSION-$SHORT_REVISION.tar.gz"
CONFIG_NAME="kernel-config-$KERNEL_VERSION.txt"
ARCHIVE="$OUT/$ARCHIVE_NAME"
ARCHIVE_TMP="$ARCHIVE.tmp"

rm -f "$ARCHIVE_TMP"
git -C "$KERNEL" archive --format=tar --prefix="kernel_common-$SHORT_REVISION/" "$REVISION" | gzip -6 > "$ARCHIVE_TMP"
gzip -t "$ARCHIVE_TMP"
mv -f "$ARCHIVE_TMP" "$ARCHIVE"
cp "$CONFIG" "$OUT/$CONFIG_NAME"
ARCHIVE_SHA256=$(sha256sum "$ARCHIVE" | awk '{print $1}')
GENERATED_UTC=$(date -u +%Y-%m-%dT%H:%M:%SZ)
cat > "$OUT/kernel-source.txt" <<EOF
repository=$REPOSITORY
revision=$REVISION
kernel_version=$KERNEL_VERSION
archive=$ARCHIVE_NAME
archive_sha256=$ARCHIVE_SHA256
config=$CONFIG_NAME
generated_utc=$GENERATED_UTC
EOF
