#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#
# Prepares an Ubuntu 22.04 host (WSL distro, container or VM) for building the self-built guest
# image. Run as root. Idempotent: packages are installed once, the build user is created once.
#
# Package list: BlissOS voyager-x86 manifest README (read 2026-10-01), plus what repo, the
# Android build system and our own scripts need (python-is-python3, rsync, p7zip, aria2), plus
# pkg-config, which the README omits and glodroid/aospext's meson cross file names as
# /usr/bin/pkg-config, and python3-ply, which Mesa's Intel GRL kernels need (Mesa's configure
# step failed without each of them, 2026-10-02).
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
    syslinux-utils gettext genisoimage bc xorriso xmlstarlet glslang-tools git-lfs libncurses5 \
    libncurses5:i386 libelf-dev aapt zstd rdfind nasm kmod \
    python3 python-is-python3 python3-pip rsync p7zip-full aria2 file cpio erofs-utils e2fsprogs \
    dosfstools mtools openssh-client less procps pkg-config python3-ply
apt-get clean
rm -rf /var/lib/apt/lists/*

# The Bliss README lists the distro meson, but Ubuntu 22.04 ships 0.61.2 and external/mesa in
# the tree (24.3.3) requires >= 1.1.0; glodroid/aospext runs whatever meson is on PATH, so the
# first build stopped at 48 % in Mesa's configure step (2026-10-02). PyPI's release, pinned in
# pins.env, lands in /usr/local/bin ahead of /usr/bin.
echo "==> meson $MESON_PIP_VERSION from PyPI"
apt-get remove -y meson >/dev/null 2>&1 || true
pip3 install --no-cache-dir "meson==$MESON_PIP_VERSION" >/dev/null
echo "meson $(meson --version) at $(command -v meson)"

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
git lfs install --system --skip-repo >/dev/null

echo "==> rust toolchain for $BUILD_USER (the README asks for rustup, not distro Rust)"
sudo -u "$BUILD_USER" -H bash -c 'if [ ! -x "$HOME/.cargo/bin/rustc" ]; then curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null; fi; "$HOME/.cargo/bin/rustc" --version'

# The README's follow-up to rustup: the Android targets and three cargo programs. Mesa's
# configure step needs the x86_64-linux-android and i686-linux-android targets (NVK is in the
# vulkan driver list, so Mesa adds Rust) and bindgen/cbindgen; the first build host lacked all of
# them (2026-10-02). Versions are pinned in pins.env; bindgen-cli 0.69.1 is the README's own pin.
echo "==> rust targets and cargo programs for $BUILD_USER"
sudo -u "$BUILD_USER" -H env CARGO_NDK_VERSION="$CARGO_NDK_VERSION" BINDGEN_CLI_VERSION="$BINDGEN_CLI_VERSION" CBINDGEN_VERSION="$CBINDGEN_VERSION" bash -c '
    export PATH="$HOME/.cargo/bin:$PATH"
    rustup target add x86_64-linux-android i686-linux-android >/dev/null
    for spec in "cargo-ndk $CARGO_NDK_VERSION" "bindgen-cli $BINDGEN_CLI_VERSION" "cbindgen $CBINDGEN_VERSION"; do
        set -- $spec
        cargo install --locked --version "$2" "$1" >/dev/null 2>&1 || cargo install --version "$2" "$1" >/dev/null
    done
    rustup target list --installed | paste -sd " " -
    cargo ndk --version; bindgen --version; cbindgen --version'

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
