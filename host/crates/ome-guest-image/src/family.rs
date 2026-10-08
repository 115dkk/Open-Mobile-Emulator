// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Android-generation dialects and the first-boot capability probe (`docs/ARCHITECTURE.md` 3.15).
//!
//! A [`FamilyAdapter`] owns nothing but text: the `adb shell` arguments one Android generation
//! understands and the parsers for their output. Running those commands is the job of a
//! [`ShellRunner`], which the product runtime implements over its adb session and tests implement
//! with recorded output. This split keeps every dialect testable without a device.
//!
//! Method signatures here are the contract between the runtime and the dialects; add helpers
//! freely, but change a signature only together with every caller.

use std::fmt;

/// Android generation that shares one command dialect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuestFamily {
    /// API 28 through 29 (Android 9 and 10).
    Legacy,
    /// API 30 through 34 (Android 11 through 14).
    Modern,
    /// API 35 and newer, and every API level this crate does not know yet.
    Current,
}

impl GuestFamily {
    /// Picks the generation for an API level; unknown levels start as [`GuestFamily::Current`].
    #[must_use]
    pub const fn for_api_level(api_level: u32) -> Self {
        match api_level {
            0..=29 => Self::Legacy,
            30..=34 => Self::Modern,
            _ => Self::Current,
        }
    }
}

/// One `adb shell` invocation: the arguments that follow `shell`, never joined into one string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellCommand {
    /// Arguments passed after `adb -s <serial> shell`.
    pub args: Vec<String>,
    /// Whether `adb root` must have succeeded before this command is useful.
    pub needs_root: bool,
}

impl ShellCommand {
    /// Builds a command that runs as the shell user.
    pub fn new<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            args: args.into_iter().map(Into::into).collect(),
            needs_root: false,
        }
    }

    /// Marks the command as requiring `adb root`.
    #[must_use]
    pub fn as_root(mut self) -> Self {
        self.needs_root = true;
        self
    }
}

/// One installed third-party package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageEntry {
    /// Package name.
    pub package: String,
    /// Version code when the dialect reports it.
    pub version_code: Option<u64>,
}

/// Current display size and density as the guest reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayInfo {
    /// Width in guest pixels.
    pub width: u32,
    /// Height in guest pixels.
    pub height: u32,
    /// Density in dots per inch.
    pub density_dpi: u32,
}

/// Commands and parsers of one Android generation.
///
/// Every `*_command` method returns `None` when the generation has no way to do the work; the
/// probe then records the capability as unavailable. Parsers return `None` when the output does
/// not carry the value, whatever the exit code was.
pub trait FamilyAdapter: Send + Sync + fmt::Debug {
    /// Generation this adapter speaks for.
    fn family(&self) -> GuestFamily;

    /// Reads the boot-completed marker.
    fn boot_completed_command(&self) -> Option<ShellCommand>;
    /// Parses the boot-completed marker.
    fn parse_boot_completed(&self, output: &str) -> bool;

    /// Reads the native-bridge property.
    fn native_bridge_command(&self) -> Option<ShellCommand>;
    /// Returns the bridge library name when a native bridge is configured.
    fn parse_native_bridge(&self, output: &str) -> Option<String>;

    /// Lists third-party packages.
    fn packages_command(&self) -> Option<ShellCommand>;
    /// Parses the package list.
    fn parse_packages(&self, output: &str) -> Vec<PackageEntry>;

    /// Reads a package's user-visible label, when the generation can.
    fn app_label_command(&self, package: &str) -> Option<ShellCommand>;
    /// Parses a package label.
    fn parse_app_label(&self, output: &str) -> Option<String>;

    /// Reads the foreground package.
    fn foreground_command(&self) -> Option<ShellCommand>;
    /// Parses the foreground package name.
    fn parse_foreground(&self, output: &str) -> Option<String>;

    /// Sets the media stream volume index (0 through 15).
    fn media_volume_set_command(&self, index: u32) -> Option<ShellCommand>;
    /// Reads the media stream volume index.
    fn media_volume_get_command(&self) -> Option<ShellCommand>;
    /// Parses the media stream volume index.
    fn parse_media_volume(&self, output: &str) -> Option<u32>;

    /// Overrides the display size.
    fn display_size_command(&self, width: u32, height: u32) -> Option<ShellCommand>;
    /// Restores the physical display size.
    fn display_size_reset_command(&self) -> Option<ShellCommand>;
    /// Overrides the display density.
    fn display_density_command(&self, density_dpi: u32) -> Option<ShellCommand>;
    /// Reads the display size.
    fn display_size_query_command(&self) -> Option<ShellCommand>;
    /// Reads the display density.
    fn display_density_query_command(&self) -> Option<ShellCommand>;
    /// Parses size and density output into one record.
    fn parse_display(&self, size_output: &str, density_output: &str) -> Option<DisplayInfo>;

    /// Reads whether apps may obtain root.
    fn root_state_command(&self) -> Option<ShellCommand>;
    /// Parses the root state.
    fn parse_root_state(&self, output: &str) -> Option<bool>;

    /// Lists input devices so the multitouch device can be recognized.
    fn input_devices_command(&self) -> Option<ShellCommand>;
    /// Returns whether a multitouch-capable device is present.
    fn parse_multitouch(&self, output: &str) -> Option<bool>;

    /// Captures a screenshot into a guest file (the probe removes it afterwards).
    fn screenshot_command(&self, remote_path: &str) -> Option<ShellCommand>;
}

/// Returns the adapter for an API level.
#[must_use]
pub fn adapter_for(api_level: u32) -> Box<dyn FamilyAdapter> {
    match GuestFamily::for_api_level(api_level) {
        GuestFamily::Legacy => Box::new(LegacyAdapter),
        GuestFamily::Modern => Box::new(ModernAdapter),
        GuestFamily::Current => Box::new(CurrentAdapter),
    }
}

/// Output of one shell command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellOutput {
    /// Standard output decoded as UTF-8 (lossy).
    pub stdout: String,
    /// Process exit code; adb reports the remote command's code.
    pub exit_code: i32,
}

/// Something that can run dialect commands against one guest.
pub trait ShellRunner {
    /// Runs one `adb shell` command and returns its output; failures to reach adb are errors.
    fn shell(&self, command: &ShellCommand) -> Result<ShellOutput, RunnerError>;
    /// Restarts adbd as root; returns `Ok(())` only when root is now available.
    fn root(&self) -> Result<(), RunnerError>;
}

/// Failure to run a command at all (adb missing, connection lost, timeout).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("guest command could not run: {reason}")]
pub struct RunnerError {
    /// Short machine-oriented reason.
    pub reason: String,
}

/// Capability items the first-boot probe establishes; they mirror the product contract's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProbeItem {
    /// Boot-completion marker.
    BootMarker,
    /// Installed application list.
    AppList,
    /// Display size and density control.
    DisplaySize,
    /// Media volume control.
    MediaVolume,
    /// Screenshot capture.
    Screenshot,
    /// Foreground application lookup.
    ForegroundApp,
    /// Multitouch input device.
    Multitouch,
    /// ARM native bridge.
    NativeBridge,
    /// Root availability.
    Root,
}

impl ProbeItem {
    /// Every item in contract order.
    pub const ALL: [Self; 9] = [
        Self::BootMarker,
        Self::AppList,
        Self::DisplaySize,
        Self::MediaVolume,
        Self::Screenshot,
        Self::ForegroundApp,
        Self::Multitouch,
        Self::NativeBridge,
        Self::Root,
    ];
}

/// Result of probing one item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeState {
    /// The command ran and the parser accepted its output.
    Available,
    /// The dialect has no command, or the command ran and established absence.
    Unavailable,
    /// The command could not run, so nothing is known.
    Unknown,
}

/// Everything one probe run learned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeOutcome {
    /// State of every item, in [`ProbeItem::ALL`] order.
    pub items: Vec<(ProbeItem, ProbeState)>,
    /// Native bridge library when configured.
    pub native_bridge: Option<String>,
    /// Media volume index when read.
    pub media_volume: Option<u32>,
    /// Foreground package when read.
    pub foreground: Option<String>,
    /// Root state when read.
    pub root_enabled: Option<bool>,
    /// Display record when read.
    pub display: Option<DisplayInfo>,
    /// Third-party packages when listed.
    pub packages: Vec<PackageEntry>,
}

/// First-boot capability probe: runs each dialect command once and records what held.
#[derive(Clone, Copy, Debug, Default)]
pub struct CapabilityProbe;

mod dialects;
mod parsers;
mod probe;

pub use dialects::{CurrentAdapter, LegacyAdapter, ModernAdapter};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_boundaries_follow_the_architecture_table() {
        assert_eq!(GuestFamily::for_api_level(28), GuestFamily::Legacy);
        assert_eq!(GuestFamily::for_api_level(29), GuestFamily::Legacy);
        assert_eq!(GuestFamily::for_api_level(30), GuestFamily::Modern);
        assert_eq!(GuestFamily::for_api_level(34), GuestFamily::Modern);
        assert_eq!(GuestFamily::for_api_level(35), GuestFamily::Current);
        assert_eq!(GuestFamily::for_api_level(99), GuestFamily::Current);
    }

    #[test]
    fn adapter_for_picks_the_family() {
        let adapter = adapter_for(33);
        assert_eq!(adapter.family(), GuestFamily::Modern);
        assert_eq!(ProbeItem::ALL.len(), 9);
    }
}
