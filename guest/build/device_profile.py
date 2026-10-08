#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Validate the identity-only input copied into the guest's read-only system."""
import re
import sys
from pathlib import Path

KEYS = {
    'ro.product.model', 'ro.product.brand', 'ro.product.manufacturer',
    'ro.product.device', 'ro.product.name', 'ro.build.fingerprint',
}


def parse_profile(text):
    values = {}
    for line in text.splitlines():
        if not line or line.startswith('#'):
            continue
        key, sep, value = line.partition('=')
        if not sep or key not in KEYS or key in values:
            raise ValueError(f'unknown, duplicate or malformed profile key: {key}')
        if not re.fullmatch(r'[A-Za-z0-9_./:+-]+', value):
            raise ValueError(f'invalid identity value: {key}')
        values[key] = value
    if values.keys() != KEYS:
        raise ValueError('profile must contain all six identity properties')
    prefix = f"{values['ro.product.brand']}/{values['ro.product.name']}/{values['ro.product.device']}:"
    if not re.fullmatch(re.escape(prefix) + r'[^/]+/[^/]+/[^:]+:[^/]+/[^/]+', values['ro.build.fingerprint']):
        raise ValueError('fingerprint does not match the product identity or Android format')
    return values


if __name__ == '__main__':
    profile = Path(sys.argv[1])
    parse_profile(profile.read_text(encoding='utf-8'))
    print(f'validated identity profile: {profile.name}')
