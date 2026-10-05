#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
# Development-only local GApps assembly prototype. Never publish its binary output.
"""Assemble three API35 Google packages into a copy of an ext4 system image.

Requires root, mount, debugfs and e2fsck. Supports only the pinned official archive.
No downloads, translator files, identity properties, SELinux policy or Google keys
are added. Source metadata and license location are in source.json. This prototype
is NOT a production installer: it lacks consent UI, rollback and crash recovery. The source and destination images must be outside the repository.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import xml.etree.ElementTree as ET
import zipfile

SHA256 = '1fb5e5fd1c4b1b54bfe5558f6f361d6c10e13786acff630153e0542df356dfe6'
SHA1 = '2f0054868e6aab3c098acd3decba17a82aed4176'
PACKAGES = {
    'product': {'PrebuiltGmsCore': 'com.google.android.gms', 'Phonesky': 'com.android.vending'},
    'system_ext': {'GoogleServicesFramework': 'com.google.android.gsf'},
}


def digest(path, algorithm):
    h = hashlib.new(algorithm)
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def logical_partitions(disk):
    """Read the pinned archive's GPT and checksum-verified LP linear extents."""
    with disk.open('rb') as stream:
        stream.seek(512)
        gpt = stream.read(92)
        if gpt[:8] != b'EFI PART':
            raise ValueError('not GPT')
        table, count, stride = struct.unpack_from('<QII', gpt, 72)
        if count > 128 or stride != 128:
            raise ValueError('unsupported GPT table')
        stream.seek(table * 512)
        entries = stream.read(count * stride)
        supers = []
        for i in range(count):
            row = entries[i * stride:(i + 1) * stride]
            name = row[56:128].decode('utf-16-le').rstrip('\0')
            if name == 'super':
                supers.append(struct.unpack_from('<QQ', row, 32))
        if len(supers) != 1:
            raise ValueError('expected one super partition')
        first, last = supers[0]
        base = first * 512
        stream.seek(base + 12288)
        header = stream.read(128)
        magic, major, minor, size = struct.unpack_from('<IHHI', header)
        if (magic, major, minor, size) != (0x414c5030, 10, 0, 128):
            raise ValueError('unsupported LP header')
        check = bytearray(header)
        check[12:44] = bytes(32)
        if hashlib.sha256(check).digest() != header[12:44]:
            raise ValueError('LP header checksum mismatch')
        table_size = struct.unpack_from('<I', header, 44)[0]
        if table_size > 65536:
            raise ValueError('oversized LP tables')
        tables = stream.read(table_size)
        if hashlib.sha256(tables).digest() != header[48:80]:
            raise ValueError('LP table checksum mismatch')
        po, pn, ps = struct.unpack_from('<III', header, 80)
        eo, en, es = struct.unpack_from('<III', header, 92)
        if ps != 52 or es != 24 or po + pn * ps > table_size or eo + en * es > table_size:
            raise ValueError('invalid LP table bounds')
        result = {}
        for i in range(pn):
            row = tables[po + i * ps:po + (i + 1) * ps]
            name = row[:36].split(b'\0')[0].decode('ascii')
            if name not in PACKAGES:
                continue
            _, index, extent_count, _ = struct.unpack_from('<IIII', row, 36)
            if extent_count != 1 or index >= en:
                raise ValueError('prototype supports one linear extent only')
            sectors, kind, sector, source = struct.unpack_from('<QIQI', tables, eo + index * es)
            if kind != 0 or source != 0 or sector + sectors > last - first + 1:
                raise ValueError('unsupported or out-of-bounds extent')
            result[name] = (base + sector * 512, sectors * 512)
        if set(result) != set(PACKAGES):
            raise ValueError('required partitions missing')
        return result


def quote(value):
    value = str(value)
    if any(c in value for c in ['"', '\n', '\r', '\\']):
        raise ValueError('unsupported debugfs path')
    return '"' + value + '"'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--zip', required=True, type=Path)
    parser.add_argument('--target', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--work', required=True, type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    for path in (args.zip, args.target, args.output, args.work):
        if path.resolve().is_relative_to(repo):
            raise ValueError('binary inputs and outputs must be outside repository')
    if os.geteuid() != 0:
        raise ValueError('root required for read-only source mounts')
    if args.output.exists() or args.target.resolve() == args.output.resolve():
        raise ValueError('output must be a new file')
    if digest(args.zip, 'sha256') != SHA256 or digest(args.zip, 'sha1') != SHA1:
        raise ValueError('official archive checksum mismatch')
    args.work.mkdir(parents=True, exist_ok=True)
    disk = args.work / 'google-system-disk.img'
    if not disk.exists():
        with zipfile.ZipFile(args.zip) as archive, archive.open('x86_64/system.img') as source, disk.open('wb') as dest:
            shutil.copyfileobj(source, dest)
    # Recheck extracted bytes against the pinned archive on every invocation.
    with zipfile.ZipFile(args.zip) as archive, archive.open('x86_64/system.img') as source:
        h = hashlib.sha256()
        for block in iter(lambda: source.read(1024 * 1024), b''):
            h.update(block)
    if digest(disk, 'sha256') != h.hexdigest():
        raise ValueError('extracted disk differs from archive')
    stage = args.work / 'payload'
    if stage.exists():
        raise ValueError('use a fresh work directory for each assembly')
    stage.mkdir()
    mounts = []
    try:
        for partition, (offset, length) in logical_partitions(disk).items():
            mount = args.work / ('mount-' + partition)
            mount.mkdir(exist_ok=True)
            run('mount', '-o', f'loop,ro,offset={offset},sizelimit={length}', str(disk), str(mount))
            mounts.append(mount)
            root = stage / 'system' / partition
            selected = set(PACKAGES[partition].values())
            for folder in PACKAGES[partition]:
                source = mount / 'priv-app' / folder
                for file in source.rglob('*'):
                    # Cross-build odex/vdex are not reusable. APK-contained native libs stay intact.
                    if file.is_file() and file.suffix == '.apk':
                        dest = root / 'priv-app' / folder / file.relative_to(source)
                        dest.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(file, dest)
            for category in ('permissions', 'default-permissions', 'sysconfig'):
                for file in sorted((mount / 'etc' / category).glob('*.xml')):
                    tree = ET.parse(file)
                    chosen = [child for child in tree.getroot() if any(value in selected for value in child.attrib.values())]
                    if not chosen:
                        continue
                    filtered = ET.Element(tree.getroot().tag)
                    filtered.extend(chosen)
                    dest = root / 'etc' / category / ('ome-' + file.name)
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    ET.ElementTree(filtered).write(dest, encoding='utf-8', xml_declaration=True)
        files = sorted(p for p in stage.rglob('*') if p.is_file())
        if sum(p.suffix == '.apk' for p in files) != 3:
            raise ValueError('unexpected APK inventory')
        args.output.parent.mkdir(parents=True, exist_ok=True)
        run('cp', '--sparse=always', str(args.target), str(args.output))
        label = args.work / 'system-file.label'
        label.write_bytes(b'u:object_r:system_file:s0\0')
        commands = []
        dirs = sorted((p for p in stage.rglob('*') if p.is_dir()), key=lambda p: len(p.parts))
        for path in dirs:
            target = '/' + path.relative_to(stage).as_posix()
            info = run('debugfs', '-R', 'stat ' + quote(target), str(args.output))
            if 'Inode:' not in info:
                commands.append('mkdir ' + quote(target))
                commands.append('set_inode_field ' + quote(target) + ' mode 040755')
                commands.append('ea_set -f ' + quote(label) + ' ' + quote(target) + ' security.selinux')
        for path in files:
            target = '/' + path.relative_to(stage).as_posix()
            info = run('debugfs', '-R', 'stat ' + quote(target), str(args.output))
            if 'Inode:' in info:
                raise ValueError('refusing to overwrite existing target: ' + target)
            commands.extend(['write ' + quote(path) + ' ' + quote(target),
                             'set_inode_field ' + quote(target) + ' mode 0100644',
                             'set_inode_field ' + quote(target) + ' uid 0',
                             'set_inode_field ' + quote(target) + ' gid 0',
                             'ea_set -f ' + quote(label) + ' ' + quote(target) + ' security.selinux'])
        batch = args.work / 'debugfs.commands'
        batch.write_text('\n'.join(commands) + '\n')
        run('debugfs', '-w', '-f', str(batch), str(args.output))
        verified = []
        readback = args.work / 'readback'
        for path in files:
            target = '/' + path.relative_to(stage).as_posix()
            run('debugfs', '-R', 'dump ' + quote(target) + ' ' + quote(readback), str(args.output))
            if digest(readback, 'sha256') != digest(path, 'sha256'):
                raise ValueError('debugfs write/readback mismatch: ' + target)
            stat = run('debugfs', '-R', 'stat ' + quote(target), str(args.output))
            if 'u:object_r:system_file:s0' not in stat or '0644' not in stat:
                raise ValueError('mode or SELinux label mismatch: ' + target)
            verified.append({'path': target, 'sha256': digest(path, 'sha256'), 'size': path.stat().st_size})
        run('e2fsck', '-fn', str(args.output))
        (args.work / 'assembled-files.json').write_text(json.dumps(verified, indent=2) + '\n')
        print(json.dumps({'files': len(verified), 'output_sha256': digest(args.output, 'sha256'), 'output': str(args.output)}))
    finally:
        for mount in reversed(mounts):
            run('umount', str(mount))


if __name__ == '__main__':
    main()
