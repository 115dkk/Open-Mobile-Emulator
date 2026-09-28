// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Manual native spike for QEMU SDL child-window hosting.
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
#[ignore = "requires an interactive Windows desktop and a QEMU SDL display"]
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

    host.place(Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 800,
    })
    .expect("place at 1280 by 800");
    wait_for_client_size(guest, (1280, 800));
    println!(
        "guest_client_after_1280x800={:?}",
        guest.client_size().expect("large guest size")
    );

    parent.resize(640, 400).expect("resize parent");
    host.place(Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 400,
    })
    .expect("place at 640 by 400");
    wait_for_client_size(guest, (640, 400));
    println!(
        "parent_client_resized={:?}",
        parent.client_size().expect("resized parent size")
    );
    println!(
        "guest_client_after_640x400={:?}",
        guest.client_size().expect("small guest size")
    );
    println!(
        "render_scale_parent_dpi={}",
        parent.handle().dpi().expect("resized parent DPI")
    );
    println!(
        "render_scale_guest_dpi={}",
        guest.dpi().expect("resized guest DPI")
    );
    println!(
        "child_rendered_at_parent_scale={}",
        guest.client_size().expect("rendered guest size") == (640, 400)
            && guest.dpi().expect("rendered guest DPI")
                == parent.handle().dpi().expect("rendered parent DPI")
    );

    host.to_front().expect("raise hosted child");
    thread::sleep(Duration::from_secs(1));
    println!("focus_method=parent-thread raise child + activate/focus parent");
    let guest_focused = guest.has_keyboard_focus().expect("query guest focus");
    println!("guest_has_keyboard_focus={guest_focused}");
    assert!(!guest_focused, "hosted child must not own keyboard focus");

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

fn wait_for_client_size(window: ome_platform_win::WindowHandle, expected: (i32, i32)) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if window.client_size().ok() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "guest client size did not become {expected:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
