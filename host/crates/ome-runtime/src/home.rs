// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

/// Validated root of all Open Mobile Emulator user data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmeHome {
    path: PathBuf,
}

impl OmeHome {
    /// Resolves `OME_HOME`, or `%LOCALAPPDATA%\OpenMobileEmulator` when no non-empty override exists.
    ///
    /// This does not create a directory. On hosts without either environment value it returns the
    /// relative product directory; [`OmeHome::from_path`] and [`OmeHome::ensure`] still validate it
    /// before storage use. Non-empty environment paths are normalized to absolute paths.
    pub fn resolve() -> PathBuf {
        let path = std::env::var_os("OME_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .map(|base| base.join("OpenMobileEmulator"))
            })
            .unwrap_or_else(|| PathBuf::from("OpenMobileEmulator"));
        absolute_without_create(&path).unwrap_or(path)
    }

    /// Validates a caller-resolved home path without creating it.
    ///
    /// Empty paths and paths containing explicit parent traversal are rejected. Relative paths are
    /// accepted for portable tests and resolved by the file system when [`OmeHome::ensure`] runs.
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, HomeError> {
        let path = path.into();
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|component| component == Component::ParentDir)
        {
            return Err(HomeError::InvalidPath);
        }
        Ok(Self { path })
    }

    /// Resolves the current process environment into an [`OmeHome`] value.
    pub fn resolved() -> Result<Self, HomeError> {
        Self::from_path(Self::resolve())
    }

    /// Returns the home path without creating it.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Resolves a supported immediate subdirectory without creating it.
    ///
    /// Rooted names, separators, `.` and `..` are rejected. The supported names are `artifacts`,
    /// `vm`, `logs`, `apks`, `screenshots`, and `diagnostics`.
    pub fn subdir(&self, kind: &str) -> Result<PathBuf, HomeError> {
        if !matches!(
            kind,
            "artifacts" | "vm" | "logs" | "apks" | "screenshots" | "diagnostics"
        ) || Path::new(kind).is_absolute()
            || kind.contains(['/', '\\'])
            || matches!(kind, "." | "..")
        {
            return Err(HomeError::InvalidKind);
        }
        let base = self.path.join(kind);
        let base_full = absolute_without_create(&base)?;
        let home_full = absolute_without_create(&self.path)?;
        if !base_full.starts_with(&home_full) {
            return Err(HomeError::Traversal);
        }
        Ok(base)
    }

    /// Creates the home and fixed product subdirectories idempotently.
    ///
    /// Any file-system failure is returned as [`HomeError::Io`]; no existing files are removed.
    pub fn ensure(&self) -> Result<(), HomeError> {
        fs::create_dir_all(&self.path).map_err(HomeError::Io)?;
        for kind in [
            "artifacts",
            "vm",
            "logs",
            "apks",
            "screenshots",
            "diagnostics",
        ] {
            fs::create_dir_all(self.subdir(kind)?).map_err(HomeError::Io)?;
        }
        Ok(())
    }
}

fn absolute_without_create(path: &Path) -> Result<PathBuf, HomeError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(HomeError::Io)
    }
}

/// OME home resolution and creation errors.
#[derive(Debug, Error)]
pub enum HomeError {
    /// Home is empty or contains explicit parent traversal.
    #[error("OME home path is invalid")]
    InvalidPath,
    /// Requested subdirectory is not one of the fixed product categories.
    #[error("OME home subdirectory is invalid")]
    InvalidKind,
    /// A resolved path escaped OME home.
    #[error("OME home path traversal was rejected")]
    Traversal,
    /// Directory creation or current-directory resolution failed.
    #[error("OME home storage failed")]
    Io(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subdir_rejects_traversal_and_does_not_create() {
        let directory = tempfile::tempdir().expect("temp directory");
        let home = OmeHome::from_path(directory.path().join("home")).expect("home");
        assert_eq!(
            home.subdir("logs").expect("logs"),
            directory.path().join("home/logs")
        );
        assert!(!directory.path().join("home/logs").exists());
        assert!(matches!(
            home.subdir("../logs"),
            Err(HomeError::InvalidKind)
        ));
        assert!(matches!(
            home.subdir("C:\\logs"),
            Err(HomeError::InvalidKind)
        ));
    }

    #[test]
    fn ensure_creates_fixed_directories_idempotently() {
        let directory = tempfile::tempdir().expect("temp directory");
        let home = OmeHome::from_path(directory.path().join("home")).expect("home");
        home.ensure().expect("first ensure");
        home.ensure().expect("second ensure");
        assert!(home.subdir("artifacts").expect("artifacts").is_dir());
        assert!(home.subdir("screenshots").expect("screenshots").is_dir());
    }
}
