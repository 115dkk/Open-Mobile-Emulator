#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Setup of the phone CI in Termux, run from a checkout of the branch selfhosted/guest-build
# (ci/selfhosted/README.md). Both steps can run again; they skip what is already in place.
#
#   bash ci/selfhosted/setup-phone.sh install    Termux packages, scripts, both containers, the
#                                                build host (hours under emulation) and the runners
#   bash ci/selfhosted/setup-phone.sh register   register both runners with the repository
#                                                (gh signed in as the repository's admin)
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
OME_CI_HOME=${OME_CI_HOME:-$HOME/ome-ci}
OME_CI_BIN=$OME_CI_HOME/bin
FILES="pins.env lib.sh bridge.sh run-task.sh request.sh job-started.sh build-host-prepare.sh ome-ci"
ROLES="main ctl"

install_scripts() {
    mkdir -p "$OME_CI_BIN" "$OME_CI_HOME/spool/requests" "$OME_CI_HOME/spool/tasks" \
        "$OME_CI_HOME/logs" "$OME_CI_HOME/cache" "$HOME/.local/bin"
    local f
    for f in $FILES; do
        install -m 755 "$HERE/$f" "$OME_CI_BIN/$f"
    done
    ln -sf "$OME_CI_BIN/ome-ci" "$HOME/.local/bin/ome-ci"
}

in_runner() { proot-distro login "$OME_CI_RUNNER_CONTAINER" -- bash -c "$1"; }

install_runner() {
    local role=$1 dir=/opt/actions-runner/$1 tarball
    # Any version will do: the runner updates itself when GitHub asks it to.
    if in_runner "[ -x $dir/bin/Runner.Listener ]"; then
        say "runner $role already installed ($(in_runner "$dir/bin/Runner.Listener --version"))"
        return 0
    fi
    tarball=$OME_CI_HOME/cache/actions-runner-linux-arm64-$OME_CI_RUNNER_VERSION.tar.gz
    if ! echo "$OME_CI_RUNNER_SHA256  $tarball" | sha256sum -c --status 2>/dev/null; then
        retry 5 curl -fL -C - --retry 5 --retry-all-errors --retry-delay 10 -o "$tarball.part" \
            "https://github.com/actions/runner/releases/download/v$OME_CI_RUNNER_VERSION/actions-runner-linux-arm64-$OME_CI_RUNNER_VERSION.tar.gz"
        mv "$tarball.part" "$tarball"
        echo "$OME_CI_RUNNER_SHA256  $tarball" | sha256sum -c -
    fi
    in_runner "mkdir -p $dir && tar -xzf '$tarball' -C $dir"
    say "runner $role installed ($(in_runner "$dir/bin/Runner.Listener --version"))"
}

cmd_install() {
    pkg install -y proot-distro qemu-user-x86-64 tmux gh jq git util-linux
    install_scripts
    # shellcheck source=lib.sh
    source "$OME_CI_BIN/lib.sh"

    local containers=$PREFIX/var/lib/proot-distro/containers
    [ -d "$containers/$OME_CI_BUILD_CONTAINER" ] ||
        retry 5 proot-distro install -a x86_64 -n "$OME_CI_BUILD_CONTAINER" "$OME_CI_BUILD_IMAGE"
    [ -d "$containers/$OME_CI_RUNNER_CONTAINER" ] ||
        retry 5 proot-distro install -a aarch64 -n "$OME_CI_RUNNER_CONTAINER" "$OME_CI_RUNNER_IMAGE"

    say "build scripts of main for the build host"
    ensure_src main

    say "build host in $OME_CI_BUILD_CONTAINER (x86_64 under qemu-user; the first run takes hours)"
    build_login -- bash "$OME_CI_BIN/build-host-prepare.sh" "$SRC/guest/build"

    say "runner container $OME_CI_RUNNER_CONTAINER"
    in_runner 'set -e
        export DEBIAN_FRONTEND=noninteractive
        printf "Acquire::Retries \"10\";\n" > /etc/apt/apt.conf.d/80-ome-retries
        apt-get update -q
        apt-get install -y -q --no-install-recommends ca-certificates curl jq libicu74 libssl3t64 zlib1g'
    local role
    for role in $ROLES; do
        install_runner "$role"
    done
    say "installed; next: bash $HERE/setup-phone.sh register, then ome-ci start"
}

cmd_register() {
    install_scripts
    # shellcheck source=lib.sh
    source "$OME_CI_BIN/lib.sh"
    local role label dir name token envfile model
    model=$(getprop ro.product.model 2>/dev/null | tr '[:upper:]' '[:lower:]')
    for role in $ROLES; do
        dir=/opt/actions-runner/$role
        if [ "$role" = main ]; then label=$OME_CI_LABEL_MAIN; else label=$OME_CI_LABEL_CTL; fi
        name=${model:-phone}-$role
        if in_runner "[ -f $dir/.runner ]"; then
            say "runner $role is already registered"
        else
            token=$(retry 5 gh api -X POST "repos/$OME_CI_REPO/actions/runners/registration-token" --jq .token)
            proot-distro login "$OME_CI_RUNNER_CONTAINER" --env RUNNER_ALLOW_RUNASROOT=1 --env OME_CI_TOKEN="$token" -- \
                bash -c "cd $dir && ./config.sh --unattended --url https://github.com/$OME_CI_REPO --token \"\$OME_CI_TOKEN\" \
                    --name $name --labels $label --no-default-labels --work _work --replace"
        fi
        # The runner reads .env at start: where the bridge's spool is, which runner this is, and
        # the gate that refuses every job but the owner's manual run of the phone workflow.
        envfile=$(container_rootfs "$OME_CI_RUNNER_CONTAINER")$dir/.env
        touch "$envfile"
        sed -i -E '/^(OME_CI_HOME|OME_CI_RUNNER_ROLE|ACTIONS_RUNNER_HOOK_JOB_STARTED|RUNNER_ALLOW_RUNASROOT)=/d' "$envfile"
        {
            echo "OME_CI_HOME=$OME_CI_HOME"
            echo "OME_CI_RUNNER_ROLE=$role"
            echo "ACTIONS_RUNNER_HOOK_JOB_STARTED=$OME_CI_BIN/job-started.sh"
            echo "RUNNER_ALLOW_RUNASROOT=1"
        } >> "$envfile"
        say "runner $role: label $label, gate $OME_CI_BIN/job-started.sh"
    done
    say "registered; ome-ci start brings the runners online (ome-ci stop first if they were running)"
}

case "${1:-}" in
    install) cmd_install ;;
    register) cmd_register ;;
    *) sed -n '8,11p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
