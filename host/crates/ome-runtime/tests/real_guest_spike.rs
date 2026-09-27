// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Manual product-runtime spike against the adopted Android guest.
#![forbid(unsafe_code)]
#![cfg(windows)]

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use ome_adb::{AdbSession, ProcessRunner};
use ome_host_check::HostProbe;
use ome_platform_win::{
    TestHostWindow, WindowHandle, find_windows_of_process, set_thread_dpi_hosting_mixed,
};
use ome_runtime::{
    AdbPowerOff, AppRuntime, Capability, CapabilityId, Command, Desktop, GuestState, HostingMode,
    OmeHome, RuntimeDeps, StageRect, WindowsProbe,
};
use ome_supervisor::windows_adapter::WindowsProcessAdapter;
use ome_supervisor::{GuestEvent, Supervisor, SupervisorPolicy, TcpQmpFactory};
use ome_window_host::GuestWindowHost;

const GUEST_ID: &str = "default";
const BOOT_DEADLINE: Duration = Duration::from_secs(180);
const STOP_OBSERVATION_DEADLINE: Duration = Duration::from_secs(40);
const PROCESS_EXIT_DEADLINE: Duration = Duration::from_secs(5);
const CAPABILITY_IDS: [CapabilityId; 10] = [
    CapabilityId::BootMarker,
    CapabilityId::AppList,
    CapabilityId::DisplaySize,
    CapabilityId::MediaVolume,
    CapabilityId::DeviceId,
    CapabilityId::Screenshot,
    CapabilityId::ForegroundApp,
    CapabilityId::Multitouch,
    CapabilityId::NativeBridge,
    CapabilityId::Root,
];

#[derive(Debug, Default)]
struct ClosedDesktop;

impl Desktop for ClosedDesktop {
    fn open_path(&self, _path: &Path) -> Result<(), String> {
        Err("desktop opening is disabled in the real-guest spike".to_owned())
    }

    fn open_url(&self, _url: &str) -> Result<(), String> {
        Err("URL opening is disabled in the real-guest spike".to_owned())
    }

    fn copy_text(&self, _text: &str) -> Result<(), String> {
        Err("clipboard writes are disabled in the real-guest spike".to_owned())
    }
}

#[derive(Debug)]
struct Measurements {
    file: File,
}

impl Measurements {
    fn create(path: &Path) -> Self {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create window-hosting evidence directory");
        }
        let file = File::create(path).expect("create measurements file");
        Self { file }
    }

    fn record(&mut self, key: &str, value: impl std::fmt::Display) {
        let value = sanitize(value.to_string());
        println!("{key}={value}");
        writeln!(self.file, "{key}={value}").expect("write measurement");
        self.file.flush().expect("flush measurement");
    }

    fn failure(&mut self, run: usize, step: &str, value: impl std::fmt::Display) {
        self.record(&format!("run{run}.failure.{step}"), value);
    }
}

#[derive(Clone, Copy, Debug)]
struct RunClock {
    start: Instant,
    running: Option<Instant>,
}

impl RunClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            running: None,
        }
    }

    fn elapsed_ms(self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

#[test]
#[ignore = "requires an interactive Windows desktop, WHPX, QEMU, adb, and the adopted default guest"]
fn measures_real_guest_through_product_runtime() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let evidence = repository.join("docs/evidence/M2/window-hosting");
    let mut measurements = Measurements::create(&evidence.join("measurements.txt"));
    measurements.record("spike.started_unix_ms", unix_millis());

    let initial_processes = qemu_processes();
    measurements.record("preflight.qemu_process_count", initial_processes.len());
    measurements.record("preflight.qemu_processes", initial_processes.join(" | "));
    if !initial_processes.is_empty() {
        measurements.record(
            "spike.skipped",
            "QEMU was already running; the spike did not stop or replace it",
        );
        panic!("QEMU was already running before the real-guest spike");
    }

    let home = OmeHome::resolved().expect("resolve product home");
    let manifest_root = repository.join("manifests");
    let probe = WindowsProbe;
    let qemu = probe.qemu().expect("probe QEMU");
    if let Some(qemu) = qemu.as_ref() {
        measurements.record("environment.qemu_version", &qemu.version);
        measurements.record("environment.qemu_source", &qemu.source);
        measurements.record("environment.qemu_program", &qemu.program);
    } else {
        measurements.record("environment.qemu_version", "not found");
    }
    measurements.record("environment.guest_id", GUEST_ID);
    measurements.record(
        "environment.guest_disk",
        home.as_path().join("vm/default/disk.qcow2").display(),
    );

    let adb = probe.adb().expect("probe adb").and_then(|found| {
        measurements.record("environment.adb_version", &found.version);
        measurements.record("environment.adb_program", &found.program);
        AdbSession::new(
            found.program,
            "127.0.0.1:5555".to_owned(),
            Box::new(ProcessRunner),
        )
        .ok()
    });
    let logs = home.subdir("logs").expect("resolve product log directory");
    let supervisor_policy = adb
        .as_ref()
        .map_or_else(SupervisorPolicy::default, |session| SupervisorPolicy {
            power_off_hook: Arc::new(AdbPowerOff::new(session.clone())),
            ..SupervisorPolicy::default()
        });
    let supervisor = Supervisor::new(
        WindowsProcessAdapter,
        TcpQmpFactory,
        logs,
        supervisor_policy,
    );
    let mut runtime = AppRuntime::open(
        home.clone(),
        RuntimeDeps {
            probe: Box::new(probe),
            artifacts: None,
            adb,
            supervisor: Some(Box::new(supervisor)),
            desktop: Box::new(ClosedDesktop),
            window_host: Box::new(GuestWindowHost::default()),
            family_adapter: None,
            images_dir: Some(manifest_root.join("images")),
            artifacts_manifest: Some(manifest_root.join("artifacts.json")),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )
    .expect("open product runtime");

    if !runtime
        .snapshot()
        .images
        .guests
        .iter()
        .any(|guest| guest.name == GUEST_ID)
    {
        measurements.record(
            "spike.skipped",
            "snapshot.images.guests does not list the adopted default guest",
        );
        return;
    }

    runtime
        .apply(Command::GuestSelect {
            id: GUEST_ID.to_owned(),
        })
        .expect("select adopted default guest");
    let selected = runtime.snapshot();
    measurements.record(
        "environment.image_id",
        selected.guest.image_id.as_deref().unwrap_or("unknown"),
    );
    measurements.record(
        "environment.android_version",
        selected
            .guest
            .android_version
            .as_deref()
            .unwrap_or("unknown"),
    );
    measurements.record(
        "environment.api_level",
        selected
            .guest
            .api_level
            .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
    );

    let _dpi_guard = set_thread_dpi_hosting_mixed().expect("set mixed DPI hosting on test thread");
    let parent = TestHostWindow::create("OME real-guest hosting spike", 1280, 800)
        .expect("create test parent window");
    let parent_dpi = parent.handle().dpi().expect("read test parent DPI");
    let parent_size = parent.client_size().expect("read test parent client size");
    measurements.record("environment.host_dpi", parent_dpi);
    measurements.record(
        "environment.host_scale_factor",
        format_args!("{:.4}", f64::from(parent_dpi) / 96.0),
    );
    measurements.record("environment.parent_client_initial", dimensions(parent_size));
    runtime.set_host_window(parent.handle().as_u64());
    let receiver = runtime
        .subscribe_guest_events()
        .expect("subscribe to real supervisor events");

    let mut panics = Vec::new();
    for run in 1..=2 {
        if run == 2 {
            if let Err(error) = parent.resize(1280, 800) {
                measurements.failure(run, "parent_reset", format!("{error:?}"));
            }
            thread::sleep(Duration::from_secs(1));
        }
        let cycle = catch_unwind(AssertUnwindSafe(|| {
            measure_one_run(
                run,
                &mut runtime,
                &receiver,
                &parent,
                &home,
                &evidence,
                &mut measurements,
            );
        }));
        if let Err(payload) = cycle {
            let message = panic_message(payload);
            measurements.failure(run, "panic", &message);
            panics.push(format!("run {run}: {message}"));
        }
        stop_through_runtime(run, &mut runtime, &receiver, &mut measurements);
        wait_for_no_qemu(run, &mut measurements);
    }

    drop(parent);
    let final_processes = qemu_processes();
    measurements.record("final.qemu_process_count", final_processes.len());
    measurements.record("final.qemu_processes", final_processes.join(" | "));
    measurements.record("spike.finished_unix_ms", unix_millis());
    assert!(
        final_processes.is_empty(),
        "QEMU remains after runtime stop: {final_processes:?}"
    );
    assert!(
        panics.is_empty(),
        "spike panicked after cleanup: {panics:?}"
    );
}

fn measure_one_run(
    run: usize,
    runtime: &mut AppRuntime,
    receiver: &Receiver<GuestEvent>,
    parent: &TestHostWindow,
    home: &OmeHome,
    evidence: &Path,
    measurements: &mut Measurements,
) {
    let before_logs = qemu_log_files(home);
    let mut clock = RunClock::new();
    let mut last_state = runtime.snapshot().guest.state;
    measurements.record(
        &format!("run{run}.state.0"),
        format!("0ms:{}", state_name(last_state)),
    );
    match runtime.apply(Command::GuestStart) {
        Ok(_) => measurements.record(&format!("run{run}.start_admitted"), true),
        Err(error) => {
            measurements.failure(run, "guest_start", format!("{error:?}"));
            return;
        }
    }

    let mut hosting_measured = false;
    let mut boot_measured = false;
    let mut transition = 1_u32;
    while clock.start.elapsed() < BOOT_DEADLINE {
        pump_once(runtime, receiver, run, measurements);
        let snapshot = runtime.snapshot();
        if snapshot.guest.state != last_state {
            measurements.record(
                &format!("run{run}.state.{transition}"),
                format!(
                    "{}ms:{}",
                    clock.elapsed_ms(),
                    state_name(snapshot.guest.state)
                ),
            );
            transition += 1;
            last_state = snapshot.guest.state;
        }
        if snapshot.guest.state == GuestState::Running && clock.running.is_none() {
            clock.running = Some(Instant::now());
            measurements.record(&format!("run{run}.start_to_running_ms"), clock.elapsed_ms());
        }
        if snapshot.guest.state == GuestState::Running
            && snapshot.guest.pid.is_some()
            && !hosting_measured
        {
            measure_hosting(run, runtime, parent, clock, measurements);
            hosting_measured = true;
        }
        let after_hosting = runtime.snapshot();
        if after_hosting.guest.boot_completed && !boot_measured {
            measurements.record(
                &format!("run{run}.start_to_boot_completed_ms"),
                clock.elapsed_ms(),
            );
            if let Some(running) = clock.running {
                measurements.record(
                    &format!("run{run}.running_to_boot_completed_ms"),
                    running.elapsed().as_millis(),
                );
            }
            record_probe(run, &after_hosting, home, clock, measurements);
            save_screenshot(run, runtime, home, evidence, measurements);
            measurements.record(
                &format!("run{run}.host_capture"),
                "skipped: ome-platform-win has no parent-region capture API",
            );
            boot_measured = true;
            break;
        }
        if matches!(after_hosting.guest.state, GuestState::Failed) {
            measurements.failure(
                run,
                "boot_terminal_state",
                state_name(after_hosting.guest.state),
            );
            break;
        }
    }
    if !boot_measured {
        let reason = if clock.start.elapsed() >= BOOT_DEADLINE {
            format!("no bootCompleted within {}s", BOOT_DEADLINE.as_secs())
        } else {
            format!(
                "no bootCompleted before terminal state {}",
                state_name(runtime.snapshot().guest.state)
            )
        };
        measurements.failure(run, "boot_incomplete", reason);
        let snapshot = runtime.snapshot();
        record_probe(run, &snapshot, home, clock, measurements);
    }
    if !hosting_measured {
        measurements.failure(run, "hosting", "running state with pid was not observed");
    }

    let after_logs = qemu_log_files(home);
    let new_logs = after_logs
        .difference(&before_logs)
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" | ");
    measurements.record(&format!("run{run}.new_log_files_before_stop"), new_logs);
}

fn measure_hosting(
    run: usize,
    runtime: &mut AppRuntime,
    parent: &TestHostWindow,
    clock: RunClock,
    measurements: &mut Measurements,
) {
    let snapshot = runtime.snapshot();
    let Some(pid) = snapshot.guest.pid else {
        measurements.failure(run, "hosting_pid", "running snapshot had no pid");
        return;
    };
    measurements.record(&format!("run{run}.qemu_pid"), pid);
    let guest = discover_qemu_window(pid, Duration::from_secs(5));
    if guest.is_none() {
        measurements.failure(run, "window_discovery", "no visible QEMU window within 5s");
    }
    let guest_dpi_before = guest.and_then(|window| window.dpi().ok());
    let guest_class = guest.and_then(|window| window.class_name().ok());
    measurements.record(
        &format!("run{run}.guest_window_class"),
        guest_class.as_deref().unwrap_or("unknown"),
    );
    measurements.record(
        &format!("run{run}.guest_dpi_before_attach"),
        optional(guest_dpi_before),
    );

    let initial_size = parent
        .client_size()
        .expect("read parent size before attach");
    let parent_dpi = parent
        .handle()
        .dpi()
        .expect("read parent DPI before attach");
    let scale = f64::from(parent_dpi) / 96.0;
    let stage = stage_for_client(initial_size, scale);
    let attach_started = Instant::now();
    match runtime.apply(Command::StageRectChanged { rect: stage }) {
        Ok(after) => {
            measurements.record(
                &format!("run{run}.hosting"),
                hosting_name(after.guest.hosting),
            );
            measurements.record(
                &format!("run{run}.stage_apply_ms"),
                attach_started.elapsed().as_millis(),
            );
            if let Some(running) = clock.running {
                measurements.record(
                    &format!("run{run}.running_to_hosting_ms"),
                    running.elapsed().as_millis(),
                );
            }
        }
        Err(error) => measurements.failure(run, "stage_initial", format!("{error:?}")),
    }
    thread::sleep(Duration::from_millis(100));
    measurements.record(
        &format!("run{run}.parent_dpi_after_attach"),
        optional(parent.handle().dpi().ok()),
    );
    measurements.record(
        &format!("run{run}.guest_dpi_after_attach"),
        optional(guest.and_then(|window| window.dpi().ok())),
    );
    let initial_guest_size = guest.and_then(|window| window.client_size().ok());
    measurements.record(
        &format!("run{run}.parent_client_attached"),
        dimensions(initial_size),
    );
    measurements.record(
        &format!("run{run}.guest_client_attached"),
        optional_dimensions(initial_guest_size),
    );
    measurements.record(
        &format!("run{run}.initial_size_matches"),
        initial_guest_size == Some(initial_size),
    );

    if let Err(error) = parent.resize(960, 600) {
        measurements.failure(run, "parent_resize", format!("{error:?}"));
        return;
    }
    let resized_parent = parent.client_size().expect("read resized parent size");
    let resized_dpi = parent.handle().dpi().expect("read resized parent DPI");
    match runtime.apply(Command::StageRectChanged {
        rect: stage_for_client(resized_parent, f64::from(resized_dpi) / 96.0),
    }) {
        Ok(after) => measurements.record(
            &format!("run{run}.hosting_after_resize"),
            hosting_name(after.guest.hosting),
        ),
        Err(error) => measurements.failure(run, "stage_resize", format!("{error:?}")),
    }
    let resized_guest = guest.and_then(|window| wait_for_client_size(window, resized_parent));
    measurements.record(
        &format!("run{run}.parent_client_resized"),
        dimensions(resized_parent),
    );
    measurements.record(
        &format!("run{run}.guest_client_resized"),
        optional_dimensions(resized_guest),
    );
    measurements.record(
        &format!("run{run}.resized_size_matches"),
        resized_guest == Some(resized_parent),
    );

    match runtime.apply(Command::GuestWindowToFront) {
        Ok(_) => measurements.record(&format!("run{run}.focus_command"), "ok"),
        Err(error) => measurements.failure(run, "focus_command", format!("{error:?}")),
    }
    thread::sleep(Duration::from_millis(100));
    match guest.and_then(|window| window.has_keyboard_focus().ok()) {
        Some(value) => measurements.record(&format!("run{run}.guest_has_keyboard_focus"), value),
        None => measurements.failure(run, "focus_query", "guest HWND was unavailable"),
    }
}

fn record_probe(
    run: usize,
    snapshot: &ome_runtime::AppSnapshot,
    home: &OmeHome,
    clock: RunClock,
    measurements: &mut Measurements,
) {
    measurements.record(
        &format!("run{run}.first_capabilities_report_ms"),
        clock.elapsed_ms(),
    );
    measurements.record(
        &format!("run{run}.capabilities.probed_at"),
        snapshot
            .guest
            .capabilities
            .probed_at
            .as_deref()
            .unwrap_or("unknown"),
    );
    measurements.record(
        &format!("run{run}.boot_completed"),
        snapshot.guest.boot_completed,
    );
    measurements.record(
        &format!("run{run}.adb_connected"),
        snapshot.guest.adb_connected,
    );
    measurements.record(
        &format!("run{run}.device_id"),
        snapshot.guest.device_id.as_deref().unwrap_or("unknown"),
    );
    measurements.record(
        &format!("run{run}.device_id_decimal"),
        snapshot
            .guest
            .device_id_decimal
            .as_deref()
            .unwrap_or("unknown"),
    );
    measurements.record(
        &format!("run{run}.device_id_present"),
        snapshot.guest.device_id.is_some() && snapshot.guest.device_id_decimal.is_some(),
    );
    measurements.record(
        &format!("run{run}.google_accounts"),
        optional(snapshot.guest.google_accounts),
    );
    measurements.record(
        &format!("run{run}.media_volume"),
        optional(snapshot.guest.media_volume),
    );
    measurements.record(
        &format!("run{run}.root_enabled"),
        optional(snapshot.guest.root_enabled),
    );
    measurements.record(
        &format!("run{run}.resolution"),
        snapshot.guest.resolution.map_or_else(
            || "unknown".to_owned(),
            |size| format!("{}x{}", size.width, size.height),
        ),
    );

    let metadata = read_guest_metadata(home);
    for capability in CAPABILITY_IDS {
        let state = snapshot
            .guest
            .capabilities
            .items
            .iter()
            .find(|item| item.id == capability)
            .map_or(Capability::Unknown, |item| item.state);
        measurements.record(
            &format!("run{run}.capability.{}.state", capability_key(capability)),
            capability_name(state),
        );
        measurements.record(
            &format!("run{run}.capability.{}.value", capability_key(capability)),
            capability_value(capability, snapshot, metadata.as_ref()),
        );
    }
    let unknown = CAPABILITY_IDS
        .into_iter()
        .filter(|capability| {
            snapshot
                .guest
                .capabilities
                .items
                .iter()
                .find(|item| item.id == *capability)
                .is_none_or(|item| item.state == Capability::Unknown)
        })
        .map(capability_key)
        .collect::<Vec<_>>()
        .join(",");
    measurements.record(&format!("run{run}.capabilities.unknown"), unknown);
}

fn capability_value(
    id: CapabilityId,
    snapshot: &ome_runtime::AppSnapshot,
    metadata: Option<&serde_json::Value>,
) -> String {
    let capabilities = metadata.and_then(|value| value.get("capabilities"));
    match id {
        CapabilityId::BootMarker => format!("bootCompleted={}", snapshot.guest.boot_completed),
        CapabilityId::AppList => capabilities
            .and_then(|value| value.get("packages"))
            .and_then(serde_json::Value::as_array)
            .map_or_else(
                || "packageCount=unknown".to_owned(),
                |items| format!("packageCount={}", items.len()),
            ),
        CapabilityId::DisplaySize => capabilities
            .and_then(|value| value.get("display"))
            .map_or_else(|| "unknown".to_owned(), compact_json),
        CapabilityId::MediaVolume => optional(snapshot.guest.media_volume),
        CapabilityId::DeviceId => snapshot
            .guest
            .device_id
            .as_deref()
            .unwrap_or("unknown")
            .to_owned(),
        CapabilityId::Screenshot => "temporary guest screencap probe".to_owned(),
        CapabilityId::ForegroundApp => capabilities
            .and_then(|value| value.get("foregroundPackage"))
            .map_or_else(|| "unknown".to_owned(), compact_json),
        CapabilityId::Multitouch => "input-device presence".to_owned(),
        CapabilityId::NativeBridge => capabilities
            .and_then(|value| value.get("nativeBridge"))
            .map_or_else(|| "unknown".to_owned(), compact_json),
        CapabilityId::Root => optional(snapshot.guest.root_enabled),
    }
}

fn save_screenshot(
    run: usize,
    runtime: &mut AppRuntime,
    home: &OmeHome,
    evidence: &Path,
    measurements: &mut Measurements,
) {
    let directory = home.as_path().join("screenshots");
    let before = png_files(&directory);
    match runtime.apply(Command::ScreenshotSave) {
        Ok(_) => {
            let after = png_files(&directory);
            let created = after
                .difference(&before)
                .next()
                .cloned()
                .or_else(|| newest_png(&directory));
            if let Some(source) = created {
                let destination = evidence.join(format!("run-{run}-adb.png"));
                match fs::copy(&source, &destination) {
                    Ok(_) => {
                        measurements.record(
                            &format!("run{run}.screenshot_product_path"),
                            source.display(),
                        );
                        measurements.record(
                            &format!("run{run}.screenshot_evidence_path"),
                            destination.display(),
                        );
                    }
                    Err(error) => measurements.failure(
                        run,
                        "screenshot_copy",
                        format!("{error}: {}", source.display()),
                    ),
                }
            } else {
                measurements.failure(
                    run,
                    "screenshot_path",
                    "runtime created no discoverable PNG",
                );
            }
        }
        Err(error) => measurements.failure(run, "screenshot_save", format!("{error:?}")),
    }
}

fn stop_through_runtime(
    run: usize,
    runtime: &mut AppRuntime,
    receiver: &Receiver<GuestEvent>,
    measurements: &mut Measurements,
) {
    let before = runtime.snapshot();
    if matches!(before.guest.state, GuestState::Stopped) {
        measurements.record(&format!("run{run}.stop_duration_ms"), 0);
        measurements.record(&format!("run{run}.last_exit"), "none: already stopped");
        return;
    }
    let started = Instant::now();
    match runtime.apply(Command::GuestStop) {
        Ok(_) => measurements.record(&format!("run{run}.stop_requested"), true),
        Err(error) => measurements.failure(run, "guest_stop", format!("{error:?}")),
    }
    let mut last_state = runtime.snapshot().guest.state;
    let mut transition = 100_u32;
    while started.elapsed() < STOP_OBSERVATION_DEADLINE {
        pump_once(runtime, receiver, run, measurements);
        let snapshot = runtime.snapshot();
        if snapshot.guest.state != last_state {
            measurements.record(
                &format!("run{run}.state.{transition}"),
                format!(
                    "stop+{}ms:{}",
                    started.elapsed().as_millis(),
                    state_name(snapshot.guest.state)
                ),
            );
            transition += 1;
            last_state = snapshot.guest.state;
        }
        if matches!(
            snapshot.guest.state,
            GuestState::Stopped | GuestState::Failed
        ) {
            break;
        }
    }
    let after = runtime.snapshot();
    measurements.record(
        &format!("run{run}.stop_duration_ms"),
        started.elapsed().as_millis(),
    );
    measurements.record(
        &format!("run{run}.stop_final_state"),
        state_name(after.guest.state),
    );
    if let Some(exit) = after.guest.last_exit {
        measurements.record(
            &format!("run{run}.last_exit"),
            format!(
                "kind={:?};at={};logPath={}",
                exit.kind,
                exit.at,
                exit.log_path.as_deref().unwrap_or("none")
            ),
        );
    } else {
        measurements.record(&format!("run{run}.last_exit"), "none");
    }
}

fn pump_once(
    runtime: &mut AppRuntime,
    receiver: &Receiver<GuestEvent>,
    run: usize,
    measurements: &mut Measurements,
) {
    match receiver.recv_timeout(Duration::from_secs(1)) {
        Ok(event) => runtime.ingest_guest_event(event),
        Err(mpsc::RecvTimeoutError::Timeout) => runtime.tick(),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            measurements.failure(
                run,
                "event_receiver",
                "supervisor event channel disconnected",
            );
            runtime.tick();
        }
    }
}

fn wait_for_no_qemu(run: usize, measurements: &mut Measurements) {
    let started = Instant::now();
    loop {
        let processes = qemu_processes();
        if processes.is_empty() || started.elapsed() >= PROCESS_EXIT_DEADLINE {
            measurements.record(
                &format!("run{run}.qemu_processes_after_stop"),
                processes.join(" | "),
            );
            measurements.record(
                &format!("run{run}.qemu_absent_after_stop"),
                processes.is_empty(),
            );
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn discover_qemu_window(pid: u32, timeout: Duration) -> Option<WindowHandle> {
    let deadline = Instant::now() + timeout;
    loop {
        let windows = find_windows_of_process(pid).ok()?;
        let visible = windows
            .iter()
            .copied()
            .filter(|window| window.is_visible().unwrap_or(false))
            .collect::<Vec<_>>();
        if let Some(window) = visible
            .iter()
            .copied()
            .find(|window| window.class_name().ok().as_deref() == Some("SDL_app"))
            .or_else(|| visible.first().copied())
        {
            return Some(window);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_client_size(window: WindowHandle, expected: (i32, i32)) -> Option<(i32, i32)> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let observed = window.client_size().ok();
        if observed == Some(expected) || Instant::now() >= deadline {
            return observed;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn stage_for_client(size: (i32, i32), scale_factor: f64) -> StageRect {
    StageRect {
        x: 0.0,
        y: 0.0,
        width: f64::from(size.0) / scale_factor,
        height: f64::from(size.1) / scale_factor,
        scale_factor,
    }
}

fn read_guest_metadata(home: &OmeHome) -> Option<serde_json::Value> {
    let path = home.as_path().join("vm/default/guest.json");
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn qemu_log_files(home: &OmeHome) -> BTreeSet<PathBuf> {
    let directory = home.as_path().join("logs");
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("qemu-default-") && name.ends_with(".log"))
        })
        .collect()
}

fn png_files(directory: &Path) -> BTreeSet<PathBuf> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
        .collect()
}

fn newest_png(directory: &Path) -> Option<PathBuf> {
    fs::read_dir(directory)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
        .filter_map(|path| {
            let modified = path.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn qemu_processes() -> Vec<String> {
    let output = match ProcessCommand::new("tasklist.exe").output() {
        Ok(output) => output,
        Err(error) => return vec![format!("tasklist error: {error}")],
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.to_ascii_lowercase().contains("qemu-system"))
        .map(str::trim)
        .map(str::to_owned)
        .collect()
}

fn capability_key(value: CapabilityId) -> &'static str {
    match value {
        CapabilityId::BootMarker => "bootMarker",
        CapabilityId::AppList => "appList",
        CapabilityId::DisplaySize => "displaySize",
        CapabilityId::MediaVolume => "mediaVolume",
        CapabilityId::DeviceId => "deviceId",
        CapabilityId::Screenshot => "screenshot",
        CapabilityId::ForegroundApp => "foregroundApp",
        CapabilityId::Multitouch => "multitouch",
        CapabilityId::NativeBridge => "nativeBridge",
        CapabilityId::Root => "root",
    }
}

fn capability_name(value: Capability) -> &'static str {
    match value {
        Capability::Available => "available",
        Capability::Unavailable => "unavailable",
        Capability::Unknown => "unknown",
    }
}

fn state_name(value: GuestState) -> &'static str {
    match value {
        GuestState::Stopped => "stopped",
        GuestState::Starting => "starting",
        GuestState::Running => "running",
        GuestState::Stopping => "stopping",
        GuestState::Restarting => "restarting",
        GuestState::Failed => "failed",
    }
}

fn hosting_name(value: HostingMode) -> &'static str {
    match value {
        HostingMode::None => "none",
        HostingMode::Embedded => "embedded",
        HostingMode::SeparateWindow => "separateWindow",
    }
}

fn dimensions(size: (i32, i32)) -> String {
    format!("{}x{}", size.0, size.1)
}

fn optional_dimensions(size: Option<(i32, i32)>) -> String {
    size.map_or_else(|| "unknown".to_owned(), dimensions)
}

fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
}

fn compact_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn sanitize(value: String) -> String {
    value.replace('\r', "\\r").replace('\n', "\\n")
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}
