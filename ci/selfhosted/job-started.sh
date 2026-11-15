#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# ACTIONS_RUNNER_HOOK_JOB_STARTED of both phone runners (setup-phone.sh writes it into each
# runner's .env). The runner runs it before the job's first step, and a non-zero exit fails the
# job without running anything. A workflow file cannot change this check, so a fork pull request
# that names these runner labels in its own workflow gets no further than here.
set -u
# shellcheck source=pins.env
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/pins.env"

refuse() {
    echo "ome-ci gate: refusing this job: $*" >&2
    exit 1
}

[ "${GITHUB_REPOSITORY:-}" = "$OME_CI_REPO" ] || refuse "repository is '${GITHUB_REPOSITORY:-}'"
[ "${GITHUB_EVENT_NAME:-}" = workflow_dispatch ] || refuse "event is '${GITHUB_EVENT_NAME:-}'; only manual runs come here"
[ "${GITHUB_REF:-}" = "refs/heads/$OME_CI_BRANCH" ] || refuse "ref is '${GITHUB_REF:-}'"
[ "${GITHUB_WORKFLOW_REF:-}" = "$OME_CI_REPO/$OME_CI_WORKFLOW@refs/heads/$OME_CI_BRANCH" ] ||
    refuse "workflow is '${GITHUB_WORKFLOW_REF:-}'"
[ "${GITHUB_ACTOR:-}" = "$OME_CI_OWNER" ] || refuse "actor is '${GITHUB_ACTOR:-}'"
[ "${GITHUB_TRIGGERING_ACTOR:-}" = "$OME_CI_OWNER" ] || refuse "triggering actor is '${GITHUB_TRIGGERING_ACTOR:-}'"
event=${GITHUB_EVENT_PATH:-}
[ -r "$event" ] || refuse "no event payload"
[ "$(jq -r '.sender.login // ""' "$event")" = "$OME_CI_OWNER" ] || refuse "the event's sender is not $OME_CI_OWNER"

stage=$(jq -r '.inputs.stage // ""' "$event")
case "${OME_CI_RUNNER_ROLE:-}" in
    main)
        case "$stage" in
            host-setup|init|sync|aaropa|build|collect|all) ;;
            *) refuse "stage '$stage' does not run on the main runner" ;;
        esac
        ;;
    ctl)
        case "$stage" in
            status|stop) ;;
            *) refuse "stage '$stage' does not run on the control runner" ;;
        esac
        ;;
    *) refuse "OME_CI_RUNNER_ROLE is not set in this runner's .env" ;;
esac
echo "ome-ci gate: accepted run $GITHUB_RUN_ID of $GITHUB_WORKFLOW_REF, stage $stage, by $GITHUB_ACTOR"
