#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
STEP=${1:-all}
case "$STEP" in all|deps|clone|configure|build|dist) ;; *) fail 'Unknown build step.' ;; esac
init_log "build-$STEP"
[[ ! "$WORK" =~ [[:space:]:] ]] || fail 'QEMU rejects spaces/colons in the work tree; set OME_QEMU_BUILD_WORK (Build-Qemu.ps1 does this).'

install_deps() {
    local packages=()
    read -r -a packages <<< "$MSYS2_PACKAGES"
    pacman -S --needed --noconfirm "${packages[@]}"
    pacman -Q "${packages[@]}" > "$OUT/pacman-requested.txt"
    pacman -Q > "$OUT/pacman-lock.txt"
}

clone_source() {
    local actual patch name
    mkdir -p -- "$BUILD_ROOT/src"
    if [[ ! -d "$SOURCE/.git" ]]; then
        [[ ! -e "$SOURCE" ]] || fail 'Incomplete src/qemu exists; inspect before cleaning source.'
        git clone --depth 1 --branch "$QEMU_TAG" -- "$QEMU_GIT_URL" "$SOURCE"
    fi
    [[ "$(git -C "$SOURCE" remote get-url origin)" == "$QEMU_GIT_URL" ]] || fail 'Unexpected QEMU origin.'
    actual=$(git -C "$SOURCE" rev-parse HEAD)
    [[ "$actual" == "$(git -C "$SOURCE" rev-parse "$QEMU_TAG^{commit}")" ]] || fail 'HEAD does not match tag.'
    [[ -z "$QEMU_COMMIT" || "$actual" == "$QEMU_COMMIT" ]] || fail 'Pinned commit mismatch; review upstream.'
    printf '%s\n' "$actual" > "$OUT/qemu-commit.txt"
    git -C "$SOURCE" diff --quiet || fail 'Unstaged QEMU edits; refusing to patch.'
    shopt -s nullglob
    for patch in "$BUILD_ROOT"/patches/*.patch; do
        if git -C "$SOURCE" apply --reverse --check --index "$patch" 2>/dev/null; then
            printf 'Already applied: %s\n' "$patch"
        else
            git -C "$SOURCE" apply --check --index "$patch"
            git -C "$SOURCE" apply --index "$patch"
        fi
    done
    # Capture firmware source, not only prebuilt blobs.
    # https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/.gitmodules
    git -C "$SOURCE" submodule update --init --recursive
    git -C "$SOURCE" submodule status --recursive > "$OUT/submodules-lock.txt"
    # Pinned wraps used by this non-Rust build.
    # https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/scripts/archive-source.sh
    (cd -- "$SOURCE"; meson subprojects download "${WRAPS[@]}")
    verify_wraps
    for name in "${WRAPS[@]}"; do
        printf '%s %s\n' "$name" "$(git -C "$SOURCE/subprojects/$name" rev-parse HEAD)"
    done > "$OUT/subprojects-lock.txt"
}

configure_qemu() {
    require_source
    verify_wraps
    [[ -f "$OUT/pacman-lock.txt" ]] || fail 'Run deps first.'
    cmp -s <(pacman -Q) "$OUT/pacman-lock.txt" || fail 'Package state changed; run deps then clean/reconfigure.'
    mkdir -p -- "$OUT/build"
    local fingerprint
    fingerprint=$( { input_hashes; source_tree; cat "$OUT/pacman-lock.txt"; printf '%s\n' "$WORK"; } | sha256sum)
    if [[ -f "$OUT/build/build.ninja" && -f "$OUT/configure.fingerprint" ]]; then
        [[ "$fingerprint" == "$(cat "$OUT/configure.fingerprint")" ]] || fail 'Inputs changed; use -Clean (also -IncludeSource for changed patches).'
        printf 'Configuration is current.\n'
        return
    fi
    # PKGBUILD copies: --bindir, --datadir, --disable-download. No explicit enable-gtk there.
    # https://raw.githubusercontent.com/msys2/MINGW-packages/refs/heads/master/mingw-w64-qemu/PKGBUILD
    # Feature flags map to Meson options:
    # https://www.qemu.org/docs/master/devel/build-system.html
    # https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/meson_options.txt
    # SDL uses sdl2. dsound auto-detects on Windows; require it explicitly.
    # --audio-drv-list is absent in this version.
    # https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/meson.build
    # https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/configure
    local options=(
        "--target-list=$QEMU_TARGETS" "--prefix=$OUT/install"
        --bindir=bin --datadir=share/qemu --python=/ucrt64/bin/python
        --enable-whpx --enable-virglrenderer --enable-opengl --enable-sdl
        --enable-slirp --enable-dsound --enable-tools --enable-install-blobs
        --disable-gtk --disable-sdl-image --disable-modules --disable-rust
        --disable-docs --disable-werror --disable-download
    )
    printf '%s\n' "${options[@]}" > "$OUT/configure-options.txt"
    (cd -- "$OUT/build"; "$SOURCE/configure" "${options[@]}")
    printf '%s\n' "$fingerprint" > "$OUT/configure.fingerprint"
}

build_qemu() {
    configure_qemu
    ninja -C "$OUT/build"
    ninja -C "$OUT/build" install
    input_hashes > "$OUT/build-inputs.sha256"
    source_tree > "$OUT/build-tree.txt"
    pacman -Q > "$OUT/build-packages.txt"
    wrap_hashes > "$OUT/build-wraps.sha256"
    date -u +%FT%TZ > "$OUT/build-date.txt"
}

copy_runtime() {
    verify_build
    local stage exe dependency line report index=0
    local queue=() dependencies=()
    declare -A seen=()
    stage=$(mktemp -d "$OUT/dist-stage.XXXXXX")
    mkdir -p -- "$stage/bin" "$stage/share/qemu" "$stage/licenses"
    : > "$stage/dll-packages.txt"
    : > "$stage/dll-origins.tsv"
    for exe in qemu-system-x86_64.exe qemu-img.exe; do
        [[ -f "$OUT/install/bin/$exe" ]] || fail "Missing executable: $exe"
        cp -- "$OUT/install/bin/$exe" "$stage/bin/$exe"
        queue+=("$OUT/install/bin/$exe")
    done
    while (( index < ${#queue[@]} )); do
        exe=${queue[$index]}; index=$((index + 1))
        report=$(ldd "$exe") || fail "ldd failed: $exe"
        [[ "$report" != *'not found'* ]] || fail "Unresolved DLL for $exe: $report"
        mapfile -t dependencies < <(printf '%s\n' "$report" | sed -n 's|.*=> \(/ucrt64/bin/.*\) (0x[[:xdigit:]]*)$|\1|p')
        while IFS= read -r line; do
            [[ "$line" != *'=>'* ]] && continue
            dependency=${line#*=> }; dependency=${dependency% (*}
            case "$dependency" in
                /ucrt64/bin/*) ;;
                /c/[Ww][Ii][Nn][Dd][Oo][Ww][Ss]/*|/C/[Ww][Ii][Nn][Dd][Oo][Ww][Ss]/*) ;;
                *) fail "Dependency outside UCRT64/Windows: $dependency" ;;
            esac
        done <<< "$report"
        for dependency in "${dependencies[@]}"; do
            [[ -z "${seen[$dependency]+present}" ]] || continue
            seen[$dependency]=1
            cp -- "$dependency" "$stage/bin/"
            pacman -Qqo "$dependency" >> "$stage/dll-packages.txt"
            printf '%s\t%s\n' "$(basename -- "$dependency")" "$dependency" >> "$stage/dll-origins.tsv"
            queue+=("$dependency")
        done
    done
    sort -u -o "$stage/dll-packages.txt" "$stage/dll-packages.txt"
    [[ -s "$stage/dll-packages.txt" ]] || fail 'No UCRT64 DLL dependencies resolved.'
    # https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/pc-bios/meson.build
    for exe in edk2-x86_64-code.fd edk2-i386-vars.fd; do
        cp -- "$OUT/install/share/qemu/$exe" "$stage/share/qemu/"
    done
    cp -R -- "$OUT/install/share/qemu/keymaps" "$stage/share/qemu/"
    # Preserve installed data including VGA/NIC ROMs, not just the UEFI pair.
    cp -R -- "$OUT/install/share/qemu/." "$stage/share/qemu/"
    cp -- "$SOURCE/COPYING" "$stage/licenses/QEMU-COPYING"
    cp -- "$OUT/pacman-lock.txt" "$OUT/qemu-commit.txt" "$OUT/build-date.txt" "$OUT/configure-options.txt" "$stage/"
    "$stage/bin/qemu-system-x86_64.exe" --version
    "$stage/bin/qemu-img.exe" --version
    "$stage/bin/qemu-system-x86_64.exe" -accel help | tee "$OUT/accel-help.txt"
    grep -qw whpx "$OUT/accel-help.txt" || fail 'WHPX was not built.'
    "$stage/bin/qemu-system-x86_64.exe" -display help | tee "$OUT/display-help.txt"
    grep -qw sdl "$OUT/display-help.txt" || fail 'SDL was not built.'
    "$stage/bin/qemu-system-x86_64.exe" -device help | tee "$OUT/device-help.txt"
    grep -q 'virtio-vga-gl' "$OUT/device-help.txt" || fail 'virtio-vga-gl was not built.'
    # Only generated trees; :? prevents empty prefixes naming system directories.
    rm -rf -- "${DIST:?}/qemu" "${OUT:?}/bin" "${OUT:?}/share"
    mv -- "$stage" "$DIST/qemu"
    cp -R -- "$DIST/qemu/bin" "$OUT/bin"
    cp -R -- "$DIST/qemu/share" "$OUT/share"
    (cd -- "$DIST/qemu"; find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum) > "$OUT/dist.sha256"
    cp -- "$OUT/dist.sha256" "$DIST/qemu/SHA256SUMS"
    printf '\nLauncher executable: %s\nDistribution: %s\n' "$OUT/bin/qemu-system-x86_64.exe" "$DIST/qemu/bin/qemu-system-x86_64.exe"
    "$OUT/bin/qemu-system-x86_64.exe" --version
}

case "$STEP" in
    all) install_deps; clone_source; build_qemu; copy_runtime
         bash "$BUILD_ROOT/make-third-party.sh"
         bash "$BUILD_ROOT/make-source-offer.sh" ;;
    deps) install_deps ;;
    clone) clone_source ;;
    configure) configure_qemu ;;
    build) build_qemu ;;
    dist) copy_runtime ;;
esac
