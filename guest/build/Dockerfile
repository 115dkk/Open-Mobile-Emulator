# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Build host for the self-built guest image, as a container. Same recipe as the WSL distro that
# New-BuildDistro.ps1 creates: Ubuntu 22.04 LTS plus setup-build-host.sh. Mount a volume with
# 250 GB free at /build and run guest/build/build-image.sh from a checkout of this repository.
#
#   docker build -t ome-guest-build guest/build
#   docker run --rm -it -v ome-bliss:/build -v "$PWD:/repo:ro" ome-guest-build \
#       env OME_REPO_ROOT=/repo OME_BUILD_ROOT=/build/bliss /repo/guest/build/build-image.sh all
FROM ubuntu:22.04
COPY pins.env setup-build-host.sh /opt/ome-build/
RUN bash /opt/ome-build/setup-build-host.sh
USER ome
WORKDIR /build
ENV LANG=en_US.UTF-8 PATH=/home/ome/.cargo/bin:/usr/local/bin:/usr/bin:/bin
