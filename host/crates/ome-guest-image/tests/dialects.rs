// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use ome_guest_image::family::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dialect")
            .join(name),
    )
    .expect("fixture")
    .lines()
    .filter(|line| !line.starts_with('#'))
    .collect::<Vec<_>>()
    .join("\n")
}

#[derive(Debug, Default)]
struct FakeRunner {
    outputs: HashMap<Vec<String>, ShellOutput>,
    calls: RefCell<Vec<Vec<String>>>,
    roots: Cell<u32>,
    dead: bool,
    refuse_root: bool,
}

fn error() -> RunnerError {
    RunnerError {
        reason: "fake failure".into(),
    }
}

impl ShellRunner for FakeRunner {
    fn shell(&self, command: &ShellCommand) -> Result<ShellOutput, RunnerError> {
        self.calls.borrow_mut().push(command.args.clone());
        if self.dead {
            return Err(error());
        }
        self.outputs.get(&command.args).cloned().ok_or_else(error)
    }
    fn root(&self) -> Result<(), RunnerError> {
        self.roots.set(self.roots.get() + 1);
        if self.dead || self.refuse_root {
            Err(error())
        } else {
            Ok(())
        }
    }
}

impl FakeRunner {
    fn output(&mut self, command: ShellCommand, stdout: &str, exit_code: i32) {
        self.outputs.insert(
            command.args,
            ShellOutput {
                stdout: stdout.into(),
                exit_code,
            },
        );
    }
    fn success(adapter: &dyn FamilyAdapter) -> Self {
        let mut runner = Self::default();
        for (command, name) in [
            (adapter.boot_completed_command(), "boot.txt"),
            (adapter.native_bridge_command(), "bridge.txt"),
            (adapter.packages_command(), "packages.txt"),
            (adapter.foreground_command(), "foreground-current.txt"),
            (adapter.media_volume_get_command(), "volume.txt"),
            (adapter.display_size_query_command(), "size-override.txt"),
            (
                adapter.display_density_query_command(),
                "density-override.txt",
            ),
            (adapter.root_state_command(), "root-enabled.txt"),
            (adapter.input_devices_command(), "input-multi.txt"),
        ] {
            if let Some(command) = command {
                runner.output(command, &fixture(name), 0);
            }
        }
        runner.output(
            adapter
                .screenshot_command(CapabilityProbe::SCREENSHOT_PROBE_PATH)
                .expect("screenshot"),
            "",
            0,
        );
        runner.output(
            ShellCommand::new(["rm", "-f", CapabilityProbe::SCREENSHOT_PROBE_PATH]),
            "",
            0,
        );
        runner
    }
}

#[test]
fn parsers_cover_recorded_and_synthetic_output_for_every_generation() {
    for api in [28, 29, 30, 33, 34, 35, 36, 99] {
        let a = adapter_for(api);
        assert!(a.parse_boot_completed(&fixture("boot.txt")));
        assert!(!a.parse_boot_completed("0"));
        assert_eq!(
            a.parse_native_bridge(&fixture("bridge.txt")).as_deref(),
            Some("libndk_translation.so")
        );
        assert_eq!(a.parse_native_bridge("0"), None);
        let packages = a.parse_packages(&fixture("packages.txt"));
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[1].version_code, Some(4294967296));
        assert_eq!(
            a.parse_packages(&fixture("packages-legacy.txt"))[0].version_code,
            None
        );
        assert_eq!(a.parse_app_label(&fixture("label.txt")), None);
        for name in ["foreground-legacy.txt", "foreground-current.txt"] {
            assert_eq!(
                a.parse_foreground(&fixture(name)).as_deref(),
                Some("com.example.game")
            );
        }
        assert_eq!(a.parse_media_volume(&fixture("volume.txt")), Some(7));
        assert_eq!(
            a.parse_display(&fixture("size.txt"), &fixture("density.txt")),
            Some(DisplayInfo {
                width: 1280,
                height: 800,
                density_dpi: 160
            })
        );
        assert_eq!(
            a.parse_display(
                &fixture("size-override.txt"),
                &fixture("density-override.txt")
            ),
            Some(DisplayInfo {
                width: 1920,
                height: 1080,
                density_dpi: 240
            })
        );
        assert_eq!(a.parse_root_state(&fixture("root-enabled.txt")), Some(true));
        assert_eq!(a.parse_root_state(&fixture("root-absent.txt")), Some(false));
        assert_eq!(a.parse_root_state(&fixture("root-denied.txt")), None);
        assert_eq!(a.parse_multitouch(&fixture("input-multi.txt")), Some(true));
        assert_eq!(a.parse_multitouch(&fixture("input-slot.txt")), Some(true));
        assert_eq!(
            a.parse_multitouch(&fixture("input-single.txt")),
            Some(false)
        );
        for text in [String::new(), fixture("invalid.txt")] {
            assert!(!a.parse_boot_completed(&text));
            assert_eq!(a.parse_native_bridge(&text), None);
            assert!(a.parse_packages(&text).is_empty());
            assert_eq!(a.parse_app_label(&text), None);
            assert_eq!(a.parse_foreground(&text), None);
            assert_eq!(a.parse_media_volume(&text), None);
            assert_eq!(a.parse_display(&text, &text), None);
            assert_eq!(a.parse_root_state(&text), None);
            assert_eq!(a.parse_multitouch(&text), None);
        }
    }
}

#[test]
fn malformed_values_do_not_become_capabilities() {
    let a = ModernAdapter;
    assert!(
        a.parse_packages("package:com.example versionCode:NaN")
            .is_empty()
    );
    assert_eq!(a.parse_media_volume("volume is 16 in range [0..15]"), None);
    assert_eq!(
        a.parse_display(
            "Physical size: 10x10\nOverride size: bad",
            "Physical density: 160"
        ),
        None
    );
    assert_eq!(
        a.parse_display("Physical size: 0x100", "Physical density: 160"),
        None
    );
    assert_eq!(a.parse_root_state("uid=01(root)"), None);
    assert_eq!(
        a.parse_multitouch("error opening QEMU Virtio MultiTouch ABS_MT_SLOT"),
        None
    );
    assert_eq!(
        a.parse_foreground("last topResumedActivity=ActivityRecord{x u0 com.example/.A}"),
        None
    );
}

fn command(actual: Option<ShellCommand>, args: &[&str]) {
    assert_eq!(actual, Some(ShellCommand::new(args.iter().copied())));
}

#[test]
fn every_generation_has_an_exact_command_table() {
    for api in [28, 29, 30, 34, 35, 36, 99] {
        let a = adapter_for(api);
        command(
            a.boot_completed_command(),
            &["getprop", "sys.boot_completed"],
        );
        command(
            a.native_bridge_command(),
            &["getprop", "ro.dalvik.vm.native.bridge"],
        );
        command(
            a.packages_command(),
            &["pm", "list", "packages", "-3", "--show-versioncode"],
        );
        assert_eq!(a.app_label_command("com.example.app"), None);
        command(
            a.foreground_command(),
            &["dumpsys", "activity", "activities"],
        );
        if api <= 29 {
            assert_eq!(a.media_volume_get_command(), None);
            assert_eq!(a.media_volume_set_command(7), None);
        } else {
            command(
                a.media_volume_get_command(),
                &["cmd", "media_session", "volume", "--stream", "3", "--get"],
            );
            command(
                a.media_volume_set_command(7),
                &[
                    "cmd",
                    "media_session",
                    "volume",
                    "--stream",
                    "3",
                    "--set",
                    "7",
                ],
            );
        }
        assert_eq!(a.media_volume_set_command(16), None);
        command(
            a.display_size_command(1920, 1080),
            &["wm", "size", "1920x1080"],
        );
        command(a.display_size_reset_command(), &["wm", "size", "reset"]);
        command(a.display_density_command(240), &["wm", "density", "240"]);
        command(a.display_size_query_command(), &["wm", "size"]);
        command(a.display_density_query_command(), &["wm", "density"]);
        assert_eq!(a.display_size_command(0, 1080), None);
        assert_eq!(a.display_density_command(71), None);
        command(
            a.root_state_command(),
            &[
                "sh",
                "-c",
                "'which su >/dev/null 2>&1; status=$?; if [ \"$status\" -eq 0 ]; then su -c id; elif [ \"$status\" -eq 1 ]; then echo ome-su-absent; else exit \"$status\"; fi'",
            ],
        );
        command(a.input_devices_command(), &["getevent", "-pl"]);
        command(
            a.screenshot_command("/data/local/tmp/a.png"),
            &["screencap", "-p", "/data/local/tmp/a.png"],
        );
        command(
            a.screenshot_command("/data/local/tmp/a'; echo bad.png"),
            &["screencap", "-p", "'/data/local/tmp/a'\\''; echo bad.png'"],
        );
        assert_eq!(a.screenshot_command("-h"), None);
        assert_eq!(a.screenshot_command("/tmp/a\0.png"), None);
    }
}

fn state(result: &ProbeOutcome, item: ProbeItem) -> ProbeState {
    result
        .items
        .iter()
        .find(|(key, _)| *key == item)
        .expect("item")
        .1
}

#[test]
fn successful_probe_fills_all_fields_and_observes_order() {
    for api in [33, 35] {
        let a = adapter_for(api);
        let runner = FakeRunner::success(a.as_ref());
        let result = CapabilityProbe.run(&runner, a.as_ref());
        assert_eq!(
            result.items,
            ProbeItem::ALL.map(|item| (item, ProbeState::Available))
        );
        assert_eq!(
            result.native_bridge.as_deref(),
            Some("libndk_translation.so")
        );
        assert_eq!(result.media_volume, Some(7));
        assert_eq!(result.foreground.as_deref(), Some("com.example.game"));
        assert_eq!(result.root_enabled, Some(true));
        assert_eq!(
            result.display,
            Some(DisplayInfo {
                width: 1920,
                height: 1080,
                density_dpi: 240
            })
        );
        assert_eq!(result.packages.len(), 2);
        assert_eq!(runner.roots.get(), 0);
        let expected = [
            a.boot_completed_command(),
            a.packages_command(),
            a.display_size_query_command(),
            a.display_density_query_command(),
            a.media_volume_get_command(),
            a.screenshot_command(CapabilityProbe::SCREENSHOT_PROBE_PATH),
            Some(ShellCommand::new([
                "rm",
                "-f",
                CapabilityProbe::SCREENSHOT_PROBE_PATH,
            ])),
            a.foreground_command(),
            a.input_devices_command(),
            a.native_bridge_command(),
            a.root_state_command(),
        ];
        assert_eq!(
            *runner.calls.borrow(),
            expected
                .into_iter()
                .map(|command| command.expect("command").args)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn dead_runner_leaves_every_supported_item_unknown() {
    let runner = FakeRunner {
        dead: true,
        ..FakeRunner::default()
    };
    let result = CapabilityProbe.run(&runner, &ModernAdapter);
    assert!(
        result
            .items
            .iter()
            .all(|(_, state)| *state == ProbeState::Unknown)
    );
    assert_eq!(runner.roots.get(), 0);
    assert!(result.packages.is_empty());
}

#[test]
fn empty_failure_is_unknown_and_screenshot_cleanup_is_unconditional() {
    let mut runner = FakeRunner::success(&ModernAdapter);
    for command in [
        ModernAdapter.boot_completed_command(),
        ModernAdapter.packages_command(),
        ModernAdapter.screenshot_command(CapabilityProbe::SCREENSHOT_PROBE_PATH),
    ] {
        runner.output(command.expect("command"), "", 1);
    }
    let result = CapabilityProbe.run(&runner, &ModernAdapter);
    for item in [
        ProbeItem::BootMarker,
        ProbeItem::AppList,
        ProbeItem::Screenshot,
    ] {
        assert_eq!(state(&result, item), ProbeState::Unknown);
    }
    assert!(
        runner.calls.borrow().contains(
            &ShellCommand::new(["rm", "-f", CapabilityProbe::SCREENSHOT_PROBE_PATH]).args
        )
    );
}

#[test]
fn actual_absence_differs_from_unparseable_output() {
    let mut runner = FakeRunner::success(&ModernAdapter);
    runner.output(
        ModernAdapter.native_bridge_command().expect("bridge"),
        "0",
        0,
    );
    runner.output(
        ModernAdapter.root_state_command().expect("root"),
        &fixture("root-absent.txt"),
        0,
    );
    runner.output(
        ModernAdapter.input_devices_command().expect("input"),
        &fixture("input-single.txt"),
        0,
    );
    runner.output(ModernAdapter.packages_command().expect("packages"), "", 0);
    runner.output(
        ModernAdapter.media_volume_get_command().expect("volume"),
        "malformed",
        0,
    );
    let result = CapabilityProbe.run(&runner, &ModernAdapter);
    for item in [
        ProbeItem::NativeBridge,
        ProbeItem::Root,
        ProbeItem::Multitouch,
    ] {
        assert_eq!(state(&result, item), ProbeState::Unavailable);
    }
    assert_eq!(state(&result, ProbeItem::AppList), ProbeState::Available);
    assert_eq!(state(&result, ProbeItem::MediaVolume), ProbeState::Unknown);
    assert_eq!(result.root_enabled, Some(false));
}

#[test]
fn legacy_missing_volume_command_is_unavailable() {
    let runner = FakeRunner::success(&LegacyAdapter);
    let result = CapabilityProbe.run(&runner, &LegacyAdapter);
    assert_eq!(
        state(&result, ProbeItem::MediaVolume),
        ProbeState::Unavailable
    );
    assert_eq!(state(&result, ProbeItem::AppList), ProbeState::Available);
}

// Test-only dialect to exercise trait contracts absent from the bundled adapters.
#[derive(Debug)]
struct ProbeDialect {
    screenshot: bool,
}
impl FamilyAdapter for ProbeDialect {
    fn family(&self) -> GuestFamily {
        ModernAdapter.family()
    }
    fn boot_completed_command(&self) -> Option<ShellCommand> {
        ModernAdapter.boot_completed_command()
    }
    fn parse_boot_completed(&self, output: &str) -> bool {
        ModernAdapter.parse_boot_completed(output)
    }
    fn native_bridge_command(&self) -> Option<ShellCommand> {
        ModernAdapter.native_bridge_command()
    }
    fn parse_native_bridge(&self, output: &str) -> Option<String> {
        ModernAdapter.parse_native_bridge(output)
    }
    fn packages_command(&self) -> Option<ShellCommand> {
        ModernAdapter.packages_command()
    }
    fn parse_packages(&self, output: &str) -> Vec<PackageEntry> {
        ModernAdapter.parse_packages(output)
    }
    fn app_label_command(&self, package: &str) -> Option<ShellCommand> {
        ModernAdapter.app_label_command(package)
    }
    fn parse_app_label(&self, output: &str) -> Option<String> {
        ModernAdapter.parse_app_label(output)
    }
    fn foreground_command(&self) -> Option<ShellCommand> {
        ModernAdapter.foreground_command()
    }
    fn parse_foreground(&self, output: &str) -> Option<String> {
        ModernAdapter.parse_foreground(output)
    }
    fn media_volume_set_command(&self, index: u32) -> Option<ShellCommand> {
        ModernAdapter.media_volume_set_command(index)
    }
    fn media_volume_get_command(&self) -> Option<ShellCommand> {
        ModernAdapter.media_volume_get_command()
    }
    fn parse_media_volume(&self, output: &str) -> Option<u32> {
        ModernAdapter.parse_media_volume(output)
    }
    fn display_size_command(&self, width: u32, height: u32) -> Option<ShellCommand> {
        ModernAdapter.display_size_command(width, height)
    }
    fn display_size_reset_command(&self) -> Option<ShellCommand> {
        ModernAdapter.display_size_reset_command()
    }
    fn display_density_command(&self, density_dpi: u32) -> Option<ShellCommand> {
        ModernAdapter.display_density_command(density_dpi)
    }
    fn display_size_query_command(&self) -> Option<ShellCommand> {
        ModernAdapter.display_size_query_command()
    }
    fn display_density_query_command(&self) -> Option<ShellCommand> {
        ModernAdapter.display_density_query_command()
    }
    fn parse_display(&self, size_output: &str, density_output: &str) -> Option<DisplayInfo> {
        ModernAdapter.parse_display(size_output, density_output)
    }
    fn root_state_command(&self) -> Option<ShellCommand> {
        ModernAdapter.root_state_command()
    }
    fn parse_root_state(&self, output: &str) -> Option<bool> {
        ModernAdapter.parse_root_state(output)
    }
    fn input_devices_command(&self) -> Option<ShellCommand> {
        ModernAdapter.input_devices_command()
    }
    fn parse_multitouch(&self, output: &str) -> Option<bool> {
        ModernAdapter.parse_multitouch(output)
    }
    fn screenshot_command(&self, path: &str) -> Option<ShellCommand> {
        if self.screenshot {
            ModernAdapter.screenshot_command(path)
        } else {
            None
        }
    }
}

#[test]
fn missing_capture_does_not_request_root_and_still_cleans_up() {
    let a = ProbeDialect { screenshot: false };
    let runner = FakeRunner::success(&ModernAdapter);
    let result = CapabilityProbe.run(&runner, &a);
    assert_eq!(
        state(&result, ProbeItem::Screenshot),
        ProbeState::Unavailable
    );
    assert_eq!(runner.roots.get(), 0);
    assert!(
        runner.calls.borrow().contains(
            &ShellCommand::new(["rm", "-f", CapabilityProbe::SCREENSHOT_PROBE_PATH]).args
        )
    );
}

#[test]
fn capture_transport_failure_still_cleans_up_and_nonempty_failure_is_unavailable() {
    let a = ModernAdapter;
    let command = a
        .screenshot_command(CapabilityProbe::SCREENSHOT_PROBE_PATH)
        .expect("capture");
    let mut runner = FakeRunner::success(&a);
    runner.outputs.remove(&command.args);
    let result = CapabilityProbe.run(&runner, &a);
    assert_eq!(state(&result, ProbeItem::Screenshot), ProbeState::Unknown);
    assert!(
        runner.calls.borrow().contains(
            &ShellCommand::new(["rm", "-f", CapabilityProbe::SCREENSHOT_PROBE_PATH]).args
        )
    );
    runner.output(command, "Permission denied", 1);
    assert_eq!(
        state(&CapabilityProbe.run(&runner, &a), ProbeItem::Screenshot),
        ProbeState::Unavailable
    );
}
