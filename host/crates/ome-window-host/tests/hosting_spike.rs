// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Manual native spike for QEMU SDL owned-popup hosting.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs::File;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use ome_platform_win::{JobObject, ProcessLaunch, TestHostWindow};
use ome_window_host::{GuestWindowHost, HostingTarget, Rect};

/// The custom build under the developer's local application data, then the distribution install.
fn qemu_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join(r"OpenMobileEmulator\qemu-build\out\bin\qemu-system-x86_64.exe"),
        );
    }
    candidates.push(PathBuf::from(
        r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    ));
    candidates
}
const QEMU_ARGUMENTS: [&str; 9] = [
    "-S",
    "-machine",
    "q35",
    "-m",
    "64",
    "-nodefaults",
    "-display",
    "sdl,gl=on",
    "-device",
];
const QEMU_DEVICE: &str = "virtio-vga-gl";

#[test]
#[ignore = "requires an interactive Windows desktop and a QEMU SDL owned-popup display"]
fn hosts_qemu_sdl_window_and_reports_measurements() {
    let Some(qemu) = qemu_candidates().into_iter().find(|path| path.is_file()) else {
        println!("SKIP: no QEMU executable found in either configured location");
        return;
    };
    println!("qemu={}", qemu.display());
    println!("arguments={} {}", QEMU_ARGUMENTS.join(" "), QEMU_DEVICE);

    let parent = TestHostWindow::create("OME window-hosting spike", 1280, 800)
        .expect("create parent window");
    println!(
        "parent_client_initial={:?}",
        parent.client_size().expect("parent size")
    );
    let parent_dpi_before = parent.handle().dpi().expect("parent DPI before attach");

    let scratch = std::env::temp_dir().join(format!(
        "ome-window-host-spike-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).expect("create spike scratch directory");
    let stdout = File::create(scratch.join("qemu.stdout.log")).expect("create stdout log");
    let stderr = File::create(scratch.join("qemu.stderr.log")).expect("create stderr log");
    let arguments = QEMU_ARGUMENTS
        .iter()
        .copied()
        .chain(std::iter::once(QEMU_DEVICE))
        .map(OsString::from)
        .collect();
    let job = JobObject::kill_on_close().expect("create kill-on-close job");
    let child = ProcessLaunch {
        executable: qemu,
        arguments,
        stdout,
        stderr,
        cwd: Some(scratch.clone()),
        environment: vec![(
            OsString::from("SDL_WINDOWS_DPI_AWARENESS"),
            OsString::from("permonitorv2"),
        )],
    }
    .spawn_in_job(&job)
    .expect("launch QEMU in job");
    println!("qemu_pid={}", child.pid());

    let mut host = GuestWindowHost::default();
    let discovery_started = Instant::now();
    host.attach(HostingTarget {
        parent_window: parent.handle().as_u64(),
        guest_process_id: child.pid(),
    })
    .expect("attach QEMU SDL window");
    let attach_elapsed = discovery_started.elapsed();
    let discovery_elapsed = host.last_discovery_time().expect("recorded discovery time");
    let guest = host.guest_window().expect("retained guest window");
    let guest_class = guest.class_name().expect("guest class name");
    let guest_dpi_before = host
        .guest_dpi_before_attach()
        .expect("guest DPI before attach");
    let guest_dpi_after = guest.dpi().expect("guest DPI after attach");
    println!("window_class={guest_class}");
    println!("window_discovery_ms={}", discovery_elapsed.as_millis());
    println!("attach_total_ms={}", attach_elapsed.as_millis());
    println!("parent_dpi_before={parent_dpi_before}");
    println!("guest_dpi_before_attach={guest_dpi_before}");
    println!("guest_dpi_after_attach={guest_dpi_after}");
    println!(
        "parent_dpi_after_attach={}",
        parent.handle().dpi().expect("parent DPI after")
    );

    assert!(guest.is_top_level(), "guest must remain top-level");
    assert_eq!(guest.owner(), Some(parent.handle()), "guest owner mismatch");
    assert!(
        guest
            .has_no_activate_style()
            .expect("query WS_EX_NOACTIVATE"),
        "guest popup lacks WS_EX_NOACTIVATE"
    );
    println!("guest_is_top_level={}", guest.is_top_level());
    println!(
        "guest_owner_matches_parent={}",
        guest.owner() == Some(parent.handle())
    );
    println!(
        "guest_has_no_activate_style={}",
        guest
            .has_no_activate_style()
            .expect("query WS_EX_NOACTIVATE")
    );

    let placed = Rect {
        x: 32,
        y: 24,
        width: 960,
        height: 540,
    };
    host.place(placed).expect("place owned popup");
    let parent_client = parent
        .handle()
        .client_screen_rect()
        .expect("parent client screen rect");
    let expected = ome_platform_win::ScreenRect {
        x: parent_client.x + placed.x,
        y: parent_client.y + placed.y,
        width: placed.width,
        height: placed.height,
    };
    wait_for_client_screen_rect(guest, expected);
    let actual = guest
        .client_screen_rect()
        .expect("guest client screen rect");
    println!(
        "parent_client_origin={},{}",
        parent_client.x, parent_client.y
    );
    println!(
        "guest_client_screen_rect={},{} {}x{}",
        actual.x, actual.y, actual.width, actual.height
    );
    assert_eq!(actual, expected);

    parent
        .handle()
        .to_foreground()
        .expect("activate test host before raising popup");
    host.to_front().expect("raise hosted popup");
    thread::sleep(Duration::from_secs(1));
    let guest_focused = guest.has_keyboard_focus().expect("query guest focus");
    println!("guest_has_keyboard_focus_after_raise={guest_focused}");
    println!(
        "foreground_after_raise_is_guest={}",
        ome_platform_win::foreground_window() == guest.as_u64()
    );
    assert!(
        !guest_focused,
        "raising popup must not transfer keyboard focus"
    );

    host.detach().expect("detach guest");
    println!(
        "guest_dpi_after_detach={}",
        guest.dpi().expect("guest DPI after detach")
    );
    println!(
        "guest_alive_after_detach={}",
        guest.belongs_to_process(child.pid())
    );
    println!(
        "host_tracks_guest_after_detach={}",
        host.guest_window_alive()
    );
    println!("guest_attached_after_detach={}", host.is_attached());
    host.to_front().expect("bring detached guest forward");

    child.terminate().expect("terminate QEMU");
    let _ = child
        .wait(Some(Duration::from_secs(5)))
        .expect("wait for QEMU");
    drop(job);
    drop(parent);
    let _ = std::fs::remove_dir_all(scratch);
}

fn wait_for_client_screen_rect(
    window: ome_platform_win::WindowHandle,
    expected: ome_platform_win::ScreenRect,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if window.client_screen_rect().ok() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "guest client screen rect did not become {expected:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
