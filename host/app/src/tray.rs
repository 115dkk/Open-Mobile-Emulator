// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
use ome_runtime::Command;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    commands::{self, ShellState},
    window::show_main,
};

pub(crate) fn install(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "창 보이기", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "guest-start", "게스트 시작", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &start, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "guest-start" => {
                let app = app.clone();
                let state = app.state::<ShellState>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(issue) =
                        commands::apply(app.clone(), &state, Command::GuestStart).await
                    {
                        eprintln!("tray guest start failed: {}", issue.code);
                        show_main(&app);
                    }
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
