// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Minimal writer for SVR4 `newc` archives.

/// One entry of a newc archive. Names are relative (no leading slash).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry<'a> {
    /// A directory with its complete Unix mode.
    Directory { name: &'a str, mode: u32 },
    /// A regular file with its complete Unix mode and contents.
    File {
        name: &'a str,
        mode: u32,
        data: &'a [u8],
    },
}

/// Serialises the entries as one SVR4 "newc" archive (magic "070701") ending in the TRAILER!!! record.
#[must_use]
pub fn newc_archive(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut archive = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let inode = u32::try_from(index + 1).expect("newc inode number exceeds u32");
        match entry {
            Entry::Directory { name, mode } => {
                write_record(&mut archive, inode, *mode, 2, name, &[]);
            }
            Entry::File { name, mode, data } => {
                write_record(&mut archive, inode, *mode, 1, name, data);
            }
        }
    }

    write_record(&mut archive, 0, 0, 1, "TRAILER!!!", &[]);
    archive
}

fn write_record(archive: &mut Vec<u8>, inode: u32, mode: u32, links: u32, name: &str, data: &[u8]) {
    let file_size = u32::try_from(data.len()).expect("newc file size exceeds u32");
    let name_size = u32::try_from(name.len() + 1).expect("newc name size exceeds u32");
    let header = format!(
        "070701{inode:08x}{mode:08x}{uid:08x}{gid:08x}{links:08x}{mtime:08x}\
         {file_size:08x}{dev_major:08x}{dev_minor:08x}{rdev_major:08x}\
         {rdev_minor:08x}{name_size:08x}{check:08x}",
        uid = 0,
        gid = 0,
        mtime = 0,
        dev_major = 0,
        dev_minor = 0,
        rdev_major = 0,
        rdev_minor = 0,
        check = 0,
    );
    debug_assert_eq!(header.len(), 110);
    archive.extend_from_slice(header.as_bytes());
    archive.extend_from_slice(name.as_bytes());
    archive.push(0);
    pad_to_four_bytes(archive);
    archive.extend_from_slice(data);
    pad_to_four_bytes(archive);
}

fn pad_to_four_bytes(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, newc_archive};

    #[test]
    fn writes_exact_newc_records_padding_and_trailer() {
        let archive = newc_archive(&[
            Entry::Directory {
                name: "scripts",
                mode: 0o040755,
            },
            Entry::File {
                name: "hello",
                mode: 0o100644,
                data: b"abcde",
            },
        ]);

        let directory_header = concat!(
            "070701", "00000001", "000041ed", "00000000", "00000000", "00000002", "00000000",
            "00000000", "00000000", "00000000", "00000000", "00000000", "00000008", "00000000"
        );
        let file_header = concat!(
            "070701", "00000002", "000081a4", "00000000", "00000000", "00000001", "00000000",
            "00000005", "00000000", "00000000", "00000000", "00000000", "00000006", "00000000"
        );
        let trailer_header = concat!(
            "070701", "00000000", "00000000", "00000000", "00000000", "00000001", "00000000",
            "00000000", "00000000", "00000000", "00000000", "00000000", "0000000b", "00000000"
        );

        let mut expected = Vec::new();
        expected.extend_from_slice(directory_header.as_bytes());
        expected.extend_from_slice(b"scripts\0");
        expected.extend_from_slice(b"\0\0");
        expected.extend_from_slice(file_header.as_bytes());
        expected.extend_from_slice(b"hello\0");
        expected.extend_from_slice(b"abcde");
        expected.extend_from_slice(b"\0\0\0");
        expected.extend_from_slice(trailer_header.as_bytes());
        expected.extend_from_slice(b"TRAILER!!!\0");
        expected.extend_from_slice(b"\0\0\0");

        assert_eq!(archive, expected);
        assert_eq!(archive.len() % 4, 0);
    }
}
