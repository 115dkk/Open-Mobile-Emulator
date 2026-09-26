// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Validated guest configuration and the sole builder for QEMU command lines.
#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_NAME: &str = "default";
const DEFAULT_CPU_MODEL: &str = "Skylake-Client-v4";
const DEFAULT_MEMORY_MIB: u32 = 8192;
const DEFAULT_VCPUS: u16 = 4;
const DEFAULT_QMP_PORT: u16 = 4444;
const DEFAULT_ADB_PORT: u16 = 5555;

/// Unchecked values loaded from persistent settings or an IPC request.
///
/// Every field is optional and textual choices remain strings until
/// [`GuestConfig::validate`] accepts them.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RawGuestConfig {
    /// Guest name; omitted values become `default`.
    pub name: Option<String>,
    /// Guest memory in MiB.
    pub memory_mib: Option<i64>,
    /// Number of virtual CPUs.
    pub vcpus: Option<i64>,
    /// GPU mode (`std`, `virtio`, or `virgl`).
    pub gpu: Option<String>,
    /// Accelerator (`whpx` or `tcg`).
    pub accel: Option<String>,
    /// QEMU CPU model.
    pub cpu_model: Option<String>,
    /// Loopback QMP port.
    pub qmp_port: Option<i64>,
    /// Loopback adb forwarding port.
    pub adb_port: Option<i64>,
    /// Audio backend (`dsound`, `sdl`, or `none`).
    pub audio: Option<String>,
    /// Display backend (`sdl` or `gtk`).
    pub display: Option<String>,
    /// Additional QEMU argument elements appended after all managed options.
    #[serde(default)]
    pub extra_args: Option<Vec<String>>,
}

/// A validation failure tied to one configuration field.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ConfigIssue {
    /// A required non-empty text value was blank.
    #[error("configuration field '{field}' must not be blank")]
    Blank {
        /// Name of the rejected field.
        field: &'static str,
    },
    /// An integer was outside its accepted inclusive range.
    #[error("configuration field '{field}' must be in {min}..={max}, got {value}")]
    OutOfRange {
        /// Name of the rejected field.
        field: &'static str,
        /// Smallest accepted value.
        min: i64,
        /// Largest accepted value.
        max: i64,
        /// Rejected value.
        value: i64,
    },
    /// A string was not one of the field's fixed choices.
    #[error("configuration field '{field}' has unsupported value '{value}'")]
    Choice {
        /// Name of the rejected field.
        field: &'static str,
        /// Rejected value.
        value: String,
    },
    /// A guest name was unsafe for use as one Windows path component.
    #[error("configuration field 'name' must be one safe Windows path component")]
    InvalidName,
    /// WHPX was paired with QEMU's TCG-only `max` CPU definition.
    #[error("CPU model 'max' is not supported with WHPX; see docs/evidence/M0/guest-install.md")]
    WhpxMaxCpu,
}

/// QEMU display device mode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Gpu {
    /// Emulated standard VGA.
    Std,
    /// Virtio VGA without host OpenGL.
    Virtio,
    /// Virtio VGA with virglrenderer and host OpenGL.
    Virgl,
}

/// QEMU execution accelerator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Accel {
    /// Windows Hypervisor Platform acceleration.
    Whpx,
    /// QEMU software translation, intended for smoke tests.
    Tcg,
}

/// QEMU audio backend.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Audio {
    /// Windows DirectSound backend.
    Dsound,
    /// SDL audio backend.
    Sdl,
    /// No audio devices.
    None,
}

/// QEMU window backend.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Display {
    /// SDL window backend.
    Sdl,
    /// GTK window backend.
    Gtk,
}

/// A guest configuration whose ranges and fixed choices have been validated.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GuestConfig {
    name: String,
    memory_mib: u32,
    vcpus: u16,
    gpu: Gpu,
    accel: Accel,
    cpu_model: String,
    qmp_port: u16,
    adb_port: u16,
    audio: Audio,
    display: Display,
    extra_args: Vec<OsString>,
}

impl GuestConfig {
    /// Applies product defaults and validates every unchecked field.
    ///
    /// The broad numeric ranges intentionally match the PowerShell launcher;
    /// the product UI may offer a smaller recommended range.
    pub fn validate(raw: RawGuestConfig) -> Result<Self, ConfigIssue> {
        let name = non_blank("name", raw.name.unwrap_or_else(|| DEFAULT_NAME.to_owned()))?;
        validate_guest_name(&name)?;
        let memory_mib = ranged(
            "memory_mib",
            raw.memory_mib.unwrap_or(i64::from(DEFAULT_MEMORY_MIB)),
            128,
            1_048_576,
        )? as u32;
        let vcpus = ranged(
            "vcpus",
            raw.vcpus.unwrap_or(i64::from(DEFAULT_VCPUS)),
            1,
            1024,
        )? as u16;
        let gpu = parse_gpu(raw.gpu.as_deref().unwrap_or("virgl"))?;
        let accel = parse_accel(raw.accel.as_deref().unwrap_or("whpx"))?;
        let cpu_model = non_blank(
            "cpu_model",
            raw.cpu_model
                .unwrap_or_else(|| DEFAULT_CPU_MODEL.to_owned()),
        )?;
        if accel == Accel::Whpx && cpu_model.eq_ignore_ascii_case("max") {
            return Err(ConfigIssue::WhpxMaxCpu);
        }
        let qmp_port = ranged(
            "qmp_port",
            raw.qmp_port.unwrap_or(i64::from(DEFAULT_QMP_PORT)),
            1,
            65_535,
        )? as u16;
        let adb_port = ranged(
            "adb_port",
            raw.adb_port.unwrap_or(i64::from(DEFAULT_ADB_PORT)),
            1,
            65_535,
        )? as u16;
        let audio = parse_audio(raw.audio.as_deref().unwrap_or("dsound"))?;
        let display = parse_display(raw.display.as_deref().unwrap_or("sdl"))?;

        Ok(Self {
            name,
            memory_mib,
            vcpus,
            gpu,
            accel,
            cpu_model,
            qmp_port,
            adb_port,
            audio,
            display,
            extra_args: raw
                .extra_args
                .unwrap_or_default()
                .into_iter()
                .map(OsString::from)
                .collect(),
        })
    }

    /// Returns the safe guest name used for QEMU and log files.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns validated guest memory in MiB.
    pub fn memory_mib(&self) -> u32 {
        self.memory_mib
    }

    /// Returns the validated virtual CPU count.
    pub fn vcpus(&self) -> u16 {
        self.vcpus
    }

    /// Returns the selected GPU mode.
    pub fn gpu(&self) -> Gpu {
        self.gpu
    }

    /// Returns the selected execution accelerator.
    pub fn accel(&self) -> Accel {
        self.accel
    }

    /// Returns the validated QEMU CPU model.
    pub fn cpu_model(&self) -> &str {
        &self.cpu_model
    }

    /// Returns the loopback QMP port.
    pub fn qmp_port(&self) -> u16 {
        self.qmp_port
    }

    /// Returns the loopback adb forwarding port.
    pub fn adb_port(&self) -> u16 {
        self.adb_port
    }

    /// Returns the selected audio backend.
    pub fn audio(&self) -> Audio {
        self.audio
    }

    /// Returns the selected display backend.
    pub fn display(&self) -> Display {
        self.display
    }

    /// Returns additional discrete QEMU argument elements.
    pub fn extra_args(&self) -> &[OsString] {
        &self.extra_args
    }
}

/// Files belonging to one guest and its firmware.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestPaths {
    /// Persistent qcow2 guest disk.
    pub disk: PathBuf,
    /// Read-only OVMF code image.
    pub firmware_code: PathBuf,
    /// Writable per-guest OVMF variable image.
    pub firmware_vars: PathBuf,
    /// Optional installer ISO; required by [`QemuInvocation::for_install`].
    pub iso: Option<PathBuf>,
}

/// Location of a QEMU installation used to start a guest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QemuInstall {
    /// Path to `qemu-system-x86_64.exe`.
    pub system_exe: PathBuf,
}

/// An error encountered while creating a QEMU invocation.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InvocationError {
    /// Installation mode was requested without an ISO path.
    #[error("installer ISO path is required for an install invocation")]
    MissingIso,
}

/// A QEMU process launch represented as an executable and discrete arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QemuInvocation {
    program: PathBuf,
    args: Vec<OsString>,
    printable: String,
}

impl QemuInvocation {
    /// Builds the normal persistent-disk boot invocation.
    pub fn for_boot(config: &GuestConfig, paths: &GuestPaths, install: &QemuInstall) -> Self {
        Self::build(config, paths, install, None)
    }

    /// Builds an installer invocation, returning [`InvocationError::MissingIso`]
    /// if the guest paths contain no installer image.
    pub fn for_install(
        config: &GuestConfig,
        paths: &GuestPaths,
        install: &QemuInstall,
    ) -> Result<Self, InvocationError> {
        let iso = paths.iso.as_deref().ok_or(InvocationError::MissingIso)?;
        Ok(Self::build(config, paths, install, Some(iso)))
    }

    /// Returns the QEMU executable path.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Returns the discrete argument elements for direct process creation.
    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    /// Returns the Windows-quoted command line for the `.cmd.log` file.
    pub fn printable(&self) -> &str {
        &self.printable
    }

    fn build(
        config: &GuestConfig,
        paths: &GuestPaths,
        install: &QemuInstall,
        iso: Option<&Path>,
    ) -> Self {
        let mut args = Vec::new();
        pair(&mut args, "-name", format!("OME {}", config.name()));
        pair(&mut args, "-machine", "q35");
        // kernel-irqchip=off with WHPX boots Bliss 16.9.7 on the tested host
        // (docs/evidence/M0/guest-install.md).
        pair(
            &mut args,
            "-accel",
            match config.accel() {
                Accel::Whpx => "whpx,kernel-irqchip=off",
                Accel::Tcg => "tcg,thread=multi",
            },
        );
        // Do not substitute `max` under WHPX: it is QEMU's TCG definition and
        // Bliss stalled with it (docs/evidence/M0/guest-install.md).
        pair(&mut args, "-cpu", config.cpu_model());
        pair(&mut args, "-m", config.memory_mib().to_string());
        pair(&mut args, "-smp", config.vcpus().to_string());
        pair(
            &mut args,
            "-drive",
            prefixed_path(
                "if=pflash,format=raw,readonly=on,file=",
                &paths.firmware_code,
            ),
        );
        pair(
            &mut args,
            "-drive",
            prefixed_path("if=pflash,format=raw,file=", &paths.firmware_vars),
        );
        pair(
            &mut args,
            "-drive",
            surrounded_path("file=", &paths.disk, ",if=virtio,format=qcow2"),
        );
        if let Some(iso) = iso {
            pair(
                &mut args,
                "-drive",
                surrounded_path("file=", iso, ",media=cdrom,if=none,id=cd0"),
            );
            pair(
                &mut args,
                "-device",
                "ide-cd,drive=cd0,bootindex=0,bus=ide.0",
            );
        }
        pair(
            &mut args,
            "-device",
            match config.gpu() {
                Gpu::Std => "VGA",
                Gpu::Virtio => "virtio-vga",
                // edid=off avoids QEMU's generated 75 Hz EDID, which pinned
                // the tested guest to 38 fps instead of its 60 Hz mode
                // (docs/evidence/M0/findings-20260926.md).
                Gpu::Virgl => "virtio-vga-gl,edid=off",
            },
        );
        let display = match (config.display(), config.gpu()) {
            (Display::Sdl, Gpu::Virgl) => "sdl,show-cursor=on,gl=on",
            (Display::Gtk, Gpu::Virgl) => "gtk,show-cursor=on,gl=on",
            (Display::Sdl, _) => "sdl,show-cursor=on",
            (Display::Gtk, _) => "gtk,show-cursor=on",
        };
        pair(&mut args, "-display", display);
        pair(&mut args, "-device", "virtio-net-pci,netdev=n0");
        pair(
            &mut args,
            "-netdev",
            format!(
                "user,id=n0,hostfwd=tcp:127.0.0.1:{}-:5555",
                config.adb_port()
            ),
        );
        args.push(OsString::from("-usb"));
        pair(&mut args, "-device", "usb-tablet");
        pair(&mut args, "-device", "usb-kbd");
        match config.audio() {
            Audio::Dsound => append_audio(&mut args, "dsound"),
            Audio::Sdl => append_audio(&mut args, "sdl"),
            Audio::None => {}
        }
        pair(
            &mut args,
            "-qmp",
            format!("tcp:127.0.0.1:{},server=on,wait=off", config.qmp_port()),
        );
        // A guest reset under QEMU 11.1 WHPX leaves the VM paused with an
        // xsave error. Converting reset to process shutdown lets Supervisor
        // restart it (docs/evidence/M0/guest-install.md).
        pair(&mut args, "-action", "reboot=shutdown");
        pair(&mut args, "-rtc", "base=utc");
        args.extend(config.extra_args().iter().cloned());

        let printable = std::iter::once(install.system_exe.as_os_str())
            .chain(args.iter().map(OsString::as_os_str))
            .map(quote_windows_argument)
            .collect::<Vec<_>>()
            .join(" ");
        Self {
            program: install.system_exe.clone(),
            args,
            printable,
        }
    }
}

fn append_audio(args: &mut Vec<OsString>, backend: &str) {
    pair(args, "-audiodev", format!("{backend},id=snd0"));
    pair(args, "-device", "intel-hda");
    pair(args, "-device", "hda-duplex,audiodev=snd0");
}

fn pair(args: &mut Vec<OsString>, flag: &str, value: impl Into<OsString>) {
    args.push(OsString::from(flag));
    args.push(value.into());
}

fn prefixed_path(prefix: &str, path: &Path) -> OsString {
    let mut value = OsString::from(prefix);
    value.push(path.as_os_str());
    value
}

fn surrounded_path(prefix: &str, path: &Path, suffix: &str) -> OsString {
    let mut value = prefixed_path(prefix, path);
    value.push(suffix);
    value
}

fn non_blank(field: &'static str, value: String) -> Result<String, ConfigIssue> {
    if value.trim().is_empty() {
        Err(ConfigIssue::Blank { field })
    } else {
        Ok(value)
    }
}

fn validate_guest_name(name: &str) -> Result<(), ConfigIssue> {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let invalid_character = name
        .chars()
        .any(|character| character.is_control() || r#"<>:\/\|?*"#.contains(character));
    let base = name.split('.').next().unwrap_or(name);
    if name == "."
        || name == ".."
        || name.ends_with([' ', '.'])
        || invalid_character
        || RESERVED
            .iter()
            .any(|reserved| base.eq_ignore_ascii_case(reserved))
    {
        Err(ConfigIssue::InvalidName)
    } else {
        Ok(())
    }
}

fn ranged(field: &'static str, value: i64, min: i64, max: i64) -> Result<i64, ConfigIssue> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(ConfigIssue::OutOfRange {
            field,
            min,
            max,
            value,
        })
    }
}

fn parse_gpu(value: &str) -> Result<Gpu, ConfigIssue> {
    match value {
        "std" => Ok(Gpu::Std),
        "virtio" => Ok(Gpu::Virtio),
        "virgl" => Ok(Gpu::Virgl),
        _ => Err(choice("gpu", value)),
    }
}

fn parse_accel(value: &str) -> Result<Accel, ConfigIssue> {
    match value {
        "whpx" => Ok(Accel::Whpx),
        "tcg" => Ok(Accel::Tcg),
        _ => Err(choice("accel", value)),
    }
}

fn parse_audio(value: &str) -> Result<Audio, ConfigIssue> {
    match value {
        "dsound" => Ok(Audio::Dsound),
        "sdl" => Ok(Audio::Sdl),
        "none" => Ok(Audio::None),
        _ => Err(choice("audio", value)),
    }
}

fn parse_display(value: &str) -> Result<Display, ConfigIssue> {
    match value {
        "sdl" => Ok(Display::Sdl),
        "gtk" => Ok(Display::Gtk),
        _ => Err(choice("display", value)),
    }
}

fn choice(field: &'static str, value: &str) -> ConfigIssue {
    ConfigIssue::Choice {
        field,
        value: value.to_owned(),
    }
}

fn quote_windows_argument(value: &OsStr) -> String {
    let value = value.to_string_lossy();
    if !value.is_empty()
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character == '"')
    {
        return value.into_owned();
    }
    let mut quoted = String::from('"');
    let mut backslashes = 0_usize;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
                quoted.push(character);
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Fixture {
        input: FixtureInput,
        args: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct FixtureInput {
        mode: String,
        name: String,
        disk: PathBuf,
        firmware_code: PathBuf,
        firmware_vars: PathBuf,
        iso: Option<PathBuf>,
        gpu: String,
        accel: String,
        cpu_model: String,
        memory_mib: i64,
        vcpus: i64,
        qmp_port: i64,
        adb_port: i64,
        audio: String,
        display: String,
        extra_args: Option<Vec<String>>,
    }

    #[test]
    fn defaults_match_the_product_contract() {
        let config = GuestConfig::validate(RawGuestConfig::default()).expect("defaults validate");
        assert_eq!(config.name(), "default");
        assert_eq!(config.memory_mib(), 8192);
        assert_eq!(config.vcpus(), 4);
        assert_eq!(config.gpu(), Gpu::Virgl);
        assert_eq!(config.accel(), Accel::Whpx);
        assert_eq!(config.cpu_model(), "Skylake-Client-v4");
        assert_eq!(config.qmp_port(), 4444);
        assert_eq!(config.adb_port(), 5555);
        assert_eq!(config.audio(), Audio::Dsound);
        assert_eq!(config.display(), Display::Sdl);
    }

    #[test]
    fn validation_rejects_each_invalid_class() {
        let cases = [
            (
                RawGuestConfig {
                    memory_mib: Some(127),
                    ..RawGuestConfig::default()
                },
                ConfigIssue::OutOfRange {
                    field: "memory_mib",
                    min: 128,
                    max: 1_048_576,
                    value: 127,
                },
            ),
            (
                RawGuestConfig {
                    vcpus: Some(1025),
                    ..RawGuestConfig::default()
                },
                ConfigIssue::OutOfRange {
                    field: "vcpus",
                    min: 1,
                    max: 1024,
                    value: 1025,
                },
            ),
            (
                RawGuestConfig {
                    gpu: Some("other".into()),
                    ..RawGuestConfig::default()
                },
                ConfigIssue::Choice {
                    field: "gpu",
                    value: "other".into(),
                },
            ),
            (
                RawGuestConfig {
                    qmp_port: Some(0),
                    ..RawGuestConfig::default()
                },
                ConfigIssue::OutOfRange {
                    field: "qmp_port",
                    min: 1,
                    max: 65_535,
                    value: 0,
                },
            ),
            (
                RawGuestConfig {
                    name: Some("  ".into()),
                    ..RawGuestConfig::default()
                },
                ConfigIssue::Blank { field: "name" },
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(GuestConfig::validate(raw), Err(expected));
        }
    }

    #[test]
    fn guest_name_is_one_safe_windows_path_component() {
        for name in ["../outside", r"folder\guest", "CON", "name.", "bad:name"] {
            assert_eq!(
                GuestConfig::validate(RawGuestConfig {
                    name: Some(name.into()),
                    ..RawGuestConfig::default()
                }),
                Err(ConfigIssue::InvalidName),
                "{name:?}"
            );
        }
        assert!(
            GuestConfig::validate(RawGuestConfig {
                name: Some("default-2".into()),
                ..RawGuestConfig::default()
            })
            .is_ok()
        );
    }

    #[test]
    fn whpx_rejects_max_cpu_with_evidence_in_the_error() {
        let error = GuestConfig::validate(RawGuestConfig {
            cpu_model: Some("max".into()),
            ..RawGuestConfig::default()
        })
        .expect_err("WHPX max must fail");
        assert_eq!(error, ConfigIssue::WhpxMaxCpu);
        assert!(
            error
                .to_string()
                .contains("docs/evidence/M0/guest-install.md")
        );
    }

    #[test]
    fn tcg_accepts_max_cpu() {
        GuestConfig::validate(RawGuestConfig {
            accel: Some("tcg".into()),
            cpu_model: Some("max".into()),
            ..RawGuestConfig::default()
        })
        .expect("TCG max is valid");
    }

    #[test]
    fn install_requires_an_iso() {
        let config = GuestConfig::validate(RawGuestConfig::default()).expect("valid config");
        let paths = GuestPaths {
            disk: "C:\\vm\\disk.qcow2".into(),
            firmware_code: "C:\\fw\\code.fd".into(),
            firmware_vars: "C:\\vm\\efivars.fd".into(),
            iso: None,
        };
        let install = QemuInstall {
            system_exe: "qemu-system-x86_64.exe".into(),
        };
        assert_eq!(
            QemuInvocation::for_install(&config, &paths, &install),
            Err(InvocationError::MissingIso)
        );
    }

    #[test]
    fn rust_arguments_match_powershell_fixtures_element_by_element() {
        const FIXTURES: &[(&str, &str)] = &[
            (
                "default-whpx-virgl.json",
                include_str!("../../../../tests/fixtures/qemu-args/default-whpx-virgl.json"),
            ),
            (
                "tcg-std-no-audio.json",
                include_str!("../../../../tests/fixtures/qemu-args/tcg-std-no-audio.json"),
            ),
            (
                "install-cdrom.json",
                include_str!("../../../../tests/fixtures/qemu-args/install-cdrom.json"),
            ),
            (
                "gtk-display.json",
                include_str!("../../../../tests/fixtures/qemu-args/gtk-display.json"),
            ),
            (
                "custom-ports-extra.json",
                include_str!("../../../../tests/fixtures/qemu-args/custom-ports-extra.json"),
            ),
        ];
        for (name, json) in FIXTURES {
            let fixture: Fixture = serde_json::from_str(json).expect("fixture JSON parses");
            let input = fixture.input;
            let config = GuestConfig::validate(RawGuestConfig {
                name: Some(input.name),
                memory_mib: Some(input.memory_mib),
                vcpus: Some(input.vcpus),
                gpu: Some(input.gpu),
                accel: Some(input.accel),
                cpu_model: Some(input.cpu_model),
                qmp_port: Some(input.qmp_port),
                adb_port: Some(input.adb_port),
                audio: Some(input.audio),
                display: Some(input.display),
                extra_args: Some(input.extra_args.unwrap_or_default()),
            })
            .expect("fixture config validates");
            let paths = GuestPaths {
                disk: input.disk,
                firmware_code: input.firmware_code,
                firmware_vars: input.firmware_vars,
                iso: input.iso,
            };
            let install = QemuInstall {
                system_exe: "C:\\qemu\\qemu-system-x86_64.exe".into(),
            };
            let invocation = if input.mode == "install" {
                QemuInvocation::for_install(&config, &paths, &install)
                    .expect("install fixture has ISO")
            } else {
                QemuInvocation::for_boot(&config, &paths, &install)
            };
            let actual: Vec<String> = invocation
                .args()
                .iter()
                .map(|item| item.to_string_lossy().into_owned())
                .collect();
            assert_eq!(actual, fixture.args, "fixture {name}");
        }
    }

    #[test]
    fn printable_quotes_spaces_and_empty_values() {
        assert_eq!(quote_windows_argument(OsStr::new("plain")), "plain");
        assert_eq!(quote_windows_argument(OsStr::new("")), "\"\"");
        assert_eq!(
            quote_windows_argument(OsStr::new("two words")),
            "\"two words\""
        );
    }
}
