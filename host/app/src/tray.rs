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

#[derive(Clone)]
pub(crate) struct TrayPowerItem(MenuItem<tauri::Wry>);

pub(crate) fn update_power_label(app: &tauri::AppHandle, state: ome_runtime::GuestState) {
    let label = if matches!(
        state,
        ome_runtime::GuestState::Stopped | ome_runtime::GuestState::Failed
    ) {
        "시작"
    } else {
        "끄기"
    };
    if let Err(error) = app.state::<TrayPowerItem>().0.set_text(label) {
        eprintln!("tray power label could not change: {error}");
    }
}

pub(crate) fn install(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "창 보이기", true, None::<&str>)?;
    let power = MenuItem::with_id(app, "guest-power", "시작", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &power, &quit])?;
    let event_power = power.clone();
    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "guest-power" => {
                let app = app.clone();
                let state = app.state::<ShellState>().inner().clone();
                let running = state.runtime.lock().ok().is_some_and(|guard| {
                    guard.as_ref().ok().is_some_and(|runtime| {
                        !matches!(
                            runtime.snapshot().guest.state,
                            ome_runtime::GuestState::Stopped | ome_runtime::GuestState::Failed
                        )
                    })
                });
                let command = if running {
                    Command::GuestStop
                } else {
                    Command::GuestStart
                };
                let power = event_power.clone();
                tauri::async_runtime::spawn(async move {
                    match commands::apply(app.clone(), &state, command).await {
                        Ok(snapshot) => {
                            let label = if matches!(
                                snapshot.guest.state,
                                ome_runtime::GuestState::Stopped | ome_runtime::GuestState::Failed
                            ) {
                                "시작"
                            } else {
                                "끄기"
                            };
                            if let Err(error) = power.set_text(label) {
                                eprintln!("tray power label could not change: {error}");
                            }
                        }
                        Err(issue) => {
                            eprintln!("tray guest power action failed: {}", issue.code);
                            show_main(&app);
                        }
                    }
                });
            }
            "quit" => {
                let state = app.state::<ShellState>();
                commands::request_app_exit(app, &state);
            }
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
    app.manage(TrayPowerItem(power));
    tray.build(app)?;
    Ok(())
}
