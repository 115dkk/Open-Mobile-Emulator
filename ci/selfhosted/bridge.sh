#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# The phone CI's bridge (ci/selfhosted/README.md). It runs in Termux, outside proot, because a
# proot cannot start inside another proot on this phone. The runner container hands it stage
# requests as files under $OME_CI_HOME/spool, and it runs each stage in the x86_64 build
# container. A stage outlives the GitHub job that asked for it (5-day job limit, cancellation,
# lost connection); asking for the same stage again follows it. One stage runs at a time.
#
#   bridge.sh [serve]   answer requests until killed (ome-ci start keeps it running)
#   bridge.sh status    print the current or last stage and the end of its log
#   bridge.sh stop      stop the running stage
set -uo pipefail
# shellcheck source=lib.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

LONG_STAGES="host-setup init sync aaropa build collect all"
mkdir -p "$SPOOL/requests" "$SPOOL/tasks"

current() { cat "$SPOOL/current" 2>/dev/null || true; }

alive() {
    local pid
    pid=$(cat "$SPOOL/tasks/$1/pid" 2>/dev/null) || return 1
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null || return 1
    # After a reboot the old pid can belong to another of our processes; only this stage's own
    # run-task.sh counts, so a dead stage is reaped and stop never signals a stranger.
    tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | grep -q -F "run-task.sh $SPOOL/tasks/$1"
}

finish() {  # task, exit code, note for the log
    local d=$SPOOL/tasks/$1
    [ -f "$d/exit" ] && return 0
    echo "ome-ci: $3" >> "$d/log"
    date '+%F %T' > "$d/finished"
    echo "$2" > "$d/exit"
}

# A stage whose process is gone without an exit code was lost: Termux or the phone stopped.
reap() {
    local t
    t=$(current)
    [ -n "$t" ] || return 0
    if [ ! -f "$SPOOL/tasks/$t/exit" ] && ! alive "$t"; then
        finish "$t" 255 "the stage's process is gone (Termux or the phone stopped); ask for the stage again to resume"
    fi
    [ -f "$SPOOL/tasks/$t/exit" ] && rm -f "$SPOOL/current"
    return 0
}

status_text() {
    reap
    local t d
    t=$(current)
    if [ -n "$t" ]; then
        d=$SPOOL/tasks/$t
        echo "running: $t, scripts $(cat "$d/ref") $(cat "$d/sha" 2>/dev/null), since $(cat "$d/started")"
    else
        t=$(find "$SPOOL/tasks" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort | tail -n 1)
        if [ -z "$t" ]; then
            echo "idle; no stage has run yet"
        else
            d=$SPOOL/tasks/$t
            echo "idle; last: $t, exit $(cat "$d/exit" 2>/dev/null || echo '?'), $(cat "$d/started") to $(cat "$d/finished" 2>/dev/null || echo '?')"
        fi
    fi
    if [ -n "$t" ]; then
        echo "--- end of $SPOOL/tasks/$t/log"
        tail -n 60 "$SPOOL/tasks/$t/log" 2>/dev/null
    fi
    echo "--- phone"
    df -h "$OME_CI_HOME" | awk 'NR == 2 {print "storage: " $4 " free of " $2}'
    awk '/^MemAvailable:/ {a = $2} /^SwapFree:/ {s = $2} END {printf "memory: %.1f GB available, swap %.1f GB free\n", a / 1048576, s / 1048576}' /proc/meminfo
}

stop_stage() {
    reap
    local t pid
    t=$(current)
    if [ -z "$t" ]; then
        echo "nothing is running"
        return 0
    fi
    pid=$(cat "$SPOOL/tasks/$t/pid")
    echo "stopping $t (process group $pid)"
    kill -TERM -- "-$pid" 2>/dev/null
    for _ in $(seq 1 30); do
        alive "$t" || break
        sleep 1
    done
    alive "$t" && kill -KILL -- "-$pid" 2>/dev/null
    # Whatever the build container still runs belongs to this stage.
    proot-distro kill "$OME_CI_BUILD_CONTAINER" >/dev/null 2>&1 || true
    finish "$t" 143 "stopped on request"
    rm -f "$SPOOL/current"
    echo "stopped; the tree keeps what was built, and asking for the stage again resumes it"
}

start_stage() {  # stage, ref, request id; prints the task id
    local t d
    t=$(date +%Y%m%d-%H%M%S)-$1
    d=$SPOOL/tasks/$t
    mkdir -p "$d"
    printf '%s\n' "$1" > "$d/stage"
    printf '%s\n' "$2" > "$d/ref"
    printf '%s\n' "$3" > "$d/request"
    date '+%F %T' > "$d/started"
    : > "$d/log"
    # Its own session, so stopping the bridge, the runner or the job leaves the stage running.
    setsid bash "$OME_CI_BIN/run-task.sh" "$d" </dev/null >/dev/null 2>&1 &
    echo $! > "$d/pid"
    echo "$t" > "$SPOOL/current"
    echo "$t"
}

answer() {  # request id; the answer's key=value lines come on stdin
    cat > "$SPOOL/requests/$1.ack.tmp" && mv "$SPOOL/requests/$1.ack.tmp" "$SPOOL/requests/$1.ack"
}

field() { sed -n "s/^$1=//p" "$2" | head -n 1; }

handle() {
    local req=$1 id stage ref t
    id=$(basename "$req" .req)
    stage=$(field stage "$req")
    ref=$(field ref "$req")
    rm -f "$req"
    case "$stage" in
        status|stop)
            if [ "$stage" = status ]; then status_text; else stop_stage; fi > "$SPOOL/requests/$id.out" 2>&1
            printf 'kind=inline\ncode=0\n' | answer "$id"
            return
            ;;
    esac
    if [[ " $LONG_STAGES " != *" $stage "* ]]; then
        printf 'kind=error\nmessage=unknown stage: %s\n' "$stage" | answer "$id"
        return
    fi
    if ! valid_ref "$ref"; then
        printf 'kind=error\nmessage=not a branch, tag or commit name: %s\n' "$ref" | answer "$id"
        return
    fi
    reap
    t=$(current)
    if [ -n "$t" ]; then
        if [ "$(cat "$SPOOL/tasks/$t/stage")" = "$stage" ] && [ "$(cat "$SPOOL/tasks/$t/ref")" = "$ref" ]; then
            printf 'kind=task\ntask=%s\nattached=1\n' "$t" | answer "$id"
        else
            printf 'kind=error\nmessage=busy: %s is running; wait for it or run the stop stage\n' "$t" | answer "$id"
        fi
        return
    fi
    t=$(start_stage "$stage" "$ref" "$id")
    printf 'kind=task\ntask=%s\n' "$t" | answer "$id"
}

serve() {
    say "bridge answering requests in $SPOOL"
    while true; do
        reap
        for req in "$SPOOL"/requests/*.req; do
            [ -e "$req" ] || continue
            handle "$req"
        done
        sleep 3
    done
}

case "${1:-serve}" in
    serve) serve ;;
    status) status_text ;;
    stop) stop_stage ;;
    *) sed -n '11,13p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
