// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Test-only helpers for crates that integrate the ISO reader.

use std::fs;
use std::path::Path;

const SECTOR_SIZE: usize = 2_048;
const ROOT_EXTENT: u32 = 20;
const KERNEL_EXTENT: u32 = 21;
const INITRD_EXTENT: u32 = 22;

/// Writes a minimal ISO 9660 image containing `/kernel` and `/initrd.img`.
pub fn write_test_iso(path: &Path, kernel: &[u8], initrd: &[u8]) {
    let mut image = vec![0_u8; 24 * SECTOR_SIZE];
    let root_record = directory_record(&[0], ROOT_EXTENT, SECTOR_SIZE as u32, 0x02);
    let pvd = &mut image[16 * SECTOR_SIZE..17 * SECTOR_SIZE];
    pvd[0] = 1;
    pvd[1..6].copy_from_slice(b"CD001");
    pvd[6] = 1;
    pvd[156..156 + root_record.len()].copy_from_slice(&root_record);

    let terminator = &mut image[17 * SECTOR_SIZE..18 * SECTOR_SIZE];
    terminator[0] = 255;
    terminator[1..6].copy_from_slice(b"CD001");
    terminator[6] = 1;

    let mut records = Vec::new();
    records.extend(directory_record(
        &[0],
        ROOT_EXTENT,
        SECTOR_SIZE as u32,
        0x02,
    ));
    records.extend(directory_record(
        &[1],
        ROOT_EXTENT,
        SECTOR_SIZE as u32,
        0x02,
    ));
    records.extend(directory_record(
        b"KERNEL;1",
        KERNEL_EXTENT,
        u32::try_from(kernel.len()).expect("test kernel length fits u32"),
        0,
    ));
    records.extend(directory_record(
        b"INITRD.IMG;1",
        INITRD_EXTENT,
        u32::try_from(initrd.len()).expect("test initrd length fits u32"),
        0,
    ));
    let root_start = ROOT_EXTENT as usize * SECTOR_SIZE;
    image[root_start..root_start + records.len()].copy_from_slice(&records);
    write_extent(&mut image, KERNEL_EXTENT, kernel);
    write_extent(&mut image, INITRD_EXTENT, initrd);
    fs::write(path, image).expect("write test ISO");
}

fn write_extent(image: &mut [u8], extent: u32, content: &[u8]) {
    let start = extent as usize * SECTOR_SIZE;
    image[start..start + content.len()].copy_from_slice(content);
}

fn directory_record(identifier: &[u8], extent: u32, size: u32, flags: u8) -> Vec<u8> {
    let padding = usize::from(identifier.len().is_multiple_of(2));
    let length = 33 + identifier.len() + padding;
    let mut record = vec![0_u8; length];
    record[0] = u8::try_from(length).expect("test directory record fits u8");
    record[2..6].copy_from_slice(&extent.to_le_bytes());
    record[6..10].copy_from_slice(&extent.to_be_bytes());
    record[10..14].copy_from_slice(&size.to_le_bytes());
    record[14..18].copy_from_slice(&size.to_be_bytes());
    record[25] = flags;
    record[28..30].copy_from_slice(&1_u16.to_le_bytes());
    record[30..32].copy_from_slice(&1_u16.to_be_bytes());
    record[32] = u8::try_from(identifier.len()).expect("test identifier fits u8");
    record[33..33 + identifier.len()].copy_from_slice(identifier);
    record
}
