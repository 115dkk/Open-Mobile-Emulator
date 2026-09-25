#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
set -euo pipefail

BUILD_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -L)
# QEMU configure rejects whitespace in source/build paths, and Meson resolves a
# SUBST drive back to the real path (os.path.realpath), which breaks
# "meson subprojects download" with "path is on mount 'C:', start on mount 'Z:'".
# So the work tree (src, out, dist) may live outside the repository:
# Build-Qemu.ps1 sets OME_QEMU_BUILD_WORK to a whitespace-free directory.
# https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/configure
WORK="$BUILD_ROOT"
if [[ -n "${OME_QEMU_BUILD_WORK:-}" ]]; then
    WORK=$(cygpath -u -- "$OME_QEMU_BUILD_WORK")
fi
OUT="$WORK/out"
DIST="$WORK/dist"
SOURCE="$WORK/src/qemu"
# shellcheck source=pins.env
source "$BUILD_ROOT/pins.env"
export LC_ALL=C

fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

init_log() {
    [[ "${MSYSTEM:-}" == "$MSYS2_ENV" ]] || fail 'Run in MSYS2 UCRT64 (use Build-Qemu.ps1).'
    mkdir -p -- "$OUT/logs" "$DIST"
    exec > >(tee -a "$OUT/logs/$1.log") 2>&1
    printf '\n[%s] %s\n' "$(date -u +%FT%TZ)" "$1"
}

input_hashes() (
    cd -- "$BUILD_ROOT"
    sha256sum -- pins.env common.sh build-qemu.sh Build-Qemu.ps1 make-source-offer.sh make-third-party.sh
    shopt -s nullglob
    for patch in patches/*.patch; do sha256sum -- "$patch"; done
)

require_source() {
    [[ -d "$SOURCE/.git" ]] || fail 'Run the clone step first.'
    [[ -f "$OUT/qemu-commit.txt" ]] || fail 'Missing clone provenance; run clone.'
    [[ "$(git -C "$SOURCE" rev-parse HEAD)" == "$(cat "$OUT/qemu-commit.txt")" ]] || fail 'Source HEAD changed.'
    [[ -z "$QEMU_COMMIT" || "$(cat "$OUT/qemu-commit.txt")" == "$QEMU_COMMIT" ]] || fail 'Pinned commit mismatch.'
    git -C "$SOURCE" diff --quiet || fail 'Unstaged source edits are not permitted.'
    [[ -z "$(git -C "$SOURCE" ls-files --others --exclude-standard)" ]] || fail 'Untracked QEMU source files must not affect the build.'
}

# Archive the index, NOT HEAD: git apply --index includes modified and added files.
source_tree() { git -C "$SOURCE" write-tree; }

# Exact wraps/revisions come from upstream v11.1.1, not floating refs.
# https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/scripts/archive-source.sh
WRAPS=(keycodemapdb berkeley-softfloat-3 berkeley-testfloat-3)
verify_wraps() {
    local name revision actual
    for name in "${WRAPS[@]}"; do
        revision=$(sed -n 's/^revision *= *//p' "$SOURCE/subprojects/$name.wrap" | tr -d '\r')
        [[ "$revision" =~ ^[0-9a-f]{40}$ ]] || fail "Non-commit wrap pin: $name"
        actual=$(git -C "$SOURCE/subprojects/$name" rev-parse HEAD)
        [[ "$actual" == "$revision" ]] || fail "Wrap commit mismatch: $name"
    done
}

wrap_hashes() (
    cd -- "$SOURCE"
    for name in "${WRAPS[@]}"; do
        find "subprojects/$name" -name .git -prune -o -type f -print0
    done | sort -z | xargs -0 sha256sum
)

verify_build() {
    require_source
    [[ -f "$OUT/build-inputs.sha256" && -f "$OUT/build-tree.txt" && -f "$OUT/build-packages.txt" ]] || fail 'Run build first.'
    [[ "$(source_tree)" == "$(cat "$OUT/build-tree.txt")" ]] || fail 'Source index changed since build.'
    cmp -s <(input_hashes) "$OUT/build-inputs.sha256" || fail 'Build scripts/pins/patches changed; clean and rebuild.'
    cmp -s <(pacman -Q) "$OUT/build-packages.txt" || fail 'Installed packages changed since build; clean and rebuild.'
    cmp -s "$OUT/pacman-lock.txt" "$OUT/build-packages.txt" || fail 'Package lock differs from build.'
    verify_wraps
    verify_submodules
    cmp -s <(wrap_hashes) "$OUT/build-wraps.sha256" || fail 'Wrap sources changed since build.'
}

verify_dist() {
    verify_build
    [[ -f "$DIST/qemu/SHA256SUMS" && -f "$OUT/dist.sha256" ]] || fail 'Run dist first.'
    (cd -- "$DIST/qemu"; sha256sum -c "$OUT/dist.sha256")
    cmp -s <(cd -- "$DIST/qemu"; find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum) "$OUT/dist.sha256" || fail 'Distribution files added or removed.'
    cmp -s "$OUT/dist.sha256" "$DIST/qemu/SHA256SUMS" || fail 'Distribution inventory changed.'
}

verify_submodules() {
    local status
    status=$(git -C "$SOURCE" submodule status --recursive)
    [[ ! "$status" =~ (^|$'\n')[-+U] ]] || fail 'Firmware submodules do not match their pins.'
    cmp -s <(printf '%s\n' "$status") "$OUT/submodules-lock.txt" || fail 'Firmware submodule inventory changed.'
    git -C "$SOURCE" submodule foreach --quiet --recursive '
        test -z "$(git status --porcelain)" || exit 1
    ' || fail 'Firmware submodule has local changes.'
}
