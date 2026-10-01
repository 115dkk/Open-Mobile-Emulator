// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Read-only access to files in an ISO 9660 image with Rock Ridge `NM` names.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;

use thiserror::Error;

const LOGICAL_SECTOR_SIZE: u64 = 2_048;
const PRIMARY_VOLUME_DESCRIPTOR_SECTOR: u64 = 16;
const COPY_BUFFER_SIZE: usize = 1024 * 1024;

/// Errors encountered while reading or extracting an ISO 9660 image.
#[derive(Debug, Error)]
pub enum IsoError {
    /// An operating-system I/O operation failed.
    #[error("{0}")]
    Io(#[from] io::Error),
    /// Sector 16 does not contain an ISO 9660 primary volume descriptor.
    #[error("not an ISO 9660 image")]
    NotIso,
    /// The requested absolute path does not name a file in the image.
    #[error("{0} was not found in the image")]
    NotFound(String),
    /// The image uses a feature this reader deliberately does not support.
    #[error("{0}")]
    Unsupported(String),
}

#[derive(Clone, Debug)]
struct DirectoryRecord {
    extent: u32,
    size: u32,
    flags: u8,
    identifier: Vec<u8>,
    rock_ridge_name: Option<String>,
}

impl DirectoryRecord {
    fn is_directory(&self) -> bool {
        self.flags & 0x02 != 0
    }

    fn is_multi_extent(&self) -> bool {
        self.flags & 0x80 != 0
    }

    fn matches(&self, component: &str) -> bool {
        if let Some(name) = &self.rock_ridge_name {
            return name == component;
        }

        let identifier = strip_version(&self.identifier);
        identifier.eq_ignore_ascii_case(component.as_bytes())
    }
}

/// One opened ISO 9660 image.
#[derive(Debug)]
pub struct IsoImage {
    file: BufReader<File>,
    root: DirectoryRecord,
}

impl IsoImage {
    /// Opens an ISO 9660 image and reads its root directory record.
    pub fn open(path: &Path) -> Result<Self, IsoError> {
        let mut file = BufReader::new(File::open(path)?);
        file.seek(SeekFrom::Start(
            PRIMARY_VOLUME_DESCRIPTOR_SECTOR * LOGICAL_SECTOR_SIZE,
        ))?;

        let mut descriptor = [0_u8; LOGICAL_SECTOR_SIZE as usize];
        if let Err(error) = file.read_exact(&mut descriptor) {
            return if error.kind() == io::ErrorKind::UnexpectedEof {
                Err(IsoError::NotIso)
            } else {
                Err(error.into())
            };
        }
        if descriptor[0] != 1 || &descriptor[1..6] != b"CD001" {
            return Err(IsoError::NotIso);
        }

        let root_length = usize::from(descriptor[156]);
        if root_length < 34 || 156 + root_length > descriptor.len() {
            return Err(IsoError::Unsupported(
                "the ISO root directory record is invalid".to_owned(),
            ));
        }
        let root = parse_directory_record(&descriptor[156..156 + root_length])?;
        if !root.is_directory() {
            return Err(IsoError::Unsupported(
                "the ISO root record is not a directory".to_owned(),
            ));
        }
        if root.is_multi_extent() {
            return Err(IsoError::Unsupported(
                "multi-extent root directories are not supported".to_owned(),
            ));
        }

        Ok(Self { file, root })
    }

    /// Size in bytes of the file at an absolute path such as "/kernel" or "/efi/boot/android.cfg".
    pub fn file_size(&mut self, path: &str) -> Result<u64, IsoError> {
        let record = self.find_file(path)?;
        Ok(u64::from(record.size))
    }

    /// Copies the file at `path` to `destination` (created or truncated) and returns the bytes written.
    pub fn extract(&mut self, path: &str, destination: &Path) -> Result<u64, IsoError> {
        let record = self.find_file(path)?;
        let offset = u64::from(record.extent) * LOGICAL_SECTOR_SIZE;
        let mut remaining = u64::from(record.size);
        let mut destination = File::create(destination)?;
        let mut buffer = vec![0_u8; COPY_BUFFER_SIZE];

        self.file.seek(SeekFrom::Start(offset))?;
        while remaining > 0 {
            let requested = usize::try_from(remaining.min(COPY_BUFFER_SIZE as u64))
                .expect("copy request fits usize");
            let read = self.file.read(&mut buffer[..requested])?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "ISO file extent ended before its recorded size",
                )
                .into());
            }
            destination.write_all(&buffer[..read])?;
            remaining -= u64::try_from(read).expect("read length fits u64");
        }

        Ok(u64::from(record.size))
    }

    fn find_file(&mut self, path: &str) -> Result<DirectoryRecord, IsoError> {
        let components = path_components(path)?;
        let mut directory = self.root.clone();

        for (index, component) in components.iter().enumerate() {
            let record =
                self.find_in_directory(&directory, component)
                    .map_err(|error| match error {
                        IsoError::NotFound(_) => IsoError::NotFound(path.to_owned()),
                        other => other,
                    })?;
            let is_last = index + 1 == components.len();
            if record.is_multi_extent() {
                return Err(IsoError::Unsupported(format!(
                    "multi-extent entry {component} is not supported"
                )));
            }
            if is_last {
                if record.is_directory() {
                    return Err(IsoError::NotFound(path.to_owned()));
                }
                return Ok(record);
            }
            if !record.is_directory() {
                return Err(IsoError::NotFound(path.to_owned()));
            }
            directory = record;
        }

        Err(IsoError::NotFound(path.to_owned()))
    }

    fn find_in_directory(
        &mut self,
        directory: &DirectoryRecord,
        component: &str,
    ) -> Result<DirectoryRecord, IsoError> {
        let start = u64::from(directory.extent) * LOGICAL_SECTOR_SIZE;
        let length = u64::from(directory.size);
        let mut position = 0_u64;

        while position < length {
            self.file.seek(SeekFrom::Start(start + position))?;
            let mut record_length = [0_u8; 1];
            self.file.read_exact(&mut record_length)?;
            let record_length = u64::from(record_length[0]);

            if record_length == 0 {
                position = ((position / LOGICAL_SECTOR_SIZE) + 1) * LOGICAL_SECTOR_SIZE;
                continue;
            }
            if record_length < 34
                || position + record_length > length
                || position % LOGICAL_SECTOR_SIZE + record_length > LOGICAL_SECTOR_SIZE
            {
                return Err(IsoError::Unsupported(
                    "an ISO directory record has an invalid length".to_owned(),
                ));
            }

            let mut bytes = vec![0_u8; usize::try_from(record_length).expect("record fits usize")];
            bytes[0] = u8::try_from(record_length).expect("record length fits u8");
            self.file.read_exact(&mut bytes[1..])?;
            position += record_length;

            let record = parse_directory_record(&bytes)?;
            if is_current_or_parent(&record.identifier) {
                continue;
            }
            if record.matches(component) {
                return Ok(record);
            }
        }

        Err(IsoError::NotFound(component.to_owned()))
    }
}

fn path_components(path: &str) -> Result<Vec<&str>, IsoError> {
    if !path.starts_with('/') || path == "/" {
        return Err(IsoError::Unsupported(
            "ISO file paths must be absolute and name a file".to_owned(),
        ));
    }
    let components: Vec<_> = path[1..].split('/').collect();
    if components.iter().any(|component| component.is_empty()) {
        return Err(IsoError::Unsupported(
            "ISO file paths may not contain empty components".to_owned(),
        ));
    }
    Ok(components)
}

fn parse_directory_record(bytes: &[u8]) -> Result<DirectoryRecord, IsoError> {
    if bytes.len() < 34 || usize::from(bytes[0]) != bytes.len() {
        return Err(IsoError::Unsupported(
            "an ISO directory record is malformed".to_owned(),
        ));
    }

    let identifier_length = usize::from(bytes[32]);
    let identifier_end = 33_usize
        .checked_add(identifier_length)
        .ok_or_else(|| IsoError::Unsupported("an ISO file identifier is too long".to_owned()))?;
    if identifier_end > bytes.len() {
        return Err(IsoError::Unsupported(
            "an ISO file identifier exceeds its directory record".to_owned(),
        ));
    }

    let system_use_start = identifier_end + usize::from(identifier_length.is_multiple_of(2));
    let rock_ridge_name = if system_use_start <= bytes.len() {
        parse_rock_ridge_name(&bytes[system_use_start..])?
    } else {
        None
    };

    let extent = read_le_u32(bytes, 2)?
        .checked_add(u32::from(bytes[1]))
        .ok_or_else(|| IsoError::Unsupported("an ISO extent location overflows".to_owned()))?;
    Ok(DirectoryRecord {
        extent,
        size: read_le_u32(bytes, 10)?,
        flags: bytes[25],
        identifier: bytes[33..identifier_end].to_vec(),
        rock_ridge_name,
    })
}

fn read_le_u32(bytes: &[u8], offset: usize) -> Result<u32, IsoError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| IsoError::Unsupported("an ISO numeric field is truncated".to_owned()))?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("slice has four bytes"),
    ))
}

fn parse_rock_ridge_name(system_use: &[u8]) -> Result<Option<String>, IsoError> {
    let mut position = 0_usize;
    let mut name = Vec::new();
    let mut found = false;
    let mut expecting_continuation = false;

    while position + 4 <= system_use.len() {
        let length = usize::from(system_use[position + 2]);
        if length < 4 || position + length > system_use.len() {
            break;
        }

        if &system_use[position..position + 2] == b"NM"
            && length >= 5
            && (!found || expecting_continuation)
        {
            found = true;
            let flags = system_use[position + 4];
            name.extend_from_slice(&system_use[position + 5..position + length]);
            expecting_continuation = flags & 0x01 != 0;
            if !expecting_continuation {
                break;
            }
        }
        position += length;
    }

    if !found {
        return Ok(None);
    }
    String::from_utf8(name)
        .map(Some)
        .map_err(|_| IsoError::Unsupported("a Rock Ridge NM name is not valid UTF-8".to_owned()))
}

fn strip_version(identifier: &[u8]) -> &[u8] {
    let Some(separator) = identifier.iter().rposition(|byte| *byte == b';') else {
        return identifier;
    };
    let suffix = &identifier[separator + 1..];
    if !suffix.is_empty() && suffix.iter().all(u8::is_ascii_digit) {
        &identifier[..separator]
    } else {
        identifier
    }
}

fn is_current_or_parent(identifier: &[u8]) -> bool {
    identifier == [0] || identifier == [1]
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{IsoError, IsoImage, LOGICAL_SECTOR_SIZE};

    const ROOT_EXTENT: u32 = 20;
    const EFI_EXTENT: u32 = 22;
    const BOOT_EXTENT: u32 = 23;
    const KERNEL_EXTENT: u32 = 24;
    const INITRD_EXTENT: u32 = 25;
    const CONFIG_EXTENT: u32 = 26;
    const LARGE_EXTENT: u32 = 27;
    const MULTI_EXTENT: u32 = 30;
    const SECTOR_SIZE: usize = LOGICAL_SECTOR_SIZE as usize;
    const KERNEL_CONTENT: &[u8] = b"test kernel";
    const INITRD_CONTENT: &[u8] = b"test initrd";
    const CONFIG_CONTENT: &[u8] = b"set timeout=0\nset quiet=1\n";

    static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

    fn temp_path(label: &str) -> PathBuf {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ome-guest-install-iso-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn reads_rock_ridge_fallback_nested_spanning_and_padded_entries() {
        let image_path = temp_path("minimal");
        let extracted_path = temp_path("extracted");
        let large_path = temp_path("large");
        let large_content = build_test_iso(&image_path);
        let mut image = IsoImage::open(&image_path).expect("open test ISO");

        assert_eq!(
            image.file_size("/kernel").expect("kernel size"),
            KERNEL_CONTENT.len() as u64
        );
        assert_eq!(
            image
                .extract("/efi/boot/android.cfg", &extracted_path)
                .expect("extract nested file"),
            CONFIG_CONTENT.len() as u64
        );
        assert_eq!(
            fs::read(&extracted_path).expect("read nested extraction"),
            CONFIG_CONTENT
        );
        assert_eq!(
            image.file_size("/initrd.img").expect("fallback name"),
            INITRD_CONTENT.len() as u64
        );
        assert_eq!(
            image
                .extract("/large.bin", &large_path)
                .expect("extract multi-sector file"),
            large_content.len() as u64
        );
        assert_eq!(
            fs::read(&large_path).expect("read large extraction"),
            large_content
        );

        fs::remove_file(image_path).expect("remove test ISO");
        fs::remove_file(extracted_path).expect("remove nested extraction");
        fs::remove_file(large_path).expect("remove large extraction");
    }

    #[test]
    fn reports_missing_not_iso_and_multi_extent() {
        let image_path = temp_path("errors");
        let zero_path = temp_path("zeros");
        build_test_iso(&image_path);
        fs::write(&zero_path, vec![0_u8; 17 * SECTOR_SIZE]).expect("write zero image");

        let mut image = IsoImage::open(&image_path).expect("open test ISO");
        assert!(matches!(
            image.file_size("/missing"),
            Err(IsoError::NotFound(path)) if path == "/missing"
        ));
        assert!(matches!(
            image.file_size("/multi.bin"),
            Err(IsoError::Unsupported(message)) if message.contains("multi-extent")
        ));
        assert!(matches!(IsoImage::open(&zero_path), Err(IsoError::NotIso)));

        fs::remove_file(image_path).expect("remove test ISO");
        fs::remove_file(zero_path).expect("remove zero image");
    }

    #[test]
    #[ignore = "requires OME_TEST_ISO to name the local Bliss 16.9.7 ISO"]
    fn reads_real_bliss_iso() {
        let Ok(image_path) = std::env::var("OME_TEST_ISO") else {
            return;
        };
        let extracted_path = temp_path("real-initrd");
        let mut image = IsoImage::open(Path::new(&image_path)).expect("open real ISO");

        assert_eq!(image.file_size("/kernel").expect("kernel size"), 16_278_528);
        assert_eq!(
            image.file_size("/initrd.img").expect("initrd size"),
            8_492_807
        );
        assert_eq!(
            image
                .extract("/initrd.img", &extracted_path)
                .expect("extract initrd"),
            8_492_807
        );
        assert_eq!(
            fs::metadata(&extracted_path)
                .expect("read extracted metadata")
                .len(),
            8_492_807
        );

        fs::remove_file(extracted_path).expect("remove real ISO extraction");
    }

    fn build_test_iso(path: &Path) -> Vec<u8> {
        let large_content: Vec<u8> = (0_u16..5_000)
            .map(|value| u8::try_from(value % 251).expect("value fits u8"))
            .collect();
        let mut image = vec![0_u8; 32 * SECTOR_SIZE];

        let root_record = directory_record(&[0], ROOT_EXTENT, (2 * SECTOR_SIZE) as u32, 0x02, None);
        let pvd = &mut image[16 * SECTOR_SIZE..17 * SECTOR_SIZE];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        pvd[156..156 + root_record.len()].copy_from_slice(&root_record);

        let terminator = &mut image[17 * SECTOR_SIZE..18 * SECTOR_SIZE];
        terminator[0] = 255;
        terminator[1..6].copy_from_slice(b"CD001");
        terminator[6] = 1;

        let mut first_root_sector = Vec::new();
        first_root_sector.extend(directory_record(
            &[0],
            ROOT_EXTENT,
            (2 * SECTOR_SIZE) as u32,
            0x02,
            None,
        ));
        first_root_sector.extend(directory_record(
            &[1],
            ROOT_EXTENT,
            (2 * SECTOR_SIZE) as u32,
            0x02,
            None,
        ));
        first_root_sector.extend(directory_record(
            b"KERNEL.;1",
            KERNEL_EXTENT,
            KERNEL_CONTENT.len() as u32,
            0,
            Some("kernel"),
        ));
        first_root_sector.extend(directory_record(
            b"INITRD.IMG;1",
            INITRD_EXTENT,
            INITRD_CONTENT.len() as u32,
            0,
            None,
        ));
        first_root_sector.extend(directory_record(
            b"LARGE.BIN;1",
            LARGE_EXTENT,
            large_content.len() as u32,
            0,
            Some("large.bin"),
        ));
        first_root_sector.extend(directory_record(
            b"MULTI.BIN;1",
            MULTI_EXTENT,
            4,
            0x80,
            Some("multi.bin"),
        ));
        let mut filler = 0_u32;
        loop {
            let identifier = format!("FILLER{filler:04}.;1");
            let record = directory_record(identifier.as_bytes(), MULTI_EXTENT, 0, 0, None);
            if first_root_sector.len() + record.len() > SECTOR_SIZE - 32 {
                break;
            }
            first_root_sector.extend(record);
            filler += 1;
        }
        assert!(first_root_sector.len() < SECTOR_SIZE);
        let root_start = ROOT_EXTENT as usize * SECTOR_SIZE;
        image[root_start..root_start + first_root_sector.len()].copy_from_slice(&first_root_sector);

        let efi_record =
            directory_record(b"EFI", EFI_EXTENT, SECTOR_SIZE as u32, 0x02, Some("efi"));
        image[root_start + SECTOR_SIZE..root_start + SECTOR_SIZE + efi_record.len()]
            .copy_from_slice(&efi_record);

        write_directory(
            &mut image,
            EFI_EXTENT,
            ROOT_EXTENT,
            &[directory_record(
                b"BOOT",
                BOOT_EXTENT,
                SECTOR_SIZE as u32,
                0x02,
                Some("boot"),
            )],
        );
        write_directory(
            &mut image,
            BOOT_EXTENT,
            EFI_EXTENT,
            &[directory_record(
                b"ANDROID.CFG;1",
                CONFIG_EXTENT,
                CONFIG_CONTENT.len() as u32,
                0,
                Some("android.cfg"),
            )],
        );

        write_extent(&mut image, KERNEL_EXTENT, KERNEL_CONTENT);
        write_extent(&mut image, INITRD_EXTENT, INITRD_CONTENT);
        write_extent(&mut image, CONFIG_EXTENT, CONFIG_CONTENT);
        write_extent(&mut image, LARGE_EXTENT, &large_content);
        write_extent(&mut image, MULTI_EXTENT, b"part");

        fs::write(path, &image).expect("write test ISO");
        large_content
    }

    fn write_directory(image: &mut [u8], extent: u32, parent_extent: u32, children: &[Vec<u8>]) {
        let mut records = Vec::new();
        records.extend(directory_record(
            &[0],
            extent,
            SECTOR_SIZE as u32,
            0x02,
            None,
        ));
        records.extend(directory_record(
            &[1],
            parent_extent,
            SECTOR_SIZE as u32,
            0x02,
            None,
        ));
        for child in children {
            records.extend_from_slice(child);
        }
        assert!(records.len() <= SECTOR_SIZE);
        let start = extent as usize * SECTOR_SIZE;
        image[start..start + records.len()].copy_from_slice(&records);
    }

    fn write_extent(image: &mut [u8], extent: u32, content: &[u8]) {
        let start = extent as usize * SECTOR_SIZE;
        image[start..start + content.len()].copy_from_slice(content);
    }

    fn directory_record(
        identifier: &[u8],
        extent: u32,
        size: u32,
        flags: u8,
        rock_ridge_name: Option<&str>,
    ) -> Vec<u8> {
        let identifier_padding = usize::from(identifier.len().is_multiple_of(2));
        let mut system_use = Vec::new();
        if let Some(name) = rock_ridge_name {
            let length = 5 + name.len();
            system_use.extend_from_slice(b"NM");
            system_use.push(u8::try_from(length).expect("NM entry length fits u8"));
            system_use.push(1);
            system_use.push(0);
            system_use.extend_from_slice(name.as_bytes());
        }
        let record_length = 33 + identifier.len() + identifier_padding + system_use.len();
        let mut record = vec![0_u8; record_length];
        record[0] = u8::try_from(record_length).expect("directory record length fits u8");
        record[2..6].copy_from_slice(&extent.to_le_bytes());
        record[6..10].copy_from_slice(&extent.to_be_bytes());
        record[10..14].copy_from_slice(&size.to_le_bytes());
        record[14..18].copy_from_slice(&size.to_be_bytes());
        record[25] = flags;
        record[28..30].copy_from_slice(&1_u16.to_le_bytes());
        record[30..32].copy_from_slice(&1_u16.to_be_bytes());
        record[32] = u8::try_from(identifier.len()).expect("identifier length fits u8");
        record[33..33 + identifier.len()].copy_from_slice(identifier);
        let system_use_start = 33 + identifier.len() + identifier_padding;
        record[system_use_start..].copy_from_slice(&system_use);
        record
    }
}
