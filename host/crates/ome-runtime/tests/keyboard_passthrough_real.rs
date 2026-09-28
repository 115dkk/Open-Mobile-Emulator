// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Manual QMP keyboard pass-through check against the adopted Android guest.
#![forbid(unsafe_code)]
#![cfg(windows)]

use std::io::{BufRead as _, BufReader};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use ome_adb::{AdbSession, ProcessRunner};
use ome_host_check::HostProbe;
use ome_platform_win::{
    TestHostWindow, WindowHandle, click_primary_at, set_thread_dpi_hosting_mixed, window_at,
};
use ome_runtime::{
    AdbPowerOff, AppRuntime, Command, Desktop, GuestState, OmeHome, RuntimeDeps, StageRect,
    WindowsProbe,
};
use ome_supervisor::windows_adapter::WindowsProcessAdapter;
use ome_supervisor::{GuestEvent, Supervisor, SupervisorPolicy, TcpQmpFactory};
use ome_window_host::GuestWindowHost;

const GUEST_ID: &str = "default";
const BOOT_DEADLINE: Duration = Duration::from_secs(180);
const STOP_DEADLINE: Duration = Duration::from_secs(40);
const ADB_ADDRESS: &str = "127.0.0.1:5555";

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
        if found.is_empty() {
            println!("getevent unfiltered={observed:?}");
        }
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
    let _dpi_guard = set_thread_dpi_hosting_mixed().expect("set mixed DPI hosting");
    let parent = TestHostWindow::create("OME browser input verification", 1280, 720)
        .expect("create test parent window");
    let mut runtime = AppRuntime::open(
        home,
        RuntimeDeps {
            probe: Box::new(probe),
            artifacts: None,
            adb: Some(adb),
            supervisor: Some(Box::new(supervisor)),
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
    runtime.apply(Command::GuestStart).expect("start guest");

    let check = catch_unwind(AssertUnwindSafe(|| {
        wait_for_boot(&mut runtime, &receiver);
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
        click_primary_at(click_x, click_y).expect("click embedded guest");
        let focused_after_click = runtime
            .guest_has_keyboard_focus()
            .expect("query guest focus after click");
        println!("focus_immediately_after_click_is_guest={focused_after_click}");
        let touch_lines = getevent.collect_pointer_click(touch_started, Duration::from_secs(5));
        assert!(
            !touch_lines.is_empty(),
            "embedded guest click produced no touch event"
        );
        println!(
            "case=mouse-touch elapsed_ms={}",
            touch_started.elapsed().as_millis()
        );

        thread::sleep(Duration::from_millis(100));
        if focused_after_click {
            runtime
                .apply(Command::GuestWindowToFront)
                .expect("return focus to parent after click");
            thread::sleep(Duration::from_millis(100));
        }
        let focused_after_correction = runtime
            .guest_has_keyboard_focus()
            .expect("query corrected guest focus");
        println!("focus_after_click_is_guest={focused_after_click}");
        println!("focus_after_correction_is_guest={focused_after_correction}");
        assert!(
            !focused_after_correction,
            "guest retained focus after correction"
        );

        getevent.drain();
        let key_started = Instant::now();
        runtime.ingest_browser_key("KeyA", true, true);
        runtime.ingest_browser_key("KeyA", false, true);
        let key_lines = getevent.collect_key_a(key_started, Duration::from_secs(5));
        assert_key_pair("browser", &key_lines);
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
    assert!(
        matches!(
            runtime.snapshot().guest.state,
            GuestState::Stopped | GuestState::Failed
        ),
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
