#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Builds every asset uploaded with a self-built Android 15 guest-image release.
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <tree> <dist> <out>" >&2
    exit 2
fi

START_SECONDS=$SECONDS
TREE=$(realpath "$1")
DIST=$(realpath "$2")
mkdir -p "$3"
OUT=$(realpath "$3")
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
DIST_NAME=$(basename "$DIST")

if [[ ! "$DIST_NAME" =~ ^([0-9]{8})-([[:alnum:]]+)$ ]]; then
    echo "dist directory must be named <YYYYMMDD>-<manifest-short>: $DIST_NAME" >&2
    exit 1
fi
BUILD_DATE=${BASH_REMATCH[1]}
MANIFEST_SHORT=${BASH_REMATCH[2]}
RELEASE_NAME="ome-android-15-x86_64-$BUILD_DATE-$MANIFEST_SHORT.iso"
RELEASE_ID="guest-android-15-$BUILD_DATE"
CHUNK_SIZE=1073741824

mapfile -t ISO_FILES < <(find "$DIST" -maxdepth 1 -type f -name '*.iso' -print)
mapfile -t ISO_SUM_FILES < <(find "$DIST" -maxdepth 1 -type f -name '*.iso.sha256' -print)
if [ "${#ISO_FILES[@]}" -ne 1 ] || [ "${#ISO_SUM_FILES[@]}" -ne 1 ]; then
    echo "expected one ISO and one ISO checksum in $DIST" >&2
    exit 1
fi
ISO=${ISO_FILES[0]}
ISO_SUM_FILE=${ISO_SUM_FILES[0]}
RECORDED_SHA256=$(awk 'NF {print tolower($1); exit}' "$ISO_SUM_FILE")
ACTUAL_SHA256=$(sha256sum "$ISO" | awk '{print $1}')
if [ -z "$RECORDED_SHA256" ] || [ "$RECORDED_SHA256" != "$ACTUAL_SHA256" ]; then
    echo "ISO checksum mismatch: recorded=$RECORDED_SHA256 calculated=$ACTUAL_SHA256" >&2
    exit 1
fi
ISO_SIZE=$(stat -c %s "$ISO")
PART_COUNT=$(((ISO_SIZE + CHUNK_SIZE - 1) / CHUNK_SIZE))
if [ "$PART_COUNT" -ge 10 ]; then
    echo "the ISO needs $PART_COUNT parts; one-digit split suffixes support at most 9" >&2
    exit 1
fi

"$HERE/make-notice-guest.sh" "$TREE" "$DIST" "$OUT"
"$HERE/make-kernel-source.sh" "$TREE" "$OUT"

PART_PREFIX="$OUT/$RELEASE_NAME.part"
find "$OUT" -maxdepth 1 -type f -name "$RELEASE_NAME.part[0-9]" -delete
split -b 1073741824 -d -a 1 "$ISO" "$PART_PREFIX"
mapfile -t PART_FILES < <(find "$OUT" -maxdepth 1 -type f -name "$RELEASE_NAME.part[0-9]" -print | LC_ALL=C sort)
if [ "${#PART_FILES[@]}" -ne "$PART_COUNT" ]; then
    echo "expected $PART_COUNT ISO parts; found ${#PART_FILES[@]}" >&2
    exit 1
fi

cp "$DIST/ome.xml" "$OUT/ome.xml"
cp "$DIST/pins.env" "$OUT/pins.env"
cp "$DIST/build.prop" "$OUT/build.prop"
mapfile -t BUILD_LOGS < <(find "$DIST" -maxdepth 1 -type f -name 'build-*.log' -printf '%f\n' | LC_ALL=C sort)
if [ "${#BUILD_LOGS[@]}" -eq 0 ]; then
    echo "no build logs found in $DIST" >&2
    exit 1
fi
tar -C "$DIST" -cJf "$OUT/build-logs.tar.xz" -- "${BUILD_LOGS[@]}"
printf '%s  %s\n' "$ACTUAL_SHA256" "$RELEASE_NAME" > "$OUT/$RELEASE_NAME.sha256"

rm -f "$OUT/release-assets.json" "$OUT/SHA256SUMS"
python3 - "$OUT" "$RELEASE_ID" "$RELEASE_NAME" "$ISO_SIZE" "$ACTUAL_SHA256" <<'PY'
from pathlib import Path
import hashlib
import json
import sys

out = Path(sys.argv[1])
release_id = sys.argv[2]
whole_name = sys.argv[3]
whole_size = int(sys.argv[4])
whole_sha256 = sys.argv[5]


def describe(path: Path) -> dict[str, str | int]:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return {
        "filename": path.name,
        "size_bytes": path.stat().st_size,
        "sha256": digest.hexdigest(),
    }


parts = sorted(out.glob(f"{whole_name}.part[0-9]"), key=lambda path: path.name)
excluded = {"release-assets.json", "SHA256SUMS"}
files = sorted(
    (
        path
        for path in out.iterdir()
        if path.is_file()
        and path.name not in excluded
        and not path.name.startswith(f"{whole_name}.part")
    ),
    key=lambda path: path.name,
)
payload = {
    "release": release_id,
    "whole": {
        "filename": whole_name,
        "size_bytes": whole_size,
        "sha256": whole_sha256,
    },
    "parts": [describe(path) for path in parts],
    "files": [describe(path) for path in files],
}
with (out / "release-assets.json").open("w", encoding="utf-8", newline="\n") as target:
    json.dump(payload, target, ensure_ascii=False, indent=2)
    target.write("\n")
PY

{
    echo "# whole image after concatenating the parts"
    printf '%s  %s\n' "$ACTUAL_SHA256" "$RELEASE_NAME"
    while IFS= read -r filename; do
        sha256sum "$OUT/$filename" | sed "s#  $OUT/#  #"
    done < <(find "$OUT" -maxdepth 1 -type f ! -name SHA256SUMS -printf '%f\n' | LC_ALL=C sort)
} > "$OUT/SHA256SUMS"

echo "release assets:"
find "$OUT" -maxdepth 1 -type f -printf '%f\t%s bytes\n' | LC_ALL=C sort
echo "elapsed_seconds=$((SECONDS - START_SECONDS))"
