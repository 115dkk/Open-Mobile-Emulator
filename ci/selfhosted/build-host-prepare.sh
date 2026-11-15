#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Inside the x86_64 build container, as root: what the emulated host needs, then the regular
# build host recipe. The only argument is a guest/build directory (pins.env, setup-build-host.sh).
set -euo pipefail
dir=${1:?usage: build-host-prepare.sh <guest/build directory>}

# glibc's static-pie ldconfig crashes under qemu-user on this phone (brk does not grow, then TLS
# setup faults; docs/evidence/P3/selfhosted-phone-20261115.md). apt runs it from libc-bin's
# trigger, so it becomes a no-op and the ld.so.cache shipped in the image stays; libraries in the
# default multiarch directories are found without the cache.
if [ -z "$(dpkg-divert --list /sbin/ldconfig.real)" ]; then
    dpkg-divert --local --rename --add /sbin/ldconfig.real
    printf '#!/bin/sh\n# ome-ci: static-pie ldconfig crashes under qemu-user here; the shipped cache stays.\nexit 0\n' \
        > /sbin/ldconfig.real
    chmod 755 /sbin/ldconfig.real
fi
dpkg --configure -a

install -d /opt/ome-setup
cp "$dir/pins.env" "$dir/setup-build-host.sh" /opt/ome-setup/
bash /opt/ome-setup/setup-build-host.sh
