// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Shared command table. Verified means existing project device evidence, not a new device run.
//!
//! Source-checked 2026-09-27 (AOSP mirror, tags android-{9,13,15}.0.0_r1):
//! https://github.com/aosp-mirror/platform_frameworks_base
//! - services/core/java/com/android/server/pm/PackageManagerShellCommand.java
//! - services/core/java/com/android/server/wm/WindowManagerShellCommand.java
//! - services/core/java/com/android/server/media/{MediaShellCommand,VolumeCtrl}.java (13,15)
//! - cmds/content/src/com/android/commands/content/Content.java (13)
//! - cmds/screencap/screencap.cpp (13)
//!
//! https://github.com/aosp-mirror/platform_system_core/tree/android-13.0.0_r1/toolbox
//! - getprop.cpp, getevent.c
//!
//! Later releases are assumed to retain these commands; the probe checks the actual image.

use super::parsers;
use super::{
    Attempt, DeviceId, DisplayInfo, FamilyAdapter, GuestFamily, PackageEntry, PushFile,
    ShellCommand,
};

/// Quotes one argument for adb's remote shell, not for the host process API.
pub(super) fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/_-.".contains(&c))
    {
        value.into()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn device_id_attempts() -> Vec<Attempt> {
    vec![
        Attempt {
            push: None,
            // Documented content CLI syntax; GSF's private provider schema is assumed.
            // The enclosing double quotes preserve the SQL single quotes in the remote shell.
            command: ShellCommand::new([
                "content", "query", "--uri", "content://com.google.android.gsf.gservices",
                "--projection", "android_id", "--where", "\"name='android_id'\"",
            ]).as_root(),
        },
        Attempt {
            // Verified on Bliss 16.9.7: docs/evidence/M0/google-account.md.
            push: Some(PushFile {
                remote_path: "/data/local/tmp/ome-gsf.sql".into(),
                contents: b"select name,value from main where name='android_id';\n".to_vec(),
            }),
            command: ShellCommand::new([
                "sh", "-c",
                "'sqlite3 /data/data/com.google.android.gsf/databases/gservices.db < /data/local/tmp/ome-gsf.sql'",
            ]).as_root(),
        },
    ]
}

macro_rules! adapter {
    ($name:ident, $family:expr, $volume:expr) => {
        /// Android command dialect with generation-specific availability.
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $name;

        impl FamilyAdapter for $name {
            fn family(&self) -> GuestFamily { $family }
            fn boot_completed_command(&self) -> Option<ShellCommand> {
                // Verified: launcher/OME.Common.psm1, Wait-OmeAdbBoot (Android 13).
                Some(ShellCommand::new(["getprop", "sys.boot_completed"]))
            }
            fn parse_boot_completed(&self, output: &str) -> bool { output.trim() == "1" }
            fn native_bridge_command(&self) -> Option<ShellCommand> {
                // Verified: docs/evidence/M0/bridge-props.txt (Android 13).
                Some(ShellCommand::new(["getprop", "ro.dalvik.vm.native.bridge"]))
            }
            fn parse_native_bridge(&self, output: &str) -> Option<String> { parsers::native_bridge(output) }
            fn packages_command(&self) -> Option<ShellCommand> {
                // Documented in PackageManagerShellCommand even at API 28; no fallback needed.
                Some(ShellCommand::new(["pm", "list", "packages", "-3", "--show-versioncode"]))
            }
            fn parse_packages(&self, output: &str) -> Vec<PackageEntry> { parsers::packages(output) }
            fn app_label_command(&self, _package: &str) -> Option<ShellCommand> {
                // No documented inexpensive shell label query without aapt. Use the package name.
                None
            }
            fn parse_app_label(&self, _output: &str) -> Option<String> { None }
            fn foreground_command(&self) -> Option<ShellCommand> {
                // Documented: ActivityStackSupervisor (9), Task (13,15), dumpActivities.
                Some(ShellCommand::new(["dumpsys", "activity", "activities"]))
            }
            fn parse_foreground(&self, output: &str) -> Option<String> { parsers::foreground(output) }
            fn media_volume_set_command(&self, index: u32) -> Option<ShellCommand> {
                // Documented: MediaShellCommand/VolumeCtrl (13,15). Legacy's binder transaction
                // numbers are not a stable documented CLI contract; do not guess service call audio.
                if !$volume || index > 15 { return None; }
                Some(ShellCommand::new(["cmd", "media_session", "volume", "--stream", "3", "--set", &index.to_string()]))
            }
            fn media_volume_get_command(&self) -> Option<ShellCommand> {
                $volume.then(|| ShellCommand::new(["cmd", "media_session", "volume", "--stream", "3", "--get"]))
            }
            fn parse_media_volume(&self, output: &str) -> Option<u32> { parsers::media_volume(output) }
            fn display_size_command(&self, width: u32, height: u32) -> Option<ShellCommand> {
                // Documented: WindowManagerShellCommand (9,15).
                (width > 0 && height > 0).then(|| ShellCommand::new(["wm", "size", &format!("{width}x{height}")]))
            }
            fn display_size_reset_command(&self) -> Option<ShellCommand> { Some(ShellCommand::new(["wm", "size", "reset"])) }
            fn display_density_command(&self, density_dpi: u32) -> Option<ShellCommand> {
                // wm rejects densities below 72.
                (density_dpi >= 72).then(|| ShellCommand::new(["wm", "density", &density_dpi.to_string()]))
            }
            fn display_size_query_command(&self) -> Option<ShellCommand> { Some(ShellCommand::new(["wm", "size"])) }
            fn display_density_query_command(&self) -> Option<ShellCommand> { Some(ShellCommand::new(["wm", "density"])) }
            fn parse_display(&self, size: &str, density: &str) -> Option<DisplayInfo> { parsers::display(size, density) }
            fn device_id_attempts(&self) -> Vec<Attempt> { device_id_attempts() }
            fn parse_device_id(&self, output: &str) -> Option<DeviceId> { parsers::device_id(output) }
            fn google_accounts_command(&self) -> Option<ShellCommand> {
                // Verified: docs/evidence/M0/google-account-added.txt. Documented:
                // AccountManagerService.java (13), dumpUser(). Never return account names.
                Some(ShellCommand::new(["dumpsys", "account"]))
            }
            fn parse_google_accounts(&self, output: &str) -> Option<u32> { parsers::google_accounts(output) }
            fn add_google_account_command(&self) -> Option<ShellCommand> {
                // Verified: docs/evidence/M0/google-account.md, 2026-09-26.
                Some(ShellCommand::new(["am", "start", "-a", "android.settings.ADD_ACCOUNT_SETTINGS", "--esa", "account_types", "com.google"]))
            }
            fn root_state_command(&self) -> Option<ShellCommand> {
                // Assumed su CLI (implementation-dependent). Do not mistake adbd uid=0 for app
                // root. Verified absence on Bliss: docs/DECISION-root-adb.md section 3.
                // which status 1 means no match (toybox toys/other/which.c); other errors stay unknown.
                Some(ShellCommand::new(["sh", "-c", "'which su >/dev/null 2>&1; status=$?; if [ \"$status\" -eq 0 ]; then su -c id; elif [ \"$status\" -eq 1 ]; then echo ome-su-absent; else exit \"$status\"; fi'"]))
            }
            fn parse_root_state(&self, output: &str) -> Option<bool> { parsers::root_state(output) }
            fn input_devices_command(&self) -> Option<ShellCommand> {
                // Documented: AOSP toolbox/getevent.c, -p lists devices and -l prints labels.
                Some(ShellCommand::new(["getevent", "-pl"]))
            }
            fn parse_multitouch(&self, output: &str) -> Option<bool> { parsers::multitouch(output) }
            fn screenshot_command(&self, remote_path: &str) -> Option<ShellCommand> {
                // Documented filename form: cmds/screencap/screencap.cpp (13). PNG capture
                // itself verified by launcher/Invoke-GuestTest.ps1 (exec-out, without filename).
                if !remote_path.starts_with('/') || remote_path.contains(['\0', '\n', '\r']) { return None; }
                Some(ShellCommand::new(["screencap", "-p", &shell_word(remote_path)]))
            }
        }
    };
}

adapter!(LegacyAdapter, GuestFamily::Legacy, false);
adapter!(ModernAdapter, GuestFamily::Modern, true);
adapter!(CurrentAdapter, GuestFamily::Current, true);
