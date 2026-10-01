#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Prepares an Ubuntu 22.04 host (WSL distro, container or VM) for building the self-built guest
# image. Run as root. Idempotent: packages are installed once, the build user is created once.
#
# Package list: BlissOS voyager-x86 manifest README (read 2026-10-01), plus what repo, the
# Android build system and our own scripts need (python-is-python3, rsync, p7zip, aria2).
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=pins.env
source "$HERE/pins.env"

BUILD_USER=${OME_BUILD_USER:-ome}
export DEBIAN_FRONTEND=noninteractive

if [ "$(id -u)" -ne 0 ]; then
    echo "run as root" >&2
    exit 1
fi

echo "==> packages"
dpkg --add-architecture i386
apt-get update
apt-get install -y --no-install-recommends \
    ca-certificates curl wget sudo locales tzdata \
    git-core gnupg flex bison gperf build-essential zip zlib1g-dev gcc-multilib g++-multilib \
    libc6-dev-i386 lib32ncurses5-dev x11proto-core-dev libx11-dev lib32z-dev ccache libgl1-mesa-dev \
    libxml2-utils xsltproc unzip squashfs-tools python3-mako libssl-dev ninja-build lunzip syslinux \
    syslinux-utils gettext genisoimage bc xorriso xmlstarlet meson glslang-tools git-lfs libncurses5 \
    libncurses5:i386 libelf-dev aapt zstd rdfind nasm kmod \
    python3 python-is-python3 python3-pip rsync p7zip-full aria2 file cpio erofs-utils e2fsprogs \
    dosfstools mtools openssh-client less procps
apt-get clean
rm -rf /var/lib/apt/lists/*

echo "==> locale"
sed -i 's/^# *en_US.UTF-8 UTF-8/en_US.UTF-8 UTF-8/' /etc/locale.gen
locale-gen >/dev/null
update-locale LANG=en_US.UTF-8

echo "==> repo launcher"
install -d /usr/local/bin
curl -fsSL "$REPO_LAUNCHER_URL" -o /usr/local/bin/repo
chmod 755 /usr/local/bin/repo
echo "repo launcher sha256: $(sha256sum /usr/local/bin/repo | cut -d' ' -f1)"

echo "==> build user $BUILD_USER"
if ! id "$BUILD_USER" >/dev/null 2>&1; then
    useradd -m -s /bin/bash "$BUILD_USER"
fi
echo "$BUILD_USER ALL=(ALL) NOPASSWD:ALL" > "/etc/sudoers.d/90-$BUILD_USER"
chmod 440 "/etc/sudoers.d/90-$BUILD_USER"
git lfs install --system >/dev/null

echo "==> rust toolchain for $BUILD_USER (the README asks for rustup, not distro Rust)"
sudo -u "$BUILD_USER" -H bash -c 'if [ ! -x "$HOME/.cargo/bin/rustc" ]; then curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null; fi; "$HOME/.cargo/bin/rustc" --version'

if grep -qi microsoft /proc/version 2>/dev/null; then
    echo "==> wsl.conf (systemd on, default user $BUILD_USER)"
    cat > /etc/wsl.conf <<EOF
[boot]
systemd=true

[user]
default=$BUILD_USER

[automount]
enabled=true
options="metadata,umask=22,fmask=11"

[interop]
enabled=true
appendWindowsPath=false
EOF
fi

echo "==> done: $(lsb_release -ds 2>/dev/null || cat /etc/os-release | head -1), $(nproc) cpus, $(free -g | awk '/Mem:/ {print $2}') GB RAM"
