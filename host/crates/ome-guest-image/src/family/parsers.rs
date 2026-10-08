// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use super::{DisplayInfo, PackageEntry};

pub(super) fn native_bridge(output: &str) -> Option<String> {
    let name = output.trim();
    (!name.is_empty()
        && name != "0"
        && name.ends_with(".so")
        && !name.contains(char::is_whitespace))
    .then(|| name.into())
}

fn package_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._".contains(&c))
}

pub(super) fn packages(output: &str) -> Vec<PackageEntry> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.trim().strip_prefix("package:")?.split_whitespace();
            let package = fields.next()?;
            if !package_name(package) {
                return None;
            }
            let mut version_code = None;
            for field in fields {
                if let Some(version) = field.strip_prefix("versionCode:") {
                    version_code = Some(version.parse().ok()?);
                }
            }
            Some(PackageEntry {
                package: package.into(),
                version_code,
            })
        })
        .collect()
}

pub(super) fn foreground(output: &str) -> Option<String> {
    // Prefer the focused resumed activity over other resumed activities in multi-window dumps.
    for marker in ["topResumedActivity", "mResumedActivity"] {
        for line in output.lines() {
            let Some(record) = line
                .trim()
                .strip_prefix(marker)
                .and_then(|s| s.strip_prefix(['=', ':']))
            else {
                continue;
            };
            let fields: Vec<_> = record.split_whitespace().collect();
            for pair in fields.windows(2) {
                if pair[0].strip_prefix('u').is_some_and(|user| {
                    !user.is_empty() && user.bytes().all(|c| c.is_ascii_digit())
                }) && let Some((package, activity)) = pair[1].split_once('/')
                    && package_name(package)
                    && !activity.is_empty()
                {
                    return Some(package.into());
                }
            }
        }
    }
    None
}

pub(super) fn media_volume(output: &str) -> Option<u32> {
    output.lines().find_map(|line| {
        let (_, value) = line.split_once("volume is ")?;
        let (value, range) = value.split_once(" in range [")?;
        let (min, max) = range.trim().strip_suffix(']')?.split_once("..")?;
        let (value, min, max): (u32, u32, u32) =
            (value.parse().ok()?, min.parse().ok()?, max.parse().ok()?);
        (min <= value && value <= max).then_some(value)
    })
}

fn display_value<'a>(output: &'a str, physical: &str, overridden: &str) -> Option<&'a str> {
    let mut base = None;
    let mut override_value = None;
    for line in output.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix(physical) {
            base = Some(value.trim());
        }
        if let Some(value) = line.strip_prefix(overridden) {
            override_value = Some(value.trim());
        }
    }
    override_value.or(base)
}

pub(super) fn display(size: &str, density: &str) -> Option<DisplayInfo> {
    let (width, height) =
        display_value(size, "Physical size:", "Override size:")?.split_once('x')?;
    let info = DisplayInfo {
        width: width.parse().ok()?,
        height: height.parse().ok()?,
        density_dpi: display_value(density, "Physical density:", "Override density:")?
            .parse()
            .ok()?,
    };
    (info.width > 0 && info.height > 0 && info.density_dpi > 0).then_some(info)
}

pub(super) fn root_state(output: &str) -> Option<bool> {
    if output.lines().any(|line| line.trim() == "ome-su-absent") {
        return Some(false);
    }
    output
        .split_whitespace()
        .any(|word| word == "uid=0" || word.starts_with("uid=0("))
        .then_some(true)
}

pub(super) fn multitouch(output: &str) -> Option<bool> {
    // Exact QEMU name verified in v11.1.0 hw/input/virtio-input-hid.c:
    // https://gitlab.com/qemu-project/qemu/-/raw/v11.1.0/hw/input/virtio-input-hid.c
    // QEMU Virtio MultiTouch; other devices may advertise ABS_MT_SLOT directly.
    let mut device = false;
    let mut found_device = false;
    for line in output.lines().map(str::trim) {
        if line.starts_with("add device ") {
            device = true;
            found_device = true;
        }
        if !device {
            continue;
        }
        if let Some(name) = line.strip_prefix("name:")
            && name.contains("Virtio")
            && name.contains("Multi")
        {
            return Some(true);
        }
        if line.split_whitespace().any(|word| word == "ABS_MT_SLOT") {
            return Some(true);
        }
    }
    found_device.then_some(false)
}
