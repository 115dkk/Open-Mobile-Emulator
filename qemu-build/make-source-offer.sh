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
build_date=$(cat "$OUT/build-date.txt")
stage=$(mktemp -d "$OUT/source-offer.XXXXXX")
root="$stage/$name"
mkdir -p -- "$root/qemu-build/out"

# Regular files that we author or record go into the staging tree.
cp -R -- "$BUILD_ROOT/patches" "$root/qemu-build/"
for file in build-qemu.sh Build-Qemu.ps1 pins.env common.sh make-source-offer.sh make-third-party.sh README.md; do
    cp -- "$BUILD_ROOT/$file" "$root/qemu-build/"
done
for file in pacman-lock.txt pacman-requested.txt qemu-commit.txt subprojects-lock.txt submodules-lock.txt configure-options.txt build-date.txt build-inputs.sha256 build-tree.txt build-wraps.sha256; do
    cp -- "$OUT/$file" "$root/qemu-build/out/"
done
cp -- "$OUT/dist.sha256" "$root/BINARY-SHA256SUMS"
if [[ -f "$OUT/THIRD_PARTY.generated.md" ]]; then
    cp -- "$OUT/THIRD_PARTY.generated.md" "$root/THIRD_PARTY.generated.md"
fi

cat > "$root/SOURCE-OFFER.txt" <<TEXT
Open Mobile Emulator: corresponding QEMU source
Upstream: $QEMU_GIT_URL
Tag: $QEMU_TAG
Full upstream commit: $commit
Patched source tree: $(cat "$OUT/build-tree.txt")
Build date (UTC): $build_date

This archive contains the complete corresponding source for the shipped QEMU
executables identified by BINARY-SHA256SUMS: the exact patched QEMU tree
(qemu-build/src/qemu, taken from the git index tree used for the build),
commit-pinned Meson subprojects, the firmware submodules at their recorded
commits, all local patches (already applied in the tree), and the MSYS2 build
scripts. MSYS2 package versions are listed below and in
qemu-build/out/pacman-lock.txt.

Scope: this statement covers QEMU, not the independently distributed MSYS2 DLLs.
Those packages require their own license notices and, where applicable, source
packages/relinking material before release. See THIRD_PARTY.generated.md.
The presence of this archive alone does not authorize release of the DLL bundle.
Prebuilt firmware retained in QEMU's pc-bios tree is included along with its
upstream source pins; firmware reproducibility is not claimed by this script.

To rebuild: install the recorded MSYS2 dependencies, run configure with the
options in qemu-build/out/configure-options.txt (substituting an absolute local
prefix and a whitespace-free source/build path), then ninja and ninja install.
The normal clone step requires Git metadata and is for a fresh upstream checkout,
not for this already-patched exported tree.

MSYS2 package lock:
TEXT
cat "$OUT/pacman-lock.txt" >> "$root/SOURCE-OFFER.txt"

# Source trees are streamed straight from git into tar parts and concatenated.
# They are never extracted on the Windows host: upstream trees contain symbolic
# links (for example roms/edk2/EmulatorPkg/Unix/Host/X11IncludeHack) that an
# unprivileged Windows user cannot materialise, and archiving from git keeps the
# entries exactly as upstream recorded them.
# https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/scripts/archive-source.sh
verify_submodules
prefix="$name/qemu-build/src/qemu"
tar_opts=(--sort=name --mtime="$build_date" --owner=0 --group=0 --numeric-owner)
tar -C "$stage" "${tar_opts[@]}" -cf "$stage/00-files.tar" "$name"
git -C "$SOURCE" archive --format=tar --prefix="$prefix/" "$(cat "$OUT/build-tree.txt")" > "$stage/10-qemu.tar"
export OME_OFFER_STAGE="$stage" OME_OFFER_PREFIX="$prefix"
git -C "$SOURCE" submodule foreach --quiet --recursive '
    part="$OME_OFFER_STAGE/20-sub-$(printf "%s" "$displaypath" | tr "/" "_").tar"
    git archive --format=tar --prefix="$OME_OFFER_PREFIX/$displaypath/" HEAD > "$part"
'
for wrap in "${WRAPS[@]}"; do
    # Includes Meson-applied packagefiles overlays, excludes every .git directory/file.
    tar -C "$SOURCE" --exclude=.git "${tar_opts[@]}" --transform="s|^|$prefix/|" \
        -cf "$stage/30-wrap-$wrap.tar" "subprojects/$wrap"
done
verify_dist

final="$stage/$name.tar"
cp -- "$stage/00-files.tar" "$final"
for part in "$stage"/10-*.tar "$stage"/20-*.tar "$stage"/30-*.tar; do
    tar --concatenate --file="$final" "$part"
done
gzip -n < "$final" > "$DIST/$name.tar.gz.tmp"
mv -- "$DIST/$name.tar.gz.tmp" "$DIST/$name.tar.gz"
rm -f -- "$stage"/*.tar
(cd -- "$DIST"; find qemu -type f -print0; find . -maxdepth 1 -name 'qemu-source-offer-*.tar.gz' -type f -print0) |
    sort -z | (cd -- "$DIST"; xargs -0 sha256sum) > "$DIST/SHA256SUMS.tmp"
mv -- "$DIST/SHA256SUMS.tmp" "$DIST/SHA256SUMS"
printf 'Source offer: %s\nMembers: %s\nChecksums: %s\n' "$DIST/$name.tar.gz" "$(tar -tzf "$DIST/$name.tar.gz" | wc -l)" "$DIST/SHA256SUMS"
# Keep the staging tree of regular files for inspection; -Clean removes it explicitly.
