// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Measures guest-visible key hold times through the real browser-to-QMP input path.
#![forbid(unsafe_code)]
#![cfg(windows)]

use std::collections::VecDeque;
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
    TestHostWindow, set_process_dpi_awareness_per_monitor_v2, set_thread_dpi_hosting_mixed,
};
use ome_runtime::{
    AdbPowerOff, AppRuntime, Command, Desktop, GuestState, OmeHome, RuntimeDeps, StageRect,
    WindowsProbe,
};
use ome_supervisor::windows_adapter::WindowsProcessAdapter;
use ome_supervisor::{GuestEvent, Supervisor, SupervisorPolicy, TcpQmpFactory};
use ome_window_host::GuestWindowHost;

const GUEST_ID: &str = "default";
const ADB_ADDRESS: &str = "127.0.0.1:5555";
const BOOT_DEADLINE: Duration = Duration::from_secs(180);
const STOP_DEADLINE: Duration = Duration::from_secs(40);
const PRESS_COUNT: usize = 60;
const PRESS_HOLD: Duration = Duration::from_millis(40);
const BETWEEN_PRESSES: Duration = Duration::from_millis(250);
const EVENT_SETTLE: Duration = Duration::from_millis(500);
const BUSY_COMMAND: &str = "for i in 1 2 3 4; do (while :; do :; done) & done; dd if=/dev/zero of=/data/local/tmp/k1.bin bs=1M count=2048 conv=fsync";
const BUSY_CLEANUP_COMMAND: &str = "pkill -f 'while :'; rm -f /data/local/tmp/k1.bin";

#[derive(Debug, Default)]
struct ClosedDesktop;

impl Desktop for ClosedDesktop {
    fn open_path(&self, _path: &Path) -> Result<(), String> {
        Err("desktop opening is disabled in the key hold check".to_owned())
    }

    fn open_url(&self, _url: &str) -> Result<(), String> {
        Err("URL opening is disabled in the key hold check".to_owned())
    }

    fn copy_text(&self, _text: &str) -> Result<(), String> {
        Err("clipboard writes are disabled in the key hold check".to_owned())
    }
}

struct GetEvent {
    child: Child,
    lines: Receiver<String>,
}

impl GetEvent {
    fn start(adb: &Path) -> Self {
        let mut child = ProcessCommand::new(adb)
            .args(["-s", ADB_ADDRESS, "shell", "getevent", "-ltq"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start timestamped adb getevent");
        let stdout = child.stdout.take().expect("capture getevent stdout");
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = sender.send(line);
            }
        });
        Self { child, lines }
    }

    fn drain(&self) {
        while self.lines.try_recv().is_ok() {}
    }

    fn collect_until_quiet(&self, quiet: Duration) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            match self.lines.recv_timeout(quiet) {
                Ok(line) => lines.push(line),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return lines,
            }
        }
    }
}

impl Drop for GetEvent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct BusyGuest {
    adb: PathBuf,
    child: Option<Child>,
}

impl BusyGuest {
    fn start(adb: &Path) -> Self {
        let child = ProcessCommand::new(adb)
            .args(["-s", ADB_ADDRESS, "shell", BUSY_COMMAND])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start guest CPU and disk load");
        Self {
            adb: adb.to_path_buf(),
            child: Some(child),
        }
    }

    fn stop(&mut self) {
        if self.child.is_none() {
            return;
        }
        let cleanup = ProcessCommand::new(&self.adb)
            .args(["-s", ADB_ADDRESS, "shell", BUSY_CLEANUP_COMMAND])
            .status();
        println!("busy_cleanup_status={cleanup:?}");
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for BusyGuest {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KeyAction {
    Down,
    Up,
    Repeat,
}

#[derive(Debug)]
struct KeyEvent {
    guest_seconds: f64,
    action: KeyAction,
    raw: String,
}

#[derive(Debug)]
struct KeyPair {
    down: KeyEvent,
    up: KeyEvent,
}

#[test]
#[ignore = "requires an interactive Windows desktop, WHPX, QEMU, adb, and the adopted default guest"]
fn short_key_presses_are_not_held_past_the_repeat_delay_in_the_guest() {
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
        "QEMU remains after key hold check: {final_processes:?}"
    );
    if let Err(payload) = run {
        std::panic::resume_unwind(payload);
    }
}

fn run_check() {
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
    let _dpi_guard = set_thread_dpi_hosting_mixed().expect("set mixed DPI hosting");
    let parent = TestHostWindow::create("OME key hold verification", 1280, 720)
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
        let getevent = GetEvent::start(&adb_program);
        thread::sleep(Duration::from_millis(500));

        measure_series(
            "enter_idle",
            "Enter",
            "KEY_ENTER",
            &mut runtime,
            &receiver,
            &getevent,
        );
        measure_series(
            "key_a_idle",
            "KeyA",
            "KEY_A",
            &mut runtime,
            &receiver,
            &getevent,
        );

        let mut busy = BusyGuest::start(&adb_program);
        measure_series(
            "enter_busy",
            "Enter",
            "KEY_ENTER",
            &mut runtime,
            &receiver,
            &getevent,
        );
        busy.stop();
    }));

    stop_through_runtime(&mut runtime, &receiver);
    if let Err(payload) = check {
        std::panic::resume_unwind(payload);
    }
}

fn measure_series(
    name: &str,
    browser_code: &str,
    guest_key: &str,
    runtime: &mut AppRuntime,
    receiver: &Receiver<GuestEvent>,
    getevent: &GetEvent,
) {
    getevent.drain();
    let mut host_intervals = Vec::with_capacity(PRESS_COUNT);
    for _ in 0..PRESS_COUNT {
        let pressed_at = Instant::now();
        runtime.ingest_browser_key(browser_code, true, true);
        pump_nonblocking(runtime, receiver);
        thread::sleep(PRESS_HOLD);
        let released_at = Instant::now();
        runtime.ingest_browser_key(browser_code, false, true);
        host_intervals.push(released_at.saturating_duration_since(pressed_at));
        pump_nonblocking(runtime, receiver);
        thread::sleep(BETWEEN_PRESSES);
        pump_nonblocking(runtime, receiver);
    }
    let lines = getevent.collect_until_quiet(EVENT_SETTLE);
    report_series(name, guest_key, &lines, &host_intervals);
}

fn report_series(name: &str, guest_key: &str, lines: &[String], host_intervals: &[Duration]) {
    let events: Vec<_> = lines
        .iter()
        .filter_map(|line| parse_key_event(line, guest_key))
        .collect();
    let repeats = events
        .iter()
        .filter(|event| event.action == KeyAction::Repeat)
        .count();
    let mut pending = VecDeque::new();
    let mut pairs = Vec::new();
    for event in events {
        match event.action {
            KeyAction::Down => pending.push_back(event),
            KeyAction::Up => {
                if let Some(down) = pending.pop_front() {
                    pairs.push(KeyPair { down, up: event });
                }
            }
            KeyAction::Repeat => {}
        }
    }
    let unpaired_down = pending.len();
    let hold_ms: Vec<_> = pairs
        .iter()
        .map(|pair| (pair.up.guest_seconds - pair.down.guest_seconds) * 1_000.0)
        .collect();
    let host_ms: Vec<_> = host_intervals
        .iter()
        .map(|duration| duration.as_secs_f64() * 1_000.0)
        .collect();
    let hold_stats = stats(&hold_ms);
    let host_stats = stats(&host_ms);
    let over_250 = hold_ms
        .iter()
        .filter(|duration| **duration >= 250.0)
        .count();
    println!(
        "series={name} sent={} pairs={} hold_ms_min={} hold_ms_p50={} hold_ms_max={} over_250={over_250} repeats={repeats} unpaired_down={unpaired_down} host_send_interval_ms_min={} host_send_interval_ms_p50={} host_send_interval_ms_max={}",
        host_intervals.len(),
        pairs.len(),
        hold_stats.0,
        hold_stats.1,
        hold_stats.2,
        host_stats.0,
        host_stats.1,
        host_stats.2,
    );
    for (index, (pair, duration)) in pairs.iter().zip(&hold_ms).enumerate() {
        if *duration >= 200.0 {
            let host = host_ms
                .get(index)
                .map_or_else(|| "n/a".to_owned(), |value| format!("{value:.3}"));
            println!(
                "series={name} pair={} guest_hold_ms={duration:.3} host_send_interval_ms={host} down={:?} up={:?}",
                index + 1,
                pair.down.raw,
                pair.up.raw,
            );
        }
    }
    assert!(
        pairs.len() >= 50,
        "series {name} observed only {} of {PRESS_COUNT} key pairs; matching raw lines: {:?}",
        pairs.len(),
        lines
            .iter()
            .filter(|line| line.split_ascii_whitespace().any(|word| word == guest_key))
            .collect::<Vec<_>>()
    );
}

fn parse_key_event(line: &str, guest_key: &str) -> Option<KeyEvent> {
    let trimmed = line.trim_start();
    let timestamp_end = trimmed.find(']')?;
    if !trimmed.starts_with('[') {
        return None;
    }
    let guest_seconds = trimmed[1..timestamp_end].trim().parse().ok()?;
    let words: Vec<_> = trimmed[timestamp_end + 1..]
        .split_ascii_whitespace()
        .collect();
    if !words.contains(&"EV_KEY") || !words.contains(&guest_key) {
        return None;
    }
    let action = if words.contains(&"DOWN") {
        KeyAction::Down
    } else if words.contains(&"UP") {
        KeyAction::Up
    } else if words.contains(&"REPEAT") {
        KeyAction::Repeat
    } else {
        return None;
    };
    Some(KeyEvent {
        guest_seconds,
        action,
        raw: line.to_owned(),
    })
}

fn stats(values: &[f64]) -> (String, String, String) {
    if values.is_empty() {
        return ("n/a".to_owned(), "n/a".to_owned(), "n/a".to_owned());
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    (
        format!("{:.3}", sorted[0]),
        format!("{:.3}", sorted[sorted.len() / 2]),
        format!("{:.3}", sorted[sorted.len() - 1]),
    )
}

fn pump_nonblocking(runtime: &mut AppRuntime, receiver: &Receiver<GuestEvent>) {
    loop {
        match receiver.try_recv() {
            Ok(event) => runtime.ingest_guest_event(event),
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("supervisor event channel disconnected")
            }
        }
    }
    runtime.tick();
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
