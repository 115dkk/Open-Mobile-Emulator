// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Parser for install-helper progress written to the serial log.

/// Stage of the helper's install, in the order the helper reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallStage {
    /// No step line has arrived yet.
    Waiting,
    /// The helper is unpacking and finding its tools.
    Tools,
    /// The helper is partitioning the guest disk.
    Partition,
    /// The helper is formatting the guest partition.
    Format,
    /// The helper is copying the guest files.
    Copy,
    /// The helper is finishing the installation.
    Finish,
    /// The helper completed successfully.
    Done,
    /// The helper reported a failure.
    Failed,
}

/// What the serial log says so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallReport {
    /// Current stage, or [`InstallStage::Waiting`] before the first step line.
    pub stage: InstallStage,
    /// Most recently reported percentage, clamped to 0 through 100.
    pub percent: u8,
    /// Text following the most recent `fail` line.
    pub failure: Option<String>,
    /// Tool, source and filesystem facts in report order.
    pub facts: Vec<String>,
}

impl Default for InstallReport {
    fn default() -> Self {
        Self {
            stage: InstallStage::Waiting,
            percent: 0,
            failure: None,
            facts: Vec::new(),
        }
    }
}

/// Parses the whole serial log into the latest install report.
#[must_use]
pub fn parse_serial_log(text: &str) -> InstallReport {
    const PREFIX: &str = "OME-INSTALL ";

    let mut report = InstallReport::default();
    for raw_line in text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let Some(prefix_offset) = line.find(PREFIX) else {
            continue;
        };
        let message = &line[prefix_offset + PREFIX.len()..];

        if let Some(step) = message.strip_prefix("step ") {
            let mut words = step.split_whitespace();
            if let Some(name) = words.next()
                && let Some(stage) = stage_from_name(name)
            {
                report.stage = stage;
            }
            if let Some(percent) = words.next().and_then(|value| value.parse::<u64>().ok()) {
                report.percent = u8::try_from(percent.min(100)).expect("percentage is at most 100");
            }
        } else if message == "done" {
            report.stage = InstallStage::Done;
            report.percent = 100;
        } else if let Some(reason) = message.strip_prefix("fail ") {
            report.stage = InstallStage::Failed;
            report.failure = Some(reason.to_owned());
        } else if message.starts_with("tools ")
            || message.starts_with("source ")
            || message.starts_with("df ")
        {
            report.facts.push(message.to_owned());
        }
    }
    report
}

fn stage_from_name(name: &str) -> Option<InstallStage> {
    match name {
        "tools" => Some(InstallStage::Tools),
        "partition" => Some(InstallStage::Partition),
        "format" => Some(InstallStage::Format),
        "copy" => Some(InstallStage::Copy),
        "finish" => Some(InstallStage::Finish),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallReport, InstallStage, parse_serial_log};

    const REAL_SERIAL_LOG: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/evidence/M2/unattended-install-spike/install-serial.log"
    ));

    #[test]
    fn parses_completed_spike_log() {
        let report = parse_serial_log(REAL_SERIAL_LOG);
        assert_eq!(report.stage, InstallStage::Done);
        assert_eq!(report.percent, 100);
        assert!(report.failure.is_none());
        assert!(
            report
                .facts
                .first()
                .is_some_and(|fact| fact.starts_with("tools sgdisk=/bin/sgdisk"))
        );
    }

    #[test]
    fn parses_spike_log_through_copy_progress() {
        let marker = "OME-INSTALL step copy 59";
        let end = REAL_SERIAL_LOG.find(marker).expect("copy marker") + marker.len();
        let report = parse_serial_log(&REAL_SERIAL_LOG[..end]);
        assert_eq!(report.stage, InstallStage::Copy);
        assert_eq!(report.percent, 59);
    }

    #[test]
    fn parses_failure_kernel_noise_and_clamped_percentage() {
        let report = parse_serial_log(
            "kernel: OME-INSTALL step copy 999\r\nOME-INSTALL step unknown nope\nOME-INSTALL fail mke2fs\r\n",
        );
        assert_eq!(report.stage, InstallStage::Failed);
        assert_eq!(report.percent, 100);
        assert_eq!(report.failure.as_deref(), Some("mke2fs"));
    }

    #[test]
    fn failure_after_done_wins() {
        let report = parse_serial_log("OME-INSTALL done\nOME-INSTALL fail late error\n");
        assert_eq!(report.stage, InstallStage::Failed);
        assert_eq!(report.percent, 100);
        assert_eq!(report.failure.as_deref(), Some("late error"));
    }

    #[test]
    fn empty_log_is_default_report() {
        assert_eq!(parse_serial_log(""), InstallReport::default());
    }
}
