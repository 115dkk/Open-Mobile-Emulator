#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Apply an identity profile to an existing rooted OME Android-x86 guest.

Requires a loopback adb target and an ext4 system-as-root guest. Backups remain
on the host. Stop/start the VM after success; guest reboot is unreliable on WHPX.
"""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path
from device_profile import parse_profile

FILES = ('/system/build.prop', '/vendor/build.prop', '/product/etc/build.prop',
         '/system_ext/etc/build.prop', '/odm/etc/build.prop')


def expanded_identity(values):
    # The all-partition trial failed to boot; the model/fingerprint-only trial
    # passed. Preserve native partition identities and hardware codenames.
    return {key: values[key] for key in ('ro.product.model', 'ro.build.fingerprint')}


def merge_properties(text, values, system=False):
    identity = expanded_identity(values)
    remaining = dict(identity)
    lines = []
    for line in text.splitlines():
        key = line.partition('=')[0]
        if key in identity:
            # Collapse duplicate entries to avoid last-wins surprises.
            if key in remaining:
                lines.append(f'{key}={remaining.pop(key)}')
        else:
            lines.append(line)
    if system:
        lines.extend(f'{key}={value}' for key, value in remaining.items())
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--adb', required=True)
    parser.add_argument('--serial', default='127.0.0.1:5555')
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--backup-dir', type=Path, required=True)
    args = parser.parse_args()
    serial = re.fullmatch(r'127\.0\.0\.1:([0-9]+)', args.serial)
    if serial is None or not 1 <= int(serial[1]) <= 65535:
        parser.error('an explicit loopback OME adb target is required')
    values = parse_profile(args.profile.read_text(encoding='utf-8'))

    def adb(*command):
        return subprocess.run([args.adb, '-s', args.serial, *command], check=True,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30).stdout

    if adb('shell', 'id -u').strip() != b'0':
        raise RuntimeError('run adb root on the OME guest first')
    if b'x86_64' not in adb('shell', 'getprop ro.product.cpu.abilist'):
        raise RuntimeError('target is not an x86_64 guest')
    mounts = adb('shell', 'cat /proc/mounts').decode().splitlines()
    root_mount = next((line.split() for line in mounts if line.split()[1] == '/'), None)
    if root_mount is None or root_mount[2] != 'ext4' or 'ro' not in root_mount[3].split(','):
        raise RuntimeError('requires a read-only ext4 system-as-root guest')
    args.backup_dir.mkdir(parents=True, exist_ok=False)
    saved = []
    # Finish all reads and backups before the first guest write.
    for remote in FILES:
        if adb('shell', f'if [ -f {remote} ]; then echo yes; fi').strip() != b'yes':
            continue
        data = adb('exec-out', 'cat', remote)
        mode = adb('shell', f'stat -c %a {remote}').decode().strip()
        if not re.fullmatch(r'[0-7]{3,4}', mode):
            raise RuntimeError(f'cannot read file mode: {remote}')
        name = remote.strip('/').replace('/', '_')
        original = args.backup_dir / (name + '.original')
        modified = args.backup_dir / (name + '.profile')
        original.write_bytes(data)
        modified.write_text(merge_properties(data.decode(), values, remote == FILES[0]),
                            encoding='utf-8', newline='\n')
        saved.append((remote, mode, original, modified))
    if not saved or saved[0][0] != FILES[0]:
        raise RuntimeError('missing system build.prop')
    # This manifest and the original files are sufficient for manual recovery,
    # including when the next boot does not reach adb.
    manifest = [{'path': remote, 'mode': mode, 'backup': original.name,
                 'sha256': hashlib.sha256(original.read_bytes()).hexdigest()}
                for remote, mode, original, modified in saved]
    (args.backup_dir / 'backup.json').write_text(json.dumps(manifest, indent=2) + '\n')
    adb('shell', 'mount -o remount,rw /')
    changed = []
    try:
        for remote, mode, original, modified in saved:
            if original.read_bytes() == modified.read_bytes():
                continue
            changed.append((remote, mode, original))
            adb('push', str(modified), remote)
            adb('shell', f'chmod {mode} {remote}')
            if adb('exec-out', 'cat', remote) != modified.read_bytes():
                raise RuntimeError(f'profile readback mismatch: {remote}')
    except Exception:
        for remote, mode, original in reversed(changed):
            adb('push', str(original), remote)
            adb('shell', f'chmod {mode} {remote}')
        raise
    finally:
        adb('shell', 'sync; mount -o remount,ro /')
    print(f'Updated {len(changed)} property files; backups: {args.backup_dir}')
    print('Stop and start the VM, then verify getprop and Play Store separately.')


if __name__ == '__main__':
    main()
