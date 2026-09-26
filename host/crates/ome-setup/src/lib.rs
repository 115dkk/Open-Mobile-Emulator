// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Fixed-verb argument parsing for the elevated setup broker.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::PathBuf;

use thiserror::Error;

/// Fixed-operation broker used by the elevated `ome-setup.exe` process.
#[derive(Clone, Copy, Debug, Default)]
pub struct SetupBroker;

impl SetupBroker {
    /// Parses exactly one supported verb and rejects every free-form command line.
    pub fn parse<I>(arguments: I) -> Result<SetupCommand, ParseError>
    where
        I: IntoIterator<Item = OsString>,
    {
        parse_args(arguments)
    }

    /// Resolves the trusted DISM executable below a caller-supplied `SystemRoot`.
    pub fn executable(system_root: Option<OsString>) -> Result<PathBuf, ParseError> {
        dism_path(system_root)
    }

    /// Returns the fixed, non-extensible arguments for enabling WHPX without reboot.
    pub fn arguments() -> [&'static str; 4] {
        dism_arguments()
    }
}

/// The only privileged operation accepted by [`SetupBroker`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupCommand {
    /// Enable the Windows `HypervisorPlatform` optional feature without rebooting.
    EnableWhpx,
}

/// A rejected command line; callers should print usage and exit with code 2.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ParseError {
    /// The exact single verb `enable-whpx` was not supplied.
    #[error("expected exactly one argument: enable-whpx")]
    Usage,
}

/// Parses arguments after the executable name into a fixed setup command.
pub fn parse_args<I>(arguments: I) -> Result<SetupCommand, ParseError>
where
    I: IntoIterator<Item = OsString>,
{
    let arguments: Vec<OsString> = arguments.into_iter().collect();
    if arguments.as_slice() == [OsString::from("enable-whpx")] {
        Ok(SetupCommand::EnableWhpx)
    } else {
        Err(ParseError::Usage)
    }
}

/// Resolves `%SystemRoot%\System32\dism.exe` without consulting `PATH`.
pub fn dism_path(system_root: Option<OsString>) -> Result<PathBuf, ParseError> {
    system_root
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|root| root.join("System32").join("dism.exe"))
        .ok_or(ParseError::Usage)
}

/// Returns the exact fixed DISM arguments used by `Enable-Whpx.ps1`.
pub fn dism_arguments() -> [&'static str; 4] {
    [
        "/Online",
        "/Enable-Feature",
        "/FeatureName:HypervisorPlatform",
        "/NoRestart",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_enable_whpx() {
        assert_eq!(
            parse_args([OsString::from("enable-whpx")]),
            Ok(SetupCommand::EnableWhpx)
        );
        assert_eq!(parse_args([]), Err(ParseError::Usage));
        assert_eq!(
            parse_args([OsString::from("enable-whpx"), OsString::from("extra")]),
            Err(ParseError::Usage)
        );
        assert_eq!(
            parse_args([OsString::from("unsupported-verb")]),
            Err(ParseError::Usage)
        );
    }

    #[test]
    fn resolves_dism_below_system_root() {
        assert_eq!(
            dism_path(Some(OsString::from(r"C:\Windows"))).expect("valid root"),
            PathBuf::from(r"C:\Windows\System32\dism.exe")
        );
        assert_eq!(dism_path(None), Err(ParseError::Usage));
    }

    #[test]
    fn fixed_arguments_match_launcher() {
        assert_eq!(
            dism_arguments(),
            [
                "/Online",
                "/Enable-Feature",
                "/FeatureName:HypervisorPlatform",
                "/NoRestart"
            ]
        );
    }
}
