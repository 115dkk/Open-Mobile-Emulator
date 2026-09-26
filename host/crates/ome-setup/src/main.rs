// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Elevated setup helper with the single fixed `enable-whpx` verb.
#![forbid(unsafe_code)]

use std::process::Command;

use ome_setup::{SetupBroker, SetupCommand};

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let command = match SetupBroker::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(_) => {
            usage();
            return 2;
        }
    };
    match command {
        SetupCommand::EnableWhpx => {
            let executable = match SetupBroker::executable(std::env::var_os("SystemRoot")) {
                Ok(executable) => executable,
                Err(error) => {
                    eprintln!("ome-setup: SystemRoot is unavailable: {error}");
                    return 1;
                }
            };
            match Command::new(executable)
                .args(SetupBroker::arguments())
                .status()
            {
                Ok(status) => status.code().unwrap_or(1),
                Err(error) => {
                    eprintln!("ome-setup: failed to start System32 dism.exe: {error}");
                    1
                }
            }
        }
    }
}

fn usage() {
    eprintln!("usage: ome-setup.exe enable-whpx");
}
