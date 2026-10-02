#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Generates NOTICE-guest.md, the Android notice index and the forbidden-file scan for a built ISO.
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <tree> <dist> <out>" >&2
    exit 2
fi

TREE=$(realpath "$1")
DIST=$(realpath "$2")
mkdir -p "$3"
OUT=$(realpath "$3")
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ISO_MOUNT=/mnt/ome-iso
EFS_MOUNT=/mnt/ome-efs
SYSTEM_MOUNT=/mnt/ome-sys

if [ ! -f "$DIST/ome.xml" ] || [ ! -f "$DIST/build.prop" ]; then
    echo "missing ome.xml or build.prop in $DIST" >&2
    exit 1
fi

mapfile -t ISO_FILES < <(find "$DIST" -maxdepth 1 -type f -name '*.iso' -print)
if [ "${#ISO_FILES[@]}" -ne 1 ]; then
    echo "expected exactly one ISO in $DIST; found ${#ISO_FILES[@]}" >&2
    exit 1
fi
ISO=${ISO_FILES[0]}

cleanup() {
    sudo umount "$SYSTEM_MOUNT" 2>/dev/null || true
    sudo umount "$EFS_MOUNT" 2>/dev/null || true
    sudo umount "$ISO_MOUNT" 2>/dev/null || true
}
trap cleanup EXIT

sudo mkdir -p "$ISO_MOUNT" "$EFS_MOUNT" "$SYSTEM_MOUNT"
mountpoint -q "$ISO_MOUNT" || sudo mount -o loop,ro "$ISO" "$ISO_MOUNT"
if [ ! -f "$ISO_MOUNT/system.efs" ]; then
    echo "system.efs not found in $ISO" >&2
    exit 1
fi
mountpoint -q "$EFS_MOUNT" || sudo mount -t erofs -o loop,ro "$ISO_MOUNT/system.efs" "$EFS_MOUNT"
if [ ! -f "$EFS_MOUNT/system.img" ]; then
    echo "system.img not found in system.efs" >&2
    exit 1
fi
mountpoint -q "$SYSTEM_MOUNT" || sudo mount -t ext4 -o loop,ro,noload "$EFS_MOUNT/system.img" "$SYSTEM_MOUNT"
if [ ! -f "$SYSTEM_MOUNT/system/etc/NOTICE.xml.gz" ]; then
    echo "system/etc/NOTICE.xml.gz not found in system.img" >&2
    exit 1
fi

sudo find "$SYSTEM_MOUNT" -xdev \( -type f -o -type l \) -printf '%P\n' | LC_ALL=C sort > "$OUT/file_list.txt"
sudo cp "$SYSTEM_MOUNT/system/etc/NOTICE.xml.gz" "$OUT/NOTICE-guest.xml.gz"
sudo chown "$(id -u):$(id -g)" "$OUT/NOTICE-guest.xml.gz"
python3 "$HERE/notice_guest.py" \
    --tree "$TREE" \
    --dist "$DIST" \
    --out "$OUT" \
    --file-list "$OUT/file_list.txt"
