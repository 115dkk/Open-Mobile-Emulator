#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
init_log source-offer
verify_dist

commit=$(cat "$OUT/qemu-commit.txt")
shortcommit=${commit:0:12}
name="qemu-source-offer-$QEMU_TAG-$shortcommit"
stage=$(mktemp -d "$OUT/source-offer.XXXXXX")
root="$stage/$name"
mkdir -p -- "$root/qemu-build/out" "$root/qemu-build/src/qemu"

# HEAD alone loses git apply changes. Archive the exact index tree used for build.
# Upstream's archive-source.sh likewise does not rely on an unmodified HEAD alone:
# https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/scripts/archive-source.sh
# Do not invoke that script here: it may download or modify the source checkout.
git -C "$SOURCE" archive --format=tar "$(cat "$OUT/build-tree.txt")" |
    tar -xf - -C "$root/qemu-build/src/qemu"
cp -R -- "$BUILD_ROOT/patches" "$root/qemu-build/"
for file in build-qemu.sh Build-Qemu.ps1 pins.env common.sh make-source-offer.sh make-third-party.sh README.md; do
    cp -- "$BUILD_ROOT/$file" "$root/qemu-build/"
done
for file in pacman-lock.txt pacman-requested.txt qemu-commit.txt subprojects-lock.txt submodules-lock.txt configure-options.txt build-date.txt build-inputs.sha256 build-tree.txt build-wraps.sha256; do
    cp -- "$OUT/$file" "$root/qemu-build/out/"
done
cp -- "$OUT/dist.sha256" "$root/BINARY-SHA256SUMS"

# Include firmware submodule source and nested dependencies at recorded commits.
verify_submodules
export OME_SOURCE_OFFER_ROOT="$root/qemu-build/src/qemu"
git -C "$SOURCE" submodule foreach --quiet --recursive '
    export OME_SUBMODULE_PATH="$displaypath"
    bash -c '\''set -euo pipefail
        git archive --format=tar --prefix="$OME_SUBMODULE_PATH/" HEAD |
            tar -xf - -C "$OME_SOURCE_OFFER_ROOT"
    '\'' || exit 1
'
for wrap in "${WRAPS[@]}"; do
    # Includes Meson-applied packagefiles overlays, excludes every .git directory/file.
    tar -C "$SOURCE" --exclude=.git -cf - "subprojects/$wrap" |
        tar -xf - -C "$root/qemu-build/src/qemu"
done
verify_dist

cat > "$root/SOURCE-OFFER.txt" <<TEXT
Open Mobile Emulator: corresponding QEMU source
Upstream: $QEMU_GIT_URL
Tag: $QEMU_TAG
Full upstream commit: $commit
Patched source tree: $(cat "$OUT/build-tree.txt")
Build date (UTC): $(cat "$OUT/build-date.txt")

This archive contains the complete corresponding source for the shipped QEMU
executables identified by BINARY-SHA256SUMS: the exact patched QEMU tree,
commit-pinned subprojects, firmware submodules, all local patches, and the
MSYS2 build scripts. MSYS2 package versions are listed below and in
qemu-build/out/pacman-lock.txt. Patches are already applied in src/qemu.

Scope: this statement covers QEMU, not the independently distributed MSYS2 DLLs.
Those packages require their own license notices and, where applicable, source
packages/relinking material before release. See THIRD_PARTY.generated.md.
The presence of this archive alone does not authorize release of the DLL bundle.
Prebuilt firmware retained in QEMU's pc-bios tree is included along with its
upstream source pins; firmware reproducibility is not claimed by this script.

To rebuild the exported QEMU source without Git metadata, install the recorded
MSYS2 dependencies and use the configure options in qemu-build/out/, substituting
an absolute local prefix, then ninja and ninja install. Use a whitespace-free
source/build path. The normal clone step requires Git metadata and is for a
fresh upstream checkout, not for this already-patched exported tree.

MSYS2 package lock:
TEXT
cat "$OUT/pacman-lock.txt" >> "$root/SOURCE-OFFER.txt"
if [[ -f "$OUT/THIRD_PARTY.generated.md" ]]; then
    cp -- "$OUT/THIRD_PARTY.generated.md" "$root/THIRD_PARTY.generated.md"
fi
# Fixed metadata gives repeatable archives for identical recorded build inputs.
# GNU tar is supplied by the MSYS2 environment, not downloaded here.
build_date=$(cat "$OUT/build-date.txt")
(cd -- "$stage"; tar --sort=name --mtime="$build_date" --owner=0 --group=0 --numeric-owner -cf - "$name") |
    gzip -n > "$DIST/$name.tar.gz.tmp"
mv -- "$DIST/$name.tar.gz.tmp" "$DIST/$name.tar.gz"
(cd -- "$DIST"; find qemu -type f -print0; find . -maxdepth 1 -name 'qemu-source-offer-*.tar.gz' -type f -print0) |
    sort -z | (cd -- "$DIST"; xargs -0 sha256sum) > "$DIST/SHA256SUMS.tmp"
mv -- "$DIST/SHA256SUMS.tmp" "$DIST/SHA256SUMS"
printf 'Source offer: %s\nChecksums: %s\n' "$DIST/$name.tar.gz" "$DIST/SHA256SUMS"
# Keep staging source for inspection; -Clean removes it explicitly.
