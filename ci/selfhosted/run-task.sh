#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# One stage of the phone CI. bridge.sh starts it in its own session with the task directory as
# the only argument; it writes the stage's log, the commit of the build scripts and, last, the
# exit code there.
set -uo pipefail
# shellcheck source=lib.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

d=$1
stage=$(cat "$d/stage")
ref=$(cat "$d/ref")
exec >>"$d/log" 2>&1

run_stage() {
    say "ome-ci stage $stage, build scripts from $ref"
    ensure_src "$ref" || return 1
    git -C "$SRC" rev-parse HEAD > "$d/sha"
    say "build scripts at $(cat "$d/sha")"
    local gb=$SRC/guest/build
    case "$stage" in
        host-setup)
            build_login -- bash "$OME_CI_BIN/build-host-prepare.sh" "$gb"
            ;;
        *)
            if ! grep -q OME_MEM_BUDGET_GB "$gb/build-image.sh"; then
                echo "the build scripts at $ref predate the low-memory build (OME_MEM_BUDGET_GB); pick a ref that has it" >&2
                return 2
            fi
            build_login --user ome --work-dir /home/ome \
                --env LANG=en_US.UTF-8 \
                --env OME_REPO_ROOT="$SRC" \
                --env OME_BUILD_ROOT="$OME_CI_TREE" \
                --env OME_MEM_BUDGET_GB="$OME_CI_MEM_BUDGET_GB" \
                --env OME_BUILD_JOBS="$OME_CI_BUILD_JOBS" \
                --env OME_SYNC_JOBS="$OME_CI_SYNC_JOBS" \
                -- bash "$gb/build-image.sh" "$stage"
            ;;
    esac
}

run_stage
rc=$?
say "stage $stage ended with exit $rc"
date '+%F %T' > "$d/finished"
echo "$rc" > "$d/exit.tmp" && mv "$d/exit.tmp" "$d/exit"
