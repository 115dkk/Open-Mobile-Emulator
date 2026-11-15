# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Shared by the Termux-side scripts of the phone CI (ci/selfhosted/README.md). Sourced, not run.
# shellcheck shell=bash disable=SC2034  # SPOOL and SRC are for the scripts that source this

OME_CI_BIN=${OME_CI_BIN:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}
# shellcheck source=pins.env
source "$OME_CI_BIN/pins.env"
OME_CI_HOME=${OME_CI_HOME:-$HOME/ome-ci}
SPOOL=$OME_CI_HOME/spool
SRC=$OME_CI_HOME/src

say() { printf '==> %s  (%s)\n' "$*" "$(date '+%F %T')"; }

# retry <attempts> <command...>: the phone's Wi-Fi drops for seconds at a time, and that should
# cost a wait, not the stage.
retry() {
    local attempts=$1 i=1
    shift
    until "$@"; do
        if [ "$i" -ge "$attempts" ]; then
            echo "giving up after $attempts attempts: $*" >&2
            return 1
        fi
        say "attempt $i of $attempts failed, retrying in $((i * 20)) s: $*" >&2
        sleep $((i * 20))
        i=$((i + 1))
    done
}

# Branch, tag or commit names only: no option-looking or path-escaping values reach git.
valid_ref() {
    [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._/-]{0,199}$ ]] && [[ "$1" != *..* ]]
}

container_rootfs() { printf '%s/var/lib/proot-distro/containers/%s/rootfs' "$PREFIX" "$1"; }

# ensure_src <ref>: the build scripts at <ref> in $SRC. Only guest/build and manifests are checked
# out (build-image.sh reads manifests/device-profile.prop); a blobless clone fetches their files
# during checkout, so the checkout retries like the fetch. --force because the snapshot stage
# rewrites guest/build/manifest/ome.xml in this checkout, and the committed copy is the one to keep.
ensure_src() {
    if [ ! -d "$SRC/.git" ]; then
        retry 5 git clone -q --filter=blob:none --no-checkout "https://github.com/$OME_CI_REPO.git" "$SRC" || return 1
        git -C "$SRC" sparse-checkout set --no-cone /guest/build/ /manifests/ || return 1
    fi
    retry 10 git -C "$SRC" fetch -q --no-tags origin "$1" || return 1
    retry 5 git -C "$SRC" checkout -q --force --detach FETCH_HEAD
}

# proot-distro hands Termux's Android variables to the container. pip's platformdirs takes
# ANDROID_DATA and ANDROID_ROOT for a sign that it runs on Android and fails (the first host setup
# stopped there on 2026-11-15), and the ART variables describe the phone, not the build host.
ANDROID_VARS=(ANDROID_ART_ROOT ANDROID_DATA ANDROID_I18N_ROOT ANDROID_ROOT ANDROID_TZDATA_ROOT
    BOOTCLASSPATH DEX2OATBOOTCLASSPATH EXTERNAL_STORAGE)

# build_login [proot-distro login options] -- command...: the build container without them.
build_login() {
    local opts=() strip=() v
    while [ $# -gt 0 ] && [ "$1" != -- ]; do
        opts+=("$1")
        shift
    done
    shift
    for v in "${ANDROID_VARS[@]}"; do
        strip+=(-u "$v")
    done
    proot-distro login "$OME_CI_BUILD_CONTAINER" "${opts[@]}" -- env "${strip[@]}" "$@"
}
