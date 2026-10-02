#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
"""Generate the source and license inventory for an OME guest image."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import fnmatch
import gzip
import hashlib
from pathlib import Path
import subprocess
import sys
from typing import Iterable
from urllib.parse import urljoin, urlparse
import xml.etree.ElementTree as ET

FORBIDDEN_PATTERNS = (
    "libndk_translation*",
    "libhoudini*",
    "houdini*",
    "libberberis*",
    "GmsCore*",
    "Phonesky*",
    "libwvdrm*",
    "libwidevine*",
    "*gapps*",
    "*microg*",
    "*ndk_translation*",
)
LICENSE_FILENAMES = {
    "license",
    "license.txt",
    "license.md",
    "copying",
    "copying.lib",
    "notice",
}
BUILD_PROPERTIES = (
    "ro.build.fingerprint",
    "ro.build.date",
    "ro.bliss.version",
    "ro.build.version.release",
    "ro.build.version.sdk",
)


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_properties(path: Path) -> dict[str, str]:
    properties: dict[str, str] = {}
    with path.open(encoding="utf-8", errors="replace") as source:
        for raw_line in source:
            line = raw_line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, value = line.split("=", 1)
            properties.setdefault(key, value)
    return properties


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_iso_identity(dist: Path) -> tuple[str, str]:
    iso_files = sorted(dist.glob("*.iso"))
    sum_files = sorted(dist.glob("*.iso.sha256"))
    if len(iso_files) != 1 or len(sum_files) != 1:
        raise ValueError(
            f"expected one ISO and one ISO checksum in {dist}; "
            f"found {len(iso_files)} and {len(sum_files)}"
        )
    fields = sum_files[0].read_text(encoding="utf-8").split()
    if not fields:
        raise ValueError(f"empty ISO checksum file: {sum_files[0]}")
    recorded = fields[0].lower()
    actual = sha256_file(iso_files[0])
    if recorded != actual:
        raise ValueError(
            f"ISO checksum mismatch: recorded {recorded}, calculated {actual}"
        )
    return iso_files[0].name, actual


def manifest_origin(tree: Path) -> str | None:
    manifest_repo = tree / ".repo" / "manifests"
    if not manifest_repo.is_dir():
        return None
    try:
        result = subprocess.run(
            ["git", "-C", str(manifest_repo), "remote", "get-url", "origin"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    origin = result.stdout.strip()
    return origin or None


def resolve_fetch(fetch: str, origin: str | None) -> str | None:
    parsed = urlparse(fetch)
    if parsed.scheme or fetch.startswith("git@"):
        return fetch.rstrip("/")
    if origin is None:
        return None
    return urljoin(origin.rstrip("/") + "/", fetch).rstrip("/")


def repository_url(
    project_name: str,
    remote_name: str,
    remotes: dict[str, str],
    origin: str | None,
) -> str:
    fetch = remotes.get(remote_name)
    if fetch is None:
        return f"{remote_name}/{project_name}"
    base = resolve_fetch(fetch, origin)
    if base is None:
        return f"{remote_name}/{project_name}"
    return f"{base}/{project_name.lstrip('/')}"


def infer_license(text: str) -> str | None:
    folded = text.casefold()
    if "apache license" in folded:
        return "APACHE2"
    if "gnu lesser" in folded:
        return "LGPL"
    if "gnu general public license" in folded and "version 2" in folded:
        return "GPL2"
    if "gnu general public license" in folded and "version 3" in folded:
        return "GPL3"
    if "mozilla public license" in folded:
        return "MPL2"
    if "mit license" in folded or "permission is hereby granted, free of charge" in folded:
        return "MIT"
    if "redistribution and use in source and binary forms" in folded:
        return "BSD"
    return None


def project_license(project_root: Path) -> tuple[str, ...]:
    if not project_root.is_dir():
        return ("not-synced",)

    try:
        entries = sorted(project_root.iterdir(), key=lambda item: item.name.casefold())
    except OSError:
        return ("not-synced",)

    module_markers = {
        entry.name[len("MODULE_LICENSE_") :].upper()
        for entry in entries
        if entry.is_file()
        and entry.name.upper().startswith("MODULE_LICENSE_")
        and len(entry.name) > len("MODULE_LICENSE_")
    }
    if module_markers:
        return tuple(sorted(module_markers))

    license_files = [
        entry
        for entry in entries
        if entry.is_file() and entry.name.casefold() in LICENSE_FILENAMES
    ]
    if not license_files:
        return ("none-found",)

    inferred: set[str] = set()
    for license_file in license_files:
        try:
            with license_file.open("rb") as source:
                text = source.read(4096).decode("utf-8", errors="replace")
        except OSError:
            continue
        marker = infer_license(text)
        if marker is not None:
            inferred.add(marker)
    return tuple(sorted(inferred)) if inferred else ("unknown",)


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def count_notice_xml(path: Path) -> tuple[int, int]:
    file_names = 0
    contents: set[str] = set()
    with gzip.open(path, "rb") as source:
        for _, element in ET.iterparse(source, events=("end",)):
            name = local_name(element.tag)
            if name == "file-name":
                file_names += 1
            elif name == "file-content":
                content_id = element.attrib.get("contentId") or element.attrib.get("id")
                if content_id is None:
                    content_id = hashlib.sha256(
                        (element.text or "").encode("utf-8")
                    ).hexdigest()
                contents.add(content_id)
            element.clear()
    return file_names, len(contents)


def markdown_cell(value: str) -> str:
    return value.replace("\\", "\\\\").replace("|", "\\|").replace("\n", " ")


def scan_file_list(file_list: Path, scan_path: Path, generated_utc: str) -> int:
    paths = [
        line
        for line in file_list.read_text(
            encoding="utf-8", errors="surrogateescape"
        ).splitlines()
        if line
    ]
    folded_patterns = tuple(pattern.casefold() for pattern in FORBIDDEN_PATTERNS)
    matches = sorted(
        path
        for path in paths
        if any(
            fnmatch.fnmatchcase(Path(path).name.casefold(), pattern)
            for pattern in folded_patterns
        )
    )
    with scan_path.open("w", encoding="utf-8", newline="\n") as output:
        output.write(
            f"checked_utc={generated_utc} patterns={','.join(FORBIDDEN_PATTERNS)}\n"
        )
        output.write(f"files={len(paths)} matches={len(matches)}\n")
        for match in matches:
            output.write(f"{match}\n")
    return len(matches)


def write_notice(
    tree: Path,
    dist: Path,
    out: Path,
    generated_utc: str,
) -> None:
    manifest_path = dist / "ome.xml"
    manifest_root = ET.parse(manifest_path).getroot()
    remote_elements = manifest_root.findall("remote")
    default = manifest_root.find("default")
    default_remote = default.get("remote", "") if default is not None else ""
    remotes = {
        remote.get("name", ""): remote.get("fetch", "")
        for remote in remote_elements
    }
    origin = manifest_origin(tree)

    projects: list[dict[str, str | tuple[str, ...]]] = []
    marker_counts: dict[str, int] = {}
    for project in manifest_root.findall("project"):
        name = project.get("name", "")
        path = project.get("path") or name
        remote = project.get("remote") or default_remote
        revision = project.get("revision", "")
        markers = project_license(tree / path)
        for marker in markers:
            marker_counts[marker] = marker_counts.get(marker, 0) + 1
        projects.append(
            {
                "name": name,
                "path": path,
                "repository": repository_url(name, remote, remotes, origin),
                "revision": revision,
                "markers": markers,
            }
        )

    properties = parse_properties(dist / "build.prop")
    iso_name, iso_sha256 = read_iso_identity(dist)
    notice_file_names, notice_contents = count_notice_xml(
        out / "NOTICE-guest.xml.gz"
    )

    notice_path = out / "NOTICE-guest.md"
    with notice_path.open("w", encoding="utf-8", newline="\n") as output:
        output.write("# Guest image source and license notice\n\n")
        output.write(f"Generated at: `{generated_utc}`\n\n")
        output.write("## Image identity\n\n")
        output.write("| property | value |\n")
        output.write("|---|---|\n")
        for key in BUILD_PROPERTIES:
            output.write(
                f"| `{key}` | `{markdown_cell(properties.get(key, 'missing'))}` |\n"
            )
        output.write("\n## Build inputs\n\n")
        output.write(
            f"The `ome.xml` snapshot contains {len(projects)} projects and "
            f"{len(remote_elements)} remotes.\n\n"
        )
        output.write(f"ISO: `{iso_name}`  \n")
        output.write(f"SHA-256: `{iso_sha256}`\n\n")
        output.write(
            "This image is built from the Android Open Source Project and the BlissOS and Android-x86 trees pinned in `ome.xml`, with every native-bridge and Google-app flag off (CLAUDE.md R3). It contains no proprietary ARM translator and no Google Mobile Services; `forbidden-scan.txt` is the file-name check that backs this statement.\n\n"
        )
        output.write(
            "Per-file license texts of the system image are in `NOTICE-guest.xml.gz` (the image's own `/system/etc/NOTICE.xml.gz`, generated by the Android build). The table below lists every source project of the build with its repository, the exact commit that was built, and the license marker found at the project root. The source of each project is that commit in that repository; the kernel source is also attached to the release as a tarball (ADR-0012 item 6).\n\n"
        )
        output.write(
            "Markers are read mechanically from `MODULE_LICENSE_*` files and the head of `LICENSE`/`COPYING` files. `unknown` means a license file exists whose text was not recognised; `none-found` means no such file sits at the project root (the project may still carry licenses deeper in its tree or in `NOTICE-guest.xml.gz`).\n\n"
        )
        output.write("## License marker summary\n\n")
        output.write("| license marker | projects |\n")
        output.write("|---|---:|\n")
        for marker in sorted(marker_counts, key=lambda item: (item.casefold(), item)):
            output.write(
                f"| `{markdown_cell(marker)}` | {marker_counts[marker]} |\n"
            )
        output.write("\n## Source projects\n\n")
        output.write("| project | path | repository | revision | license |\n")
        output.write("|---|---|---|---|---|\n")
        for project in projects:
            markers = project["markers"]
            assert isinstance(markers, tuple)
            output.write(
                "| {name} | {path} | {repository} | `{revision}` | {license} |\n".format(
                    name=markdown_cell(str(project["name"])),
                    path=markdown_cell(str(project["path"])),
                    repository=markdown_cell(str(project["repository"])),
                    revision=markdown_cell(str(project["revision"])),
                    license=markdown_cell(", ".join(markers)),
                )
            )
        output.write("\n## System-image notice index\n\n")
        output.write(
            f"`NOTICE-guest.xml.gz` contains {notice_file_names} `<file-name>` "
            f"elements and {notice_contents} distinct `<file-content>` elements.\n"
        )


def parse_args(argv: Iterable[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", type=Path, required=True)
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--file-list", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: Iterable[str]) -> int:
    args = parse_args(argv)
    generated_utc = utc_now()
    matches = scan_file_list(
        args.file_list, args.out / "forbidden-scan.txt", generated_utc
    )
    write_notice(args.tree, args.dist, args.out, generated_utc)
    return 3 if matches else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError, ET.ParseError) as error:
        print(f"notice_guest.py: {error}", file=sys.stderr)
        raise SystemExit(1)
