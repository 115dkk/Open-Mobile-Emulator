#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
init_log third-party
verify_dist

# Resolve the physical root for the repo document even under the SUBST build drive.
repo=$(dirname -- "$(cygpath -u "$(cygpath -wa "$BUILD_ROOT")")")
# SUBST may remain unexpanded in cygpath. The wrapper supplies the canonical repo path.
if [[ -n "${OME_REPOSITORY_ROOT:-}" ]]; then repo=$(cygpath -u "$OME_REPOSITORY_ROOT"); fi
[[ -f "$repo/THIRD_PARTY.md" ]] || fail 'Cannot find repo THIRD_PARTY.md; set OME_REPOSITORY_ROOT to its parent.'
metadata="$OUT/third-party-metadata.txt"
: > "$metadata"
while IFS= read -r package; do
    [[ "$package" == mingw-w64-ucrt-x86_64-* ]] || fail "Unexpected DLL owner: $package"
    pacman -Qi "$package" >> "$metadata"
    printf '\n' >> "$metadata"
done < "$DIST/qemu/dll-packages.txt"

python - "$repo/THIRD_PARTY.md" "$metadata" "$OUT/THIRD_PARTY.generated.md" "$QEMU_TAG" "$(cat "$OUT/qemu-commit.txt")" <<'PY'
import pathlib
import re
import sys

original, metadata, output, tag, commit = sys.argv[1:]
text = pathlib.Path(original).read_text(encoding='utf-8')
start, end = '<!-- BEGIN GENERATED -->', '<!-- END GENERATED -->'
if text.count(start) != 1 or text.count(end) != 1 or text.index(start) >= text.index(end):
    raise SystemExit('Expected exactly one ordered marker pair in THIRD_PARTY.md')

def escape(value):
    return value.replace('|', r'\|').replace('\n', ' ')

rows = [
    '| Component | Version | License | Source | Notes |',
    '|---|---|---|---|---|',
    f'| QEMU | {tag} ({commit}) | See bundled COPYING | https://gitlab.com/qemu-project/qemu | Separate process; corresponding source offer |',
    '| QEMU firmware/data | Same QEMU source pin | See pc-bios and roms notices in source offer | https://gitlab.com/qemu-project/qemu | Includes EDK2 and installed ROM/keymap data; review individual licenses before release |',
]
for block in re.split(r'\n\s*\n', pathlib.Path(metadata).read_text(encoding='utf-8').strip()):
    fields = {}
    key = None
    for line in block.splitlines():
        match = re.match(r'^([^:]+?)\s*:\s*(.*)$', line)
        if match:
            key, value = match.groups()
            key = key.strip()
            fields[key] = value.strip()
        elif line[:1].isspace() and key:
            fields[key] += ' ' + line.strip()
    values = [fields.get(key, '') for key in ('Name', 'Version', 'Licenses', 'URL')]
    if any(not value or value == 'None' for value in values):
        raise SystemExit(f'Incomplete pacman license metadata: {fields}')
    rows.append('| ' + ' | '.join(map(escape, values)) + ' | Copied UCRT64 DLL; upstream URL from pacman -Qi |')
block = '\n'.join(rows)
updated = text[:text.index(start) + len(start)] + '\n' + block + '\n' + text[text.index(end):]
pathlib.Path(output).write_text(updated, encoding='utf-8', newline='\n')
PY
printf 'Generated candidate: %s\n' "$OUT/THIRD_PARTY.generated.md"
status=0
diff -u -- "$repo/THIRD_PARTY.md" "$OUT/THIRD_PARTY.generated.md" || status=$?
[[ "$status" -le 1 ]] || fail 'Could not compare THIRD_PARTY.md.'
printf 'Repository document was NOT modified. Merge the candidate after review.\n'
