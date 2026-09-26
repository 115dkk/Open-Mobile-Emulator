// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Local diagnostic ZIP collection with environment-key redaction.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// Inputs for one local diagnostic archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleRequest {
    /// Host log files copied under `logs/`.
    pub host_logs: Vec<PathBuf>,
    /// Virtual-machine process logs copied under `qemu/`.
    pub qemu_logs: Vec<PathBuf>,
    /// Optional pre-collected 2000-line guest log tail.
    pub logcat: Option<Vec<u8>>,
    /// Environment entries; privacy-sensitive keys are excluded case-insensitively.
    pub environment: Vec<(String, String)>,
    /// Existing or creatable destination directory.
    pub out_dir: PathBuf,
}

/// Collector for one self-contained diagnostic archive.
#[derive(Clone, Copy, Debug, Default)]
pub struct DiagnosticBundle;

impl DiagnosticBundle {
    /// Collects all requested readable sources into `ome-diagnostics-yyyyMMdd-HHmmss.zip`.
    ///
    /// Missing, non-regular, duplicate-leaf, and unreadable source files fail the whole operation;
    /// no partial final archive is retained. Environment keys containing `serial`, `account`, or
    /// `token` in any letter case are omitted. Values are copied as supplied and must already be safe
    /// for user-controlled diagnostics.
    pub fn collect(request: BundleRequest) -> Result<PathBuf, BundleError> {
        Self::collect_at(request, SystemTime::now())
    }

    fn collect_at(request: BundleRequest, now: SystemTime) -> Result<PathBuf, BundleError> {
        fs::create_dir_all(&request.out_dir).map_err(BundleError::Io)?;
        let stamp = format_utc(now)?;
        let final_path = request.out_dir.join(format!("ome-diagnostics-{stamp}.zip"));
        if final_path.exists() {
            return Err(BundleError::AlreadyExists);
        }
        let staging_path = request
            .out_dir
            .join(format!("ome-diagnostics-{stamp}.zip.part"));
        if staging_path.exists() {
            fs::remove_file(&staging_path).map_err(BundleError::Io)?;
        }

        let result = (|| {
            let file = File::create(&staging_path).map_err(BundleError::Io)?;
            let mut archive = ZipWriter::new(file);
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o600);
            let mut names = BTreeSet::new();
            add_directory(&mut archive, "logs/", options)?;
            add_directory(&mut archive, "qemu/", options)?;
            add_files(
                &mut archive,
                &request.host_logs,
                "logs",
                options,
                &mut names,
            )?;
            add_files(
                &mut archive,
                &request.qemu_logs,
                "qemu",
                options,
                &mut names,
            )?;
            if let Some(logcat) = request.logcat {
                archive
                    .start_file("logcat.txt", options)
                    .map_err(BundleError::Zip)?;
                archive.write_all(&logcat).map_err(BundleError::Io)?;
            }
            archive
                .start_file("environment.txt", options)
                .map_err(BundleError::Zip)?;
            for (key, value) in request.environment {
                if is_private_key(&key) {
                    continue;
                }
                writeln!(archive, "{key}={value}").map_err(BundleError::Io)?;
            }
            let file = archive.finish().map_err(BundleError::Zip)?;
            file.sync_all().map_err(BundleError::Io)?;
            fs::rename(&staging_path, &final_path).map_err(BundleError::Io)?;
            Ok(final_path.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&staging_path);
        }
        result
    }
}

fn add_directory<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    name: &str,
    options: SimpleFileOptions,
) -> Result<(), BundleError> {
    archive
        .add_directory(name, options)
        .map_err(BundleError::Zip)
}

fn add_files<W: Write + Seek>(
    archive: &mut ZipWriter<W>,
    paths: &[PathBuf],
    prefix: &str,
    options: SimpleFileOptions,
    names: &mut BTreeSet<String>,
) -> Result<(), BundleError> {
    for path in paths {
        if !path.is_file() {
            return Err(BundleError::InvalidSource);
        }
        let leaf = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(BundleError::InvalidSource)?;
        let archive_name = format!("{prefix}/{leaf}");
        if !names.insert(archive_name.clone()) {
            return Err(BundleError::DuplicateName(archive_name));
        }
        archive
            .start_file(archive_name, options)
            .map_err(BundleError::Zip)?;
        let mut source = File::open(path).map_err(BundleError::Io)?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = source.read(&mut buffer).map_err(BundleError::Io)?;
            if count == 0 {
                break;
            }
            archive
                .write_all(&buffer[..count])
                .map_err(BundleError::Io)?;
        }
    }
    Ok(())
}

fn is_private_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["serial", "account", "token"]
        .iter()
        .any(|private| key.contains(private))
}

fn format_utc(time: SystemTime) -> Result<String, BundleError> {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| BundleError::InvalidTime)?
        .as_secs();
    let days = seconds / 86_400;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) =
        civil_from_days(i64::try_from(days).map_err(|_| BundleError::InvalidTime)?);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    Ok(format!(
        "{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}"
    ))
}

// Howard Hinnant's public-domain civil calendar conversion for days since 1970-01-01.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (
        year,
        u32::try_from(month).expect("calendar month is positive"),
        u32::try_from(day).expect("calendar day is positive"),
    )
}

/// Diagnostic bundle errors without source contents in display text.
#[derive(Debug, Error)]
pub enum BundleError {
    /// A source is missing, not a regular file, or has no safe UTF-8 leaf name.
    #[error("diagnostic source is invalid")]
    InvalidSource,
    /// Two requested sources would use the same archive entry name.
    #[error("diagnostic archive name is duplicated: {0}")]
    DuplicateName(String),
    /// A bundle with the same second-resolution timestamp already exists.
    #[error("diagnostic bundle already exists")]
    AlreadyExists,
    /// File-system I/O failed.
    #[error("diagnostic bundle I/O failed")]
    Io(#[source] io::Error),
    /// ZIP structure or compression failed.
    #[error("diagnostic ZIP failed")]
    Zip(#[source] zip::result::ZipError),
    /// System time cannot be represented in the filename format.
    #[error("diagnostic timestamp is invalid")]
    InvalidTime,
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::time::Duration;

    use zip::ZipArchive;

    use super::*;

    #[test]
    fn bundle_has_expected_layout_and_redacts_private_keys() {
        let directory = tempfile::tempdir().expect("temp directory");
        let host = directory.path().join("host.log");
        let qemu = directory.path().join("guest.stderr.log");
        fs::write(&host, b"host data").expect("host log");
        fs::write(&qemu, b"guest data").expect("qemu log");

        let path = DiagnosticBundle::collect_at(
            BundleRequest {
                host_logs: vec![host],
                qemu_logs: vec![qemu],
                logcat: Some(b"logcat data".to_vec()),
                environment: vec![
                    ("os".to_owned(), "Windows".to_owned()),
                    ("device_serial".to_owned(), "secret".to_owned()),
                    ("AccountName".to_owned(), "secret".to_owned()),
                    ("access_token".to_owned(), "secret".to_owned()),
                ],
                out_dir: directory.path().join("out"),
            },
            UNIX_EPOCH + Duration::from_secs(1_790_380_800),
        )
        .expect("collect bundle");
        assert_eq!(
            path.file_name().and_then(|value| value.to_str()),
            Some("ome-diagnostics-20260926-000000.zip")
        );

        let mut archive = ZipArchive::new(File::open(path).expect("open bundle")).expect("zip");
        assert_eq!(read_entry(&mut archive, "logs/host.log"), "host data");
        assert_eq!(
            read_entry(&mut archive, "qemu/guest.stderr.log"),
            "guest data"
        );
        assert_eq!(read_entry(&mut archive, "logcat.txt"), "logcat data");
        assert_eq!(read_entry(&mut archive, "environment.txt"), "os=Windows\n");
    }

    #[test]
    fn existing_timestamped_bundle_is_not_deleted() {
        let directory = tempfile::tempdir().expect("temp directory");
        let out = directory.path().join("out");
        fs::create_dir(&out).expect("output directory");
        let existing = out.join("ome-diagnostics-19700101-000000.zip");
        fs::write(&existing, b"existing").expect("existing bundle");
        let result = DiagnosticBundle::collect_at(
            BundleRequest {
                host_logs: Vec::new(),
                qemu_logs: Vec::new(),
                logcat: None,
                environment: Vec::new(),
                out_dir: out,
            },
            UNIX_EPOCH,
        );
        assert!(result.is_err());
        assert_eq!(
            fs::read(existing).expect("existing bundle remains"),
            b"existing"
        );
    }

    #[test]
    fn missing_source_fails_without_final_archive() {
        let directory = tempfile::tempdir().expect("temp directory");
        let result = DiagnosticBundle::collect_at(
            BundleRequest {
                host_logs: vec![directory.path().join("missing.log")],
                qemu_logs: Vec::new(),
                logcat: None,
                environment: Vec::new(),
                out_dir: directory.path().join("out"),
            },
            UNIX_EPOCH,
        );
        assert!(matches!(result, Err(BundleError::InvalidSource)));
        assert!(
            !directory
                .path()
                .join("out/ome-diagnostics-19700101-000000.zip")
                .exists()
        );
    }

    fn read_entry(archive: &mut ZipArchive<File>, name: &str) -> String {
        let mut value = String::new();
        archive
            .by_name(name)
            .expect("entry")
            .read_to_string(&mut value)
            .expect("read entry");
        value
    }
}
