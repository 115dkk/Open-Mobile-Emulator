// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

use super::{
    CapabilityProbe, FamilyAdapter, ProbeItem, ProbeOutcome, ProbeState, ShellCommand, ShellOutput,
    ShellRunner,
};

struct Run<'a> {
    runner: &'a dyn ShellRunner,
    root: Option<Result<(), ProbeState>>,
    reached_guest: bool,
}

impl Run<'_> {
    fn prepare(&mut self, command: &ShellCommand) -> Result<(), ProbeState> {
        if command.needs_root {
            let result = *self.root.get_or_insert_with(|| {
                self.runner.root().map_err(|_| {
                    // RunnerError does not distinguish root refusal from a dead connection.
                    // If no shell has reached the guest, do not claim root is unavailable.
                    if self.reached_guest {
                        ProbeState::Unavailable
                    } else {
                        ProbeState::Unknown
                    }
                })
            });
            result?;
        }
        Ok(())
    }

    fn shell(&mut self, command: Option<ShellCommand>) -> Result<ShellOutput, ProbeState> {
        let command = command.ok_or(ProbeState::Unavailable)?;
        self.prepare(&command)?;
        let output = self
            .runner
            .shell(&command)
            .map_err(|_| ProbeState::Unknown)?;
        self.reached_guest = true;
        if output.exit_code != 0 && output.stdout.trim().is_empty() {
            return Err(ProbeState::Unknown);
        }
        Ok(output)
    }
}

fn read<T>(
    run: &mut Run<'_>,
    command: Option<ShellCommand>,
    parse: impl FnOnce(&str) -> Option<T>,
    value: &mut Option<T>,
    absence: impl FnOnce(&str) -> bool,
) -> ProbeState {
    let output = match run.shell(command) {
        Ok(output) => output,
        Err(state) => return state,
    };
    *value = parse(&output.stdout);
    if value.is_some() {
        ProbeState::Available
    } else if absence(&output.stdout) {
        ProbeState::Unavailable
    } else {
        ProbeState::Unknown
    }
}

impl CapabilityProbe {
    /// Guest path used for the screenshot probe; removed after the check.
    pub const SCREENSHOT_PROBE_PATH: &'static str = "/data/local/tmp/ome-probe.png";

    /// Runs every probe item over `runner` with `adapter`'s dialect.
    ///
    /// Root is requested at most once, and only when an attempted command needs it.
    #[must_use]
    pub fn run(&self, runner: &dyn ShellRunner, adapter: &dyn FamilyAdapter) -> ProbeOutcome {
        let mut run = Run {
            runner,
            root: None,
            reached_guest: false,
        };
        let mut outcome = ProbeOutcome {
            items: Vec::with_capacity(ProbeItem::ALL.len()),
            device_id: None,
            native_bridge: None,
            media_volume: None,
            google_accounts: None,
            foreground: None,
            root_enabled: None,
            display: None,
            packages: Vec::new(),
        };
        for item in ProbeItem::ALL {
            let state = match item {
                ProbeItem::BootMarker => match run.shell(adapter.boot_completed_command()) {
                    Ok(output) if adapter.parse_boot_completed(&output.stdout) => {
                        ProbeState::Available
                    }
                    Ok(output) if matches!(output.stdout.trim(), "" | "0") => {
                        ProbeState::Unavailable
                    }
                    Ok(_) => ProbeState::Unknown,
                    Err(state) => state,
                },
                ProbeItem::AppList => match run.shell(adapter.packages_command()) {
                    Ok(output) => {
                        outcome.packages = adapter.parse_packages(&output.stdout);
                        if !outcome.packages.is_empty()
                            || (output.exit_code == 0 && output.stdout.trim().is_empty())
                        {
                            ProbeState::Available
                        } else {
                            ProbeState::Unknown
                        }
                    }
                    Err(state) => state,
                },
                ProbeItem::DisplaySize => {
                    match (
                        adapter.display_size_query_command(),
                        adapter.display_density_query_command(),
                    ) {
                        (Some(size), Some(density)) => {
                            let size = run.shell(Some(size));
                            let density = run.shell(Some(density));
                            match (size, density) {
                                (Ok(size), Ok(density)) => {
                                    outcome.display =
                                        adapter.parse_display(&size.stdout, &density.stdout);
                                    if outcome.display.is_some() {
                                        ProbeState::Available
                                    } else {
                                        ProbeState::Unknown
                                    }
                                }
                                (Err(ProbeState::Unknown), _) | (_, Err(ProbeState::Unknown)) => {
                                    ProbeState::Unknown
                                }
                                _ => ProbeState::Unavailable,
                            }
                        }
                        _ => ProbeState::Unavailable,
                    }
                }
                ProbeItem::MediaVolume => read(
                    &mut run,
                    adapter.media_volume_get_command(),
                    |s| adapter.parse_media_volume(s),
                    &mut outcome.media_volume,
                    |_| false,
                ),
                ProbeItem::DeviceId => {
                    let mut state = ProbeState::Unavailable;
                    for attempt in adapter.device_id_attempts() {
                        if let Err(failed) = run.prepare(&attempt.command) {
                            if failed == ProbeState::Unknown {
                                state = failed;
                            }
                            continue;
                        }
                        if let Some(file) = &attempt.push
                            && runner.push(file).is_err()
                        {
                            state = ProbeState::Unknown;
                            continue;
                        }
                        match run.shell(Some(attempt.command)) {
                            Ok(output) => {
                                if let Some(id) = adapter.parse_device_id(&output.stdout) {
                                    outcome.device_id = Some(id);
                                    state = ProbeState::Available;
                                    break;
                                }
                            }
                            Err(ProbeState::Unknown) => state = ProbeState::Unknown,
                            Err(_) => {}
                        }
                    }
                    state
                }
                ProbeItem::Screenshot => {
                    let state =
                        match run.shell(adapter.screenshot_command(Self::SCREENSHOT_PROBE_PATH)) {
                            Ok(output) if output.exit_code == 0 => ProbeState::Available,
                            Ok(_) => ProbeState::Unavailable,
                            Err(state) => state,
                        };
                    // Always try removal, including after capture/root/transport failure.
                    let _ = run.shell(Some(ShellCommand::new([
                        "rm",
                        "-f",
                        Self::SCREENSHOT_PROBE_PATH,
                    ])));
                    state
                }
                ProbeItem::ForegroundApp => read(
                    &mut run,
                    adapter.foreground_command(),
                    |s| adapter.parse_foreground(s),
                    &mut outcome.foreground,
                    |s| {
                        s.lines().any(|line| {
                            matches!(
                                line.trim(),
                                "topResumedActivity=null"
                                    | "mResumedActivity: null"
                                    | "mResumedActivity=null"
                            )
                        })
                    },
                ),
                ProbeItem::Multitouch => {
                    let mut present = None;
                    let state = read(
                        &mut run,
                        adapter.input_devices_command(),
                        |s| adapter.parse_multitouch(s),
                        &mut present,
                        |_| false,
                    );
                    if present == Some(false) {
                        ProbeState::Unavailable
                    } else {
                        state
                    }
                }
                ProbeItem::NativeBridge => read(
                    &mut run,
                    adapter.native_bridge_command(),
                    |s| adapter.parse_native_bridge(s),
                    &mut outcome.native_bridge,
                    |s| matches!(s.trim(), "" | "0"),
                ),
                ProbeItem::Root => {
                    let state = read(
                        &mut run,
                        adapter.root_state_command(),
                        |s| adapter.parse_root_state(s),
                        &mut outcome.root_enabled,
                        |_| false,
                    );
                    if outcome.root_enabled == Some(false) {
                        ProbeState::Unavailable
                    } else {
                        state
                    }
                }
            };
            outcome.items.push((item, state));
        }
        // The public contract has no GoogleAccounts ProbeItem, but carries the learned count.
        let _ = read(
            &mut run,
            adapter.google_accounts_command(),
            |s| adapter.parse_google_accounts(s),
            &mut outcome.google_accounts,
            |_| false,
        );
        outcome
    }
}
