// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Manual check against the adopted Android guest: the QEMU-owned popup (patch 0005) is placed
//! over QMP, stays live, and passes mouse and QMP keyboard input.
#![forbid(unsafe_code)]
#![cfg(windows)]

use std::io::{BufRead as _, BufReader};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ome_adb::{AdbSession, ProcessRunner};
use ome_guest_config::{GuestConfig, GuestPaths, QemuInstall};
use ome_host_check::HostProbe;
use ome_platform_win::{
    TestHostWindow, WindowHandle, click_primary_at, screen_region_lit_share,
    set_process_dpi_awareness_per_monitor_v2, set_thread_dpi_hosting_mixed, window_at,
};
use ome_runtime::{
    AdbPowerOff, AppRuntime, Command, Desktop, GuestProcess, GuestState, OmeHome, RuntimeDeps,
    StageRect, WindowsProbe,
};
use ome_supervisor::windows_adapter::WindowsProcessAdapter;
use ome_supervisor::{
    DisplayWindowGeometry, GuestEvent, InputError, Supervisor, SupervisorPolicy, TcpQmpFactory,
};
use ome_window_host::GuestWindowHost;

const GUEST_ID: &str = "default";
const BOOT_DEADLINE: Duration = Duration::from_secs(180);
const STOP_DEADLINE: Duration = Duration::from_secs(40);
const STARTUP_PLACEMENT_WINDOW: Duration = Duration::from_secs(40);
const STARTUP_TICK_INTERVAL: Duration = Duration::from_millis(200);
/// The first 15 s after `GuestStart` show the firmware and the GRUB menu's photo background.
const LIVENESS_WINDOW: Duration = Duration::from_secs(15);
const LIVENESS_INTERVAL: Duration = Duration::from_millis(500);
/// A live popup over the GRUB photo is mostly lit; a frozen one stays near 0 or constant and low.
const LIVENESS_MINIMUM: f64 = 0.6;
const ADB_ADDRESS: &str = "127.0.0.1:5555";

type SentGeometries = Arc<Mutex<Vec<(Instant, DisplayWindowGeometry)>>>;

/// Delegates to the real supervisor and records every geometry the runtime sends over QMP.
struct RecordingSupervisor {
    inner: Supervisor<WindowsProcessAdapter, TcpQmpFactory>,
    sent: SentGeometries,
}

impl GuestProcess for RecordingSupervisor {
    fn start(
        &mut self,
        config: GuestConfig,
        paths: GuestPaths,
        install: QemuInstall,
    ) -> Result<(), String> {
        GuestProcess::start(&mut self.inner, config, paths, install)
    }

    fn start_install(
        &mut self,
        config: GuestConfig,
        paths: GuestPaths,
        install: QemuInstall,
    ) -> Result<(), String> {
        GuestProcess::start_install(&mut self.inner, config, paths, install)
    }

    fn request_stop(&self) {
        GuestProcess::request_stop(&self.inner);
    }

    fn send_input(&self, events: Vec<serde_json::Value>) -> Result<(), InputError> {
        GuestProcess::send_input(&self.inner, events)
    }

    fn set_display_window(&self, geometry: DisplayWindowGeometry) -> Result<(), InputError> {
        self.sent
            .lock()
            .expect("sent geometries lock")
            .push((Instant::now(), geometry));
        GuestProcess::set_display_window(&self.inner, geometry)
    }

    fn state(&self) -> ome_supervisor::GuestState {
        GuestProcess::state(&self.inner)
    }

    fn subscribe(&self) -> Receiver<GuestEvent> {
        GuestProcess::subscribe(&self.inner)
    }
}

#[derive(Debug, Default)]
struct ClosedDesktop;

impl Desktop for ClosedDesktop {
    fn open_path(&self, _path: &Path) -> Result<(), String> {
        Err("desktop opening is disabled in the keyboard pass-through check".to_owned())
    }

    fn open_url(&self, _url: &str) -> Result<(), String> {
        Err("URL opening is disabled in the keyboard pass-through check".to_owned())
    }

    fn copy_text(&self, _text: &str) -> Result<(), String> {
        Err("clipboard writes are disabled in the keyboard pass-through check".to_owned())
    }
}

struct GetEvent {
    child: Child,
    lines: Receiver<(Instant, String)>,
}

impl GetEvent {
    fn start(adb: &Path) -> Self {
        let mut child = ProcessCommand::new(adb)
            .args(["-s", "127.0.0.1:5555", "shell", "getevent", "-lq"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start adb getevent");
        let stdout = child.stdout.take().expect("capture getevent stdout");
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = sender.send((Instant::now(), line));
            }
        });
        Self { child, lines }
    }

    fn drain(&self) {
        while self.lines.try_recv().is_ok() {}
    }

    fn collect_pointer_click(&self, start: Instant, timeout: Duration) -> Vec<String> {
        let mut found = Vec::new();
        let mut observed = Vec::new();
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(remaining) {
                Ok((seen, line)) => {
                    let line = format!(
                        "+{}ms {line}",
                        seen.saturating_duration_since(start).as_millis()
                    );
                    if line.contains("BTN_TOUCH")
                        || line.contains("ABS_MT_POSITION_X")
                        || line.contains("BTN_MOUSE")
                        || line.contains("BTN_LEFT")
                        || line.contains("ABS_X")
                    {
                        println!("getevent {line}");
                        found.push(line.clone());
                    }
                    if observed.len() < 30 {
                        observed.push(line);
                    }
                    let has_position = found
                        .iter()
                        .any(|line| line.contains("ABS_MT_POSITION_X") || line.contains("ABS_X"));
                    let has_button = found.iter().any(|line| {
                        (line.contains("BTN_TOUCH")
                            || line.contains("BTN_MOUSE")
                            || line.contains("BTN_LEFT"))
                            && line.contains("DOWN")
                    });
                    if has_position && has_button {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        println!("getevent_pointer_observed={observed:?}");
        found
    }

    fn collect_key_a(&self, start: Instant, timeout: Duration) -> Vec<String> {
        let mut found = Vec::new();
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(remaining) {
                Ok((seen, line)) if line.contains("KEY_A") => {
                    let line = format!(
                        "+{}ms {line}",
                        seen.saturating_duration_since(start).as_millis()
                    );
                    println!("getevent {line}");
                    found.push(line);
                    if found.iter().any(|line| line.contains("DOWN"))
                        && found.iter().any(|line| line.contains("UP"))
                    {
                        break;
                    }
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        found
    }
}

impl Drop for GetEvent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "requires an interactive Windows desktop, WHPX, QEMU, adb, and the adopted default guest"]
fn browser_keyboard_and_mouse_reach_unfocused_real_guest() {
    let initial = qemu_processes();
    assert!(
        initial.is_empty(),
        "QEMU was already running; this check did not start it: {initial:?}"
    );
    let run = catch_unwind(AssertUnwindSafe(run_check));
    if run.is_err() {
        emergency_stop_owned_guest();
    }
    let final_processes = wait_for_no_qemu(Duration::from_secs(40));
    assert!(
        final_processes.is_empty(),
        "QEMU remains after browser input check: {final_processes:?}"
    );
    if let Err(payload) = run {
        std::panic::resume_unwind(payload);
    }
}

fn run_check() {
    // The product shell is per-monitor DPI aware (tao), so the runtime's window and screen
    // coordinates are physical pixels, the unit QEMU takes the QMP window geometry in. Without
    // this, a scaled monitor virtualizes this test's coordinates.
    set_process_dpi_awareness_per_monitor_v2().expect("become per-monitor DPI aware");
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let home = OmeHome::resolved().expect("resolve product home");
    let probe = WindowsProbe::new(home.as_path().to_path_buf());
    let adb_found = probe
        .adb()
        .expect("probe adb")
        .expect("adb is required for the real check");
    let adb_program = PathBuf::from(&adb_found.program);
    let adb = AdbSession::new(
        &adb_program,
        ADB_ADDRESS.to_owned(),
        Box::new(ProcessRunner),
    )
    .expect("create adb session");
    let policy = SupervisorPolicy {
        power_off_hook: Arc::new(AdbPowerOff::new(adb.clone())),
        ..SupervisorPolicy::default()
    };
    let supervisor = Supervisor::new(
        WindowsProcessAdapter,
        TcpQmpFactory,
        home.subdir("logs").expect("logs"),
        policy,
    );
    let receiver = supervisor.subscribe();
    let sent: SentGeometries = Arc::new(Mutex::new(Vec::new()));
    let _dpi_guard = set_thread_dpi_hosting_mixed().expect("set mixed DPI hosting");
    let parent = TestHostWindow::create("OME browser input verification", 1280, 720)
        .expect("create test parent window");
    // Expose the host and its owned popup from the start so the liveness samples see the popup.
    parent
        .set_topmost(true)
        .expect("expose test parent during startup");
    let mut runtime = AppRuntime::open(
        home,
        RuntimeDeps {
            probe: Box::new(probe),
            artifacts: None,
            adb: Some(adb),
            supervisor: Some(Box::new(RecordingSupervisor {
                inner: supervisor,
                sent: Arc::clone(&sent),
            })),
            desktop: Box::new(ClosedDesktop),
            window_host: Box::new(GuestWindowHost::default()),
            family_adapter: None,
            images_dir: Some(repository.join("manifests/images")),
            artifacts_manifest: Some(repository.join("manifests/artifacts.json")),
            product_version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )
    .expect("open product runtime");
    runtime.set_host_window(parent.handle().as_u64());
    assert!(
        runtime
            .snapshot()
            .images
            .guests
            .iter()
            .any(|guest| guest.name == GUEST_ID),
        "adopted default guest is unavailable"
    );
    runtime
        .apply(Command::GuestSelect {
            id: GUEST_ID.to_owned(),
        })
        .expect("select default guest");
    runtime
        .apply(Command::StageRectChanged {
            rect: StageRect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
                scale_factor: 1.0,
            },
        })
        .expect("mark stage visible");
    let start_requested = Instant::now();
    runtime.apply(Command::GuestStart).expect("start guest");

    let check = catch_unwind(AssertUnwindSafe(|| {
        let placement =
            verify_startup_popup_placement(&mut runtime, &receiver, &parent, start_requested);
        println!(
            "liveness_samples_shown={} liveness_samples_not_shown={} liveness_max={:.3} liveness_consecutive_changes={}",
            placement.lit_samples.len(),
            placement.unshown_samples,
            placement.lit_max(),
            placement.lit_changes()
        );
        assert!(
            placement.lit_max() >= LIVENESS_MINIMUM,
            "guest popup looks frozen or unexposed: lit samples {:?}",
            placement.lit_samples
        );
        assert!(
            placement.lit_changes() > 0,
            "guest popup frames did not change: lit samples {:?}",
            placement.lit_samples
        );
        wait_for_boot(&mut runtime, &receiver);
        let guest = guest_window(&runtime);
        let top_level = guest.is_top_level();
        let owner = guest.owner();
        let no_activate = guest
            .has_no_activate_style()
            .expect("query WS_EX_NOACTIVATE");
        println!("owned_popup_top_level={top_level}");
        println!(
            "owned_popup_owner_matches_host={}",
            owner == Some(parent.handle())
        );
        println!("owned_popup_no_activate={no_activate}");
        assert!(top_level, "guest popup is not top-level after boot");
        assert_eq!(
            owner,
            Some(parent.handle()),
            "guest popup owner differs from test host"
        );
        assert!(no_activate, "guest popup lacks WS_EX_NOACTIVATE");
        let actual_after_boot = guest.window_rect().expect("guest window rect after boot");
        let expected_after_boot = expected_popup_rect(&parent);
        let sent_after_boot = sent.lock().expect("sent geometries lock").clone();
        let (first_sent_at, first_sent) = *sent_after_boot
            .first()
            .expect("runtime sent a display window geometry");
        let (_, last_sent) = *sent_after_boot
            .last()
            .expect("runtime sent a display window geometry");
        println!(
            "startup_ticks={} startup_visible_rect_mismatch_count={} startup_largest_deviation_px={} startup_largest_tick_interval_ms={} startup_elapsed_ms={}",
            placement.ticks,
            placement.mismatch_count,
            placement.largest_deviation_px,
            placement.largest_tick_interval.as_millis(),
            placement.elapsed.as_millis()
        );
        println!(
            "geometries_sent={} first_sent_after_start_ms={} first_sent={},{} {}x{} visible={}",
            sent_after_boot.len(),
            first_sent_at
                .saturating_duration_since(start_requested)
                .as_millis(),
            first_sent.x,
            first_sent.y,
            first_sent.width,
            first_sent.height,
            first_sent.visible
        );
        println!(
            "popup_rect_after_boot_actual={},{} {}x{} sent={},{} {}x{} visible={}",
            actual_after_boot.x,
            actual_after_boot.y,
            actual_after_boot.width,
            actual_after_boot.height,
            last_sent.x,
            last_sent.y,
            last_sent.width,
            last_sent.height,
            last_sent.visible
        );
        assert!(last_sent.visible, "the last geometry sent hides the popup");
        assert_eq!(
            (
                actual_after_boot.x,
                actual_after_boot.y,
                actual_after_boot.width,
                actual_after_boot.height
            ),
            (last_sent.x, last_sent.y, last_sent.width, last_sent.height),
            "popup rect differs from the geometry the runtime sent"
        );
        println!(
            "popup_rect_after_boot_actual={},{} {}x{} expected={},{} {}x{}",
            actual_after_boot.x,
            actual_after_boot.y,
            actual_after_boot.width,
            actual_after_boot.height,
            expected_after_boot.x,
            expected_after_boot.y,
            expected_after_boot.width,
            expected_after_boot.height
        );
        assert_eq!(actual_after_boot, expected_after_boot);
        runtime
            .apply(Command::GuestWindowToFront)
            .expect("bring hosted guest forward");
        parent.set_topmost(true).expect("expose test parent");
        thread::sleep(Duration::from_millis(500));
        println!(
            "foreground_after_front={:#x}",
            ome_platform_win::foreground_window()
        );
        let focused_after_front = runtime
            .guest_has_keyboard_focus()
            .expect("query guest focus after to_front");
        println!("focus_after_to_front_is_guest={focused_after_front}");
        assert!(!focused_after_front, "guest had focus after to_front");

        let guest_rect = runtime
            .guest_client_screen_rect()
            .expect("embedded guest client rect");
        println!(
            "guest_rect={},{} {}x{}",
            guest_rect.x, guest_rect.y, guest_rect.width, guest_rect.height
        );
        let getevent = GetEvent::start(&adb_program);
        thread::sleep(Duration::from_millis(500));
        getevent.drain();
        let touch_started = Instant::now();
        let click_x = guest_rect.x + i32::try_from(guest_rect.width / 2).expect("width fits i32");
        let click_y = guest_rect.y + i32::try_from(guest_rect.height / 2).expect("height fits i32");
        println!("click_screen={click_x},{click_y}");
        let hit = WindowHandle::from_u64(window_at(click_x, click_y)).expect("hit window");
        println!("window_at_click_before={:#x}", hit.as_u64());
        let hit_class = hit.class_name().expect("hit class");
        println!("window_at_click_class={hit_class}");
        assert_eq!(hit_class, "SDL_app");
        parent
            .handle()
            .to_foreground()
            .expect("activate test host before pointer injection");
        let foreground_before_click = ome_platform_win::foreground_window();
        println!("foreground_before_click={foreground_before_click:#x}");
        click_primary_at(click_x, click_y).expect("click guest popup");
        let foreground_immediately_after_click = ome_platform_win::foreground_window();
        println!("foreground_immediately_after_click={foreground_immediately_after_click:#x}");
        println!(
            "foreground_immediately_after_click_is_guest={}",
            foreground_immediately_after_click == guest.as_u64()
        );
        let touch_lines = getevent.collect_pointer_click(touch_started, Duration::from_secs(5));
        assert!(
            !touch_lines.is_empty(),
            "guest popup click produced no pointer event"
        );
        let pointer_position_count = touch_lines
            .iter()
            .filter(|line| line.contains("ABS_MT_POSITION_X") || line.contains("ABS_X"))
            .count();
        let pointer_down_count = touch_lines
            .iter()
            .filter(|line| {
                (line.contains("BTN_TOUCH")
                    || line.contains("BTN_MOUSE")
                    || line.contains("BTN_LEFT"))
                    && line.contains("DOWN")
            })
            .count();
        println!(
            "pointer_position_events={pointer_position_count} pointer_down_events={pointer_down_count}"
        );
        println!(
            "case=mouse-touch elapsed_ms={}",
            touch_started.elapsed().as_millis()
        );
        assert!(pointer_position_count > 0, "pointer position event missing");
        println!(
            "native_pointer_button_down_verified={}",
            pointer_down_count > 0
        );
        assert!(pointer_down_count > 0, "pointer button DOWN event missing");

        runtime
            .apply(Command::GuestWindowToFront)
            .expect("restore host foreground after guest click");
        thread::sleep(Duration::from_millis(100));
        let foreground_after_click = ome_platform_win::foreground_window();
        println!("foreground_after_click_correction={foreground_after_click:#x}");
        println!(
            "foreground_after_click_correction_is_guest={}",
            foreground_after_click == guest.as_u64()
        );
        let focused_after_click = runtime
            .guest_has_keyboard_focus()
            .expect("query guest focus after click correction");
        println!("focus_after_click_correction_is_guest={focused_after_click}");
        assert_ne!(
            foreground_after_click,
            guest.as_u64(),
            "guest popup remained the foreground window after correction"
        );
        assert!(
            !focused_after_click,
            "guest retained focus after correction"
        );

        getevent.drain();
        let key_started = Instant::now();
        runtime.ingest_browser_key("KeyA", true, true);
        runtime.ingest_browser_key("KeyA", false, true);
        let key_lines = getevent.collect_key_a(key_started, Duration::from_secs(5));
        assert_key_pair("browser", &key_lines);
        let key_down_count = key_lines
            .iter()
            .filter(|line| line.contains("DOWN"))
            .count();
        let key_up_count = key_lines.iter().filter(|line| line.contains("UP")).count();
        println!("key_a_down_events={key_down_count} key_a_up_events={key_up_count}");
        println!(
            "case=browser-key elapsed_ms={}",
            key_started.elapsed().as_millis()
        );
    }));

    stop_through_runtime(&mut runtime, &receiver);
    if let Err(payload) = check {
        std::panic::resume_unwind(payload);
    }
}

#[derive(Debug)]
struct PlacementMeasurements {
    ticks: u32,
    mismatch_count: u32,
    largest_deviation_px: u32,
    largest_tick_interval: Duration,
    elapsed: Duration,
    /// Lit shares of the samples taken while the popup was visible and on top at its centre.
    lit_samples: Vec<f64>,
    unshown_samples: u32,
}

impl PlacementMeasurements {
    fn lit_max(&self) -> f64 {
        self.lit_samples.iter().copied().fold(0.0, f64::max)
    }

    fn lit_changes(&self) -> usize {
        self.lit_samples
            .windows(2)
            .filter(|pair| pair[0] != pair[1])
            .count()
    }
}

fn verify_startup_popup_placement(
    runtime: &mut AppRuntime,
    receiver: &Receiver<GuestEvent>,
    parent: &TestHostWindow,
    start_requested: Instant,
) -> PlacementMeasurements {
    let started = Instant::now();
    let deadline = started + STARTUP_PLACEMENT_WINDOW;
    let liveness_end = start_requested + LIVENESS_WINDOW;
    let mut next_sample = start_requested;
    let mut lit_samples = Vec::new();
    let mut unshown_samples = 0_u32;
    let mut exposed = false;
    let mut ticks = 0_u32;
    let mut mismatch_count = 0_u32;
    let mut largest_deviation_px = 0_u32;
    let mut previous_rect = None;
    let mut previous_tick = None;
    let mut largest_tick_interval = Duration::ZERO;
    while Instant::now() < deadline {
        drain_guest_events(runtime, receiver);
        let tick_at = Instant::now();
        runtime.tick();
        ticks = ticks.saturating_add(1);
        if let Some(previous) = previous_tick {
            largest_tick_interval = largest_tick_interval.max(tick_at.duration_since(previous));
        }
        previous_tick = Some(tick_at);
        let guest = runtime
            .guest_window_handle()
            .map(|raw| WindowHandle::from_u64(raw).expect("valid guest window token"));
        let visible = guest.is_some_and(|guest| guest.is_visible().unwrap_or(false));
        if let Some(guest) = guest.filter(|_| visible) {
            if !exposed {
                // Re-assert the owner's topmost band now that its owned popup is shown.
                parent
                    .set_topmost(true)
                    .expect("expose test parent and its popup");
                exposed = true;
            }
            let before = guest
                .window_rect()
                .expect("guest window rect during startup");
            let expected = expected_popup_rect(parent);
            if before != expected {
                mismatch_count = mismatch_count.saturating_add(1);
                largest_deviation_px = largest_deviation_px.max(rect_deviation(before, expected));
            }
            if previous_rect.is_some_and(|rect| rect != before) {
                println!(
                    "startup_popup_rect_changed={},{} {}x{}",
                    before.x, before.y, before.width, before.height
                );
            }
            previous_rect = Some(before);
        }
        let now = Instant::now();
        if now < liveness_end && now >= next_sample {
            next_sample = now + LIVENESS_INTERVAL;
            let at = now.saturating_duration_since(start_requested).as_millis();
            match guest.filter(|_| visible).and_then(liveness_sample) {
                Some(share) => {
                    println!("liveness_sample t=+{at}ms lit={share:.3}");
                    lit_samples.push(share);
                }
                None => {
                    println!(
                        "liveness_sample t=+{at}ms not-shown attached={} visible={visible}",
                        guest.is_some()
                    );
                    unshown_samples = unshown_samples.saturating_add(1);
                }
            }
        }
        thread::sleep(if now < liveness_end {
            STARTUP_TICK_INTERVAL.min(next_sample.saturating_duration_since(now))
        } else {
            STARTUP_TICK_INTERVAL
        });
    }
    drain_guest_events(runtime, receiver);
    runtime.tick();
    ticks = ticks.saturating_add(1);
    PlacementMeasurements {
        ticks,
        mismatch_count,
        largest_deviation_px,
        largest_tick_interval,
        elapsed: started.elapsed(),
        lit_samples,
        unshown_samples,
    }
}

/// Samples the popup's client area when the popup is the window under its own centre.
fn liveness_sample(guest: WindowHandle) -> Option<f64> {
    let rect = guest.client_screen_rect().ok()?;
    if rect.width == 0 || rect.height == 0 {
        return None;
    }
    let center_x = rect.x + i32::try_from(rect.width / 2).ok()?;
    let center_y = rect.y + i32::try_from(rect.height / 2).ok()?;
    if window_at(center_x, center_y) != guest.as_u64() {
        return None;
    }
    screen_region_lit_share(rect.x, rect.y, rect.width, rect.height).ok()
}

fn guest_window(runtime: &AppRuntime) -> WindowHandle {
    let raw = runtime
        .guest_window_handle()
        .expect("runtime has an attached guest popup");
    WindowHandle::from_u64(raw).expect("valid guest window token")
}

fn expected_popup_rect(parent: &TestHostWindow) -> ome_platform_win::ScreenRect {
    let client = parent
        .handle()
        .client_screen_rect()
        .expect("test host client screen rect");
    ome_platform_win::ScreenRect {
        x: client.x,
        y: client.y,
        width: 1280,
        height: 720,
    }
}

fn rect_deviation(
    actual: ome_platform_win::ScreenRect,
    expected: ome_platform_win::ScreenRect,
) -> u32 {
    actual
        .x
        .abs_diff(expected.x)
        .max(actual.y.abs_diff(expected.y))
        .max(actual.width.abs_diff(expected.width))
        .max(actual.height.abs_diff(expected.height))
}

fn drain_guest_events(runtime: &mut AppRuntime, receiver: &Receiver<GuestEvent>) {
    loop {
        match receiver.try_recv() {
            Ok(event) => runtime.ingest_guest_event(event),
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("supervisor event channel disconnected")
            }
        }
    }
}

fn wait_for_boot(runtime: &mut AppRuntime, receiver: &Receiver<GuestEvent>) {
    let deadline = Instant::now() + BOOT_DEADLINE;
    while Instant::now() < deadline {
        pump(runtime, receiver);
        let snapshot = runtime.snapshot();
        if snapshot.guest.boot_completed {
            println!("bootCompleted=true");
            return;
        }
        assert_ne!(
            snapshot.guest.state,
            GuestState::Failed,
            "guest failed before boot"
        );
    }
    panic!("guest did not boot within {}s", BOOT_DEADLINE.as_secs());
}

fn stop_through_runtime(runtime: &mut AppRuntime, receiver: &Receiver<GuestEvent>) {
    if runtime.snapshot().guest.state == GuestState::Stopped {
        return;
    }
    runtime
        .apply(Command::GuestStop)
        .expect("request guest stop");
    let deadline = Instant::now() + STOP_DEADLINE;
    while Instant::now() < deadline {
        pump(runtime, receiver);
        if matches!(
            runtime.snapshot().guest.state,
            GuestState::Stopped | GuestState::Failed
        ) {
            break;
        }
    }
    let stopped = runtime.snapshot().guest;
    println!(
        "guest_state_after_stop={:?} last_exit={:?}",
        stopped.state,
        stopped.last_exit.map(|exit| exit.kind)
    );
    assert!(
        matches!(stopped.state, GuestState::Stopped | GuestState::Failed),
        "guest did not stop within {}s",
        STOP_DEADLINE.as_secs()
    );
}

fn pump(runtime: &mut AppRuntime, receiver: &Receiver<GuestEvent>) {
    match receiver.recv_timeout(Duration::from_secs(1)) {
        Ok(event) => runtime.ingest_guest_event(event),
        Err(RecvTimeoutError::Timeout) => runtime.tick(),
        Err(RecvTimeoutError::Disconnected) => panic!("supervisor event channel disconnected"),
    }
}

fn assert_key_pair(case: &str, lines: &[String]) {
    assert!(
        lines.iter().any(|line| line.contains("DOWN")),
        "{case}: KEY_A DOWN was not observed: {lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("UP")),
        "{case}: KEY_A UP was not observed: {lines:?}"
    );
}

fn emergency_stop_owned_guest() {
    let home = match OmeHome::resolved() {
        Ok(home) => home,
        Err(_) => return,
    };
    let probe = WindowsProbe::new(home.as_path().to_path_buf());
    let Ok(Some(adb)) = probe.adb() else {
        return;
    };
    let _ = ProcessCommand::new(adb.program)
        .args(["-s", ADB_ADDRESS, "shell", "reboot", "-p"])
        .status();
    let deadline = Instant::now() + Duration::from_secs(35);
    while Instant::now() < deadline && !qemu_processes().is_empty() {
        thread::sleep(Duration::from_millis(250));
    }
}

fn qemu_processes() -> Vec<String> {
    let output = ProcessCommand::new("tasklist.exe")
        .output()
        .expect("run tasklist");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.to_ascii_lowercase().contains("qemu-system"))
        .map(str::trim)
        .map(str::to_owned)
        .collect()
}

fn wait_for_no_qemu(timeout: Duration) -> Vec<String> {
    let deadline = Instant::now() + timeout;
    loop {
        let processes = qemu_processes();
        if processes.is_empty() || Instant::now() >= deadline {
            return processes;
        }
        thread::sleep(Duration::from_millis(100));
    }
}
