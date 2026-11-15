#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# The only step of .github/workflows/guest-build-selfhosted.yml. It runs in the runner container,
# asks the bridge in Termux for a stage through the spool and shows the stage's log until the
# stage ends. When the job ends first (5-day limit, cancellation), the stage keeps running on the
# phone and dispatching the same stage again follows it.
set -uo pipefail
: "${OME_CI_HOME:?the runner .env sets OME_CI_HOME}" "${STAGE:?}" "${GITHUB_RUN_ID:?}"
SPOOL=$OME_CI_HOME/spool
ref=${REF:-main}
id=$GITHUB_RUN_ID-${GITHUB_RUN_ATTEMPT:-1}
req=$SPOOL/requests/$id.req
ack=$SPOOL/requests/$id.ack
out=$SPOOL/requests/$id.out

mkdir -p "$SPOOL/requests"
printf 'stage=%s\nref=%s\nrun=%s/%s/actions/runs/%s\n' "$STAGE" "$ref" \
    "${GITHUB_SERVER_URL:-https://github.com}" "${GITHUB_REPOSITORY:-}" "$GITHUB_RUN_ID" > "$req.tmp"
mv "$req.tmp" "$req"
for _ in $(seq 1 60); do
    [ -f "$ack" ] && break
    sleep 2
done
if [ ! -f "$ack" ]; then
    rm -f "$req"
    echo "::error::the bridge on the phone did not answer within 120 s; run 'ome-ci start' in Termux"
    exit 1
fi

field() { sed -n "s/^$1=//p" "$ack" | head -n 1; }
kind=$(field kind)
case "$kind" in
    error)
        echo "::error::$(field message)"
        rm -f "$ack"
        exit 1
        ;;
    inline)
        cat "$out"
        code=$(field code)
        rm -f "$ack" "$out"
        exit "${code:-1}"
        ;;
    task) ;;
    *)
        echo "::error::unexpected answer from the bridge: $kind"
        exit 1
        ;;
esac

task=$(field task)
attached=$(field attached)
rm -f "$ack"
d=$SPOOL/tasks/$task
off=0
if [ "$attached" = 1 ]; then
    echo "::notice::stage $STAGE is already running as $task; following it from the last 64 KiB of its log"
    size=$(stat -c %s "$d/log" 2>/dev/null || echo 0)
    [ "$size" -gt 65536 ] && off=$((size - 65536))
else
    echo "started $task"
fi
trap 'echo; echo "::notice::this job ended before the stage; $task keeps running on the phone. Dispatch stage $STAGE again to follow it, status to look at it, stop to end it."; exit 143' TERM INT HUP

while :; do
    ended=0
    [ -f "$d/exit" ] && ended=1
    size=$(stat -c %s "$d/log" 2>/dev/null || echo 0)
    if [ "$size" -gt "$off" ]; then
        tail -c "+$((off + 1))" "$d/log" | head -c "$((size - off))"
        off=$size
    fi
    if [ "$ended" = 1 ]; then
        rc=$(cat "$d/exit")
        echo
        echo "stage $STAGE ($task) ended with exit $rc"
        exit "$rc"
    fi
    sleep 5
done
