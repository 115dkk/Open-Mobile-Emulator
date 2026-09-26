// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Read-only host readiness inspection with a replaceable probe.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result of one read-only host probe.
pub type Probe<T> = Result<T, ProbeError>;

/// Stable categories for host observation failures.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ProbeError {
    /// The Windows platform adapter has not supplied this observation yet.
    #[error("host probe is not wired")]
    Unwired,
    /// The operating-system value could not be read.
    #[error("host probe is unavailable")]
    Unavailable,
    /// The operating-system value had an unsupported representation.
    #[error("host probe returned invalid data")]
    InvalidData,
}

/// Installation state of the Windows Hypervisor Platform optional feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeatureState {
    /// The feature is installed and reports enabled.
    Enabled,
    /// The feature is installed but disabled.
    Disabled,
    /// Windows reports that the feature payload is absent.
    Absent,
    /// The state could not be mapped reliably.
    Unknown,
}

/// A discovered QEMU installation, kept internal to the host layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QemuFound {
    /// Parsed or publisher-provided version text.
    pub version: String,
    /// Discovery source such as an environment override or product bundle.
    pub source: String,
}

/// Read-only host observation seam.
///
/// Implementations must not enable features, change boot configuration, install software, or create
/// files. Each method reports observation failure separately instead of inventing a value.
pub trait HostProbe: Send + Sync {
    /// Reports whether firmware and CPU configuration expose hardware virtualization.
    fn cpu_virtualization(&self) -> Probe<bool>;
    /// Reports the optional-feature state without changing it.
    fn hypervisor_platform(&self) -> Probe<FeatureState>;
    /// Reports whether Windows has a pending reboot relevant to readiness.
    fn reboot_pending(&self) -> Probe<bool>;
    /// Reports whether the host virtualization execution interface is callable.
    fn whpx_available(&self) -> Probe<bool>;
    /// Reports a discovered virtual-machine executable, or confirmed absence.
    fn qemu(&self) -> Probe<Option<QemuFound>>;
    /// Reports whether required firmware files are present.
    fn firmware(&self) -> Probe<bool>;
    /// Reports the discovered adb version string, or confirmed absence.
    fn adb(&self) -> Probe<Option<String>>;
    /// Reports available bytes on the volume that holds OME home.
    fn free_disk_bytes(&self) -> Probe<u64>;
}

/// Identifier for one host-readiness row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostCheckId {
    /// Hardware virtualization support.
    CpuVirtualization,
    /// Windows Hypervisor Platform optional feature.
    HypervisorPlatform,
    /// Pending-reboot state.
    RebootPending,
    /// Host virtualization execution interface.
    WhpxAvailable,
    /// Virtual-machine executable presence.
    QemuPresent,
    /// Firmware presence.
    FirmwarePresent,
    /// Android debugging tool presence.
    AdbPresent,
    /// Free storage space.
    DiskSpace,
}

/// Outcome of one host-readiness observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostStatus {
    /// The observed item is ready.
    Ready,
    /// The item needs user attention but does not stop the wizard.
    Attention,
    /// The item prevents guest setup or execution.
    Blocked,
}

/// One host observation with user-facing outcome and next action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRow {
    /// Stable row identifier.
    pub id: HostCheckId,
    /// Readiness classification.
    pub status: HostStatus,
    /// Korean outcome-and-action sentence without implementation names.
    pub detail: String,
}

/// Aggregate host verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// No row requires attention.
    Ready,
    /// No row blocks setup, but at least one needs attention.
    Attention,
    /// At least one row blocks setup or execution.
    Blocked,
}

/// Complete read-only host report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Rows in stable contract order.
    pub rows: Vec<HostRow>,
    /// Aggregate verdict derived from the rows.
    pub verdict: Verdict,
}

/// Pure host-readiness evaluator.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostReadiness;

impl HostReadiness {
    /// Inspects every probe and returns all rows even when individual reads fail.
    ///
    /// CPU virtualization off, missing virtual-machine executable or firmware, less than 40 GiB,
    /// and an enabled feature with a pending reboot are blocking. A disabled feature, missing
    /// optional debugging tool, and unknown feature state require attention.
    pub fn inspect(probe: &dyn HostProbe) -> Report {
        const MINIMUM_DISK_BYTES: u64 = 40 * 1024 * 1024 * 1024;

        let feature = probe.hypervisor_platform();
        let reboot = probe.reboot_pending();
        let feature_enabled = feature == Ok(FeatureState::Enabled);

        let rows = vec![
            bool_row(
                HostCheckId::CpuVirtualization,
                probe.cpu_virtualization(),
                HostStatus::Blocked,
                "프로세서 가상화를 사용할 수 있습니다.",
                "프로세서 가상화가 꺼져 있습니다. 컴퓨터의 펌웨어 설정에서 켠 뒤 다시 확인하십시오.",
                "프로세서 가상화 상태를 확인하지 못했습니다. 컴퓨터를 다시 시작한 뒤 다시 확인하십시오.",
            ),
            match feature {
                Ok(FeatureState::Enabled) => row(
                    HostCheckId::HypervisorPlatform,
                    HostStatus::Ready,
                    "Windows 하이퍼바이저 플랫폼이 켜져 있습니다.",
                ),
                Ok(FeatureState::Disabled) => row(
                    HostCheckId::HypervisorPlatform,
                    HostStatus::Attention,
                    "Windows 하이퍼바이저 플랫폼이 꺼져 있습니다. 다음 단계에서 동의하면 켭니다.",
                ),
                Ok(FeatureState::Absent) => row(
                    HostCheckId::HypervisorPlatform,
                    HostStatus::Attention,
                    "Windows 하이퍼바이저 플랫폼 구성 요소가 없습니다. 다음 단계에서 동의하면 설치합니다.",
                ),
                Ok(FeatureState::Unknown) | Err(_) => row(
                    HostCheckId::HypervisorPlatform,
                    HostStatus::Attention,
                    "Windows 하이퍼바이저 플랫폼 상태를 확인하지 못했습니다. Windows 기능 설정을 확인하십시오.",
                ),
            },
            match reboot {
                Ok(true) if feature_enabled => row(
                    HostCheckId::RebootPending,
                    HostStatus::Blocked,
                    "기능 변경을 적용하려면 다시 시작해야 합니다. 작업을 저장하고 컴퓨터를 다시 시작하십시오.",
                ),
                Ok(true) => row(
                    HostCheckId::RebootPending,
                    HostStatus::Attention,
                    "Windows가 다시 시작을 기다리고 있습니다. 설정을 진행하기 전에 다시 시작하는 편이 좋습니다.",
                ),
                Ok(false) => row(
                    HostCheckId::RebootPending,
                    HostStatus::Ready,
                    "적용을 기다리는 다시 시작 작업이 없습니다.",
                ),
                Err(_) => row(
                    HostCheckId::RebootPending,
                    HostStatus::Attention,
                    "다시 시작이 필요한지 확인하지 못했습니다. Windows 업데이트 상태를 확인하십시오.",
                ),
            },
            match probe.whpx_available() {
                Ok(true) => row(
                    HostCheckId::WhpxAvailable,
                    HostStatus::Ready,
                    "가상화 실행 기능을 사용할 수 있습니다.",
                ),
                Ok(false) if feature_enabled => row(
                    HostCheckId::WhpxAvailable,
                    HostStatus::Blocked,
                    "가상화 실행 기능을 사용할 수 없습니다. 컴퓨터를 다시 시작한 뒤 다시 확인하십시오.",
                ),
                Ok(false) => row(
                    HostCheckId::WhpxAvailable,
                    HostStatus::Attention,
                    "가상화 실행 기능이 아직 준비되지 않았습니다. Windows 하이퍼바이저 플랫폼을 켜십시오.",
                ),
                Err(_) => row(
                    HostCheckId::WhpxAvailable,
                    HostStatus::Attention,
                    "가상화 실행 기능을 확인하지 못했습니다. 컴퓨터를 다시 시작한 뒤 다시 확인하십시오.",
                ),
            },
            match probe.qemu() {
                Ok(Some(_)) => row(
                    HostCheckId::QemuPresent,
                    HostStatus::Ready,
                    "가상 머신 실행 파일을 찾았습니다.",
                ),
                Ok(None) => row(
                    HostCheckId::QemuPresent,
                    HostStatus::Blocked,
                    "가상 머신 실행 파일이 없습니다. 앱을 다시 설치하십시오.",
                ),
                Err(_) => row(
                    HostCheckId::QemuPresent,
                    HostStatus::Blocked,
                    "가상 머신 실행 파일을 확인하지 못했습니다. 앱을 다시 설치한 뒤 다시 확인하십시오.",
                ),
            },
            bool_row(
                HostCheckId::FirmwarePresent,
                probe.firmware(),
                HostStatus::Blocked,
                "게스트 시작에 필요한 펌웨어를 찾았습니다.",
                "게스트 시작에 필요한 펌웨어가 없습니다. 앱을 다시 설치하십시오.",
                "게스트 시작에 필요한 펌웨어를 확인하지 못했습니다. 앱을 다시 설치한 뒤 다시 확인하십시오.",
            ),
            match probe.adb() {
                Ok(Some(_)) => row(
                    HostCheckId::AdbPresent,
                    HostStatus::Ready,
                    "게스트 앱 관리 도구를 찾았습니다.",
                ),
                Ok(None) => row(
                    HostCheckId::AdbPresent,
                    HostStatus::Attention,
                    "게스트 앱 관리 도구가 없습니다. 앱 설치 기능을 쓰려면 개발 도구를 설치하십시오.",
                ),
                Err(_) => row(
                    HostCheckId::AdbPresent,
                    HostStatus::Attention,
                    "게스트 앱 관리 도구를 확인하지 못했습니다. 개발 도구 설치 상태를 확인하십시오.",
                ),
            },
            match probe.free_disk_bytes() {
                Ok(bytes) if bytes >= MINIMUM_DISK_BYTES => row(
                    HostCheckId::DiskSpace,
                    HostStatus::Ready,
                    "게스트 설치에 필요한 저장 공간이 있습니다.",
                ),
                Ok(_) => row(
                    HostCheckId::DiskSpace,
                    HostStatus::Blocked,
                    "저장 공간이 40GB보다 적습니다. 파일을 정리한 뒤 다시 확인하십시오.",
                ),
                Err(_) => row(
                    HostCheckId::DiskSpace,
                    HostStatus::Blocked,
                    "남은 저장 공간을 확인하지 못했습니다. 저장 장치를 확인한 뒤 다시 시도하십시오.",
                ),
            },
        ];

        let verdict = if rows.iter().any(|row| row.status == HostStatus::Blocked) {
            Verdict::Blocked
        } else if rows.iter().any(|row| row.status == HostStatus::Attention) {
            Verdict::Attention
        } else {
            Verdict::Ready
        };
        Report { rows, verdict }
    }
}

fn bool_row(
    id: HostCheckId,
    value: Probe<bool>,
    false_status: HostStatus,
    ready_detail: &str,
    false_detail: &str,
    error_detail: &str,
) -> HostRow {
    match value {
        Ok(true) => row(id, HostStatus::Ready, ready_detail),
        Ok(false) => row(id, false_status, false_detail),
        Err(_) => row(id, false_status, error_detail),
    }
}

fn row(id: HostCheckId, status: HostStatus, detail: &str) -> HostRow {
    HostRow {
        id,
        status,
        detail: detail.to_owned(),
    }
}

/// Table-backed probe for deterministic tests and non-native previews.
///
/// Missing entries return [`ProbeError::Unwired`]; callers never receive guessed host state.
#[derive(Clone, Debug, Default)]
pub struct TableProbe {
    /// Probe values keyed by stable row identifier.
    pub values: BTreeMap<HostCheckId, ProbeValue>,
}

impl TableProbe {
    /// Creates an empty table where every observation is unwired.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces one table value and returns the table for fluent fixture construction.
    pub fn with(mut self, id: HostCheckId, value: ProbeValue) -> Self {
        self.values.insert(id, value);
        self
    }

    fn value(&self, id: HostCheckId) -> Probe<&ProbeValue> {
        match self.values.get(&id) {
            Some(ProbeValue::Error(error)) => Err(*error),
            Some(value) => Ok(value),
            None => Err(ProbeError::Unwired),
        }
    }
}

/// Typed fixture value stored by [`TableProbe`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeValue {
    /// Boolean observation.
    Bool(bool),
    /// Optional-feature observation.
    Feature(FeatureState),
    /// Optional virtual-machine executable observation.
    Qemu(Option<QemuFound>),
    /// Optional Android debugging tool version.
    Adb(Option<String>),
    /// Free storage bytes.
    Bytes(u64),
    /// Explicit observation failure.
    Error(ProbeError),
}

impl HostProbe for TableProbe {
    fn cpu_virtualization(&self) -> Probe<bool> {
        bool_value(self.value(HostCheckId::CpuVirtualization)?)
    }

    fn hypervisor_platform(&self) -> Probe<FeatureState> {
        match self.value(HostCheckId::HypervisorPlatform)? {
            ProbeValue::Feature(value) => Ok(*value),
            _ => Err(ProbeError::InvalidData),
        }
    }

    fn reboot_pending(&self) -> Probe<bool> {
        bool_value(self.value(HostCheckId::RebootPending)?)
    }

    fn whpx_available(&self) -> Probe<bool> {
        bool_value(self.value(HostCheckId::WhpxAvailable)?)
    }

    fn qemu(&self) -> Probe<Option<QemuFound>> {
        match self.value(HostCheckId::QemuPresent)? {
            ProbeValue::Qemu(value) => Ok(value.clone()),
            _ => Err(ProbeError::InvalidData),
        }
    }

    fn firmware(&self) -> Probe<bool> {
        bool_value(self.value(HostCheckId::FirmwarePresent)?)
    }

    fn adb(&self) -> Probe<Option<String>> {
        match self.value(HostCheckId::AdbPresent)? {
            ProbeValue::Adb(value) => Ok(value.clone()),
            _ => Err(ProbeError::InvalidData),
        }
    }

    fn free_disk_bytes(&self) -> Probe<u64> {
        match self.value(HostCheckId::DiskSpace)? {
            ProbeValue::Bytes(value) => Ok(*value),
            _ => Err(ProbeError::InvalidData),
        }
    }
}

fn bool_value(value: &ProbeValue) -> Probe<bool> {
    match value {
        ProbeValue::Bool(value) => Ok(*value),
        _ => Err(ProbeError::InvalidData),
    }
}

/// Windows placeholder until `ome-platform-win` supplies native observations.
///
/// Every method returns [`ProbeError::Unwired`], never a synthetic success.
#[cfg(windows)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsProbe;

#[cfg(windows)]
impl HostProbe for WindowsProbe {
    fn cpu_virtualization(&self) -> Probe<bool> {
        Err(ProbeError::Unwired)
    }

    fn hypervisor_platform(&self) -> Probe<FeatureState> {
        Err(ProbeError::Unwired)
    }

    fn reboot_pending(&self) -> Probe<bool> {
        Err(ProbeError::Unwired)
    }

    fn whpx_available(&self) -> Probe<bool> {
        Err(ProbeError::Unwired)
    }

    fn qemu(&self) -> Probe<Option<QemuFound>> {
        Err(ProbeError::Unwired)
    }

    fn firmware(&self) -> Probe<bool> {
        Err(ProbeError::Unwired)
    }

    fn adb(&self) -> Probe<Option<String>> {
        Err(ProbeError::Unwired)
    }

    fn free_disk_bytes(&self) -> Probe<u64> {
        Err(ProbeError::Unwired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_probe() -> TableProbe {
        TableProbe::new()
            .with(HostCheckId::CpuVirtualization, ProbeValue::Bool(true))
            .with(
                HostCheckId::HypervisorPlatform,
                ProbeValue::Feature(FeatureState::Enabled),
            )
            .with(HostCheckId::RebootPending, ProbeValue::Bool(false))
            .with(HostCheckId::WhpxAvailable, ProbeValue::Bool(true))
            .with(
                HostCheckId::QemuPresent,
                ProbeValue::Qemu(Some(QemuFound {
                    version: "11.1".to_owned(),
                    source: "bundle".to_owned(),
                })),
            )
            .with(HostCheckId::FirmwarePresent, ProbeValue::Bool(true))
            .with(
                HostCheckId::AdbPresent,
                ProbeValue::Adb(Some("37".to_owned())),
            )
            .with(
                HostCheckId::DiskSpace,
                ProbeValue::Bytes(40 * 1024 * 1024 * 1024),
            )
    }

    #[test]
    fn all_ready_values_produce_ready_verdict() {
        let report = HostReadiness::inspect(&ready_probe());
        assert_eq!(report.verdict, Verdict::Ready);
        assert_eq!(report.rows.len(), 8);
        assert!(
            report
                .rows
                .iter()
                .all(|row| row.status == HostStatus::Ready)
        );
    }

    #[test]
    fn exact_blocking_rules_stop_readiness() {
        for (id, value) in [
            (HostCheckId::CpuVirtualization, ProbeValue::Bool(false)),
            (HostCheckId::QemuPresent, ProbeValue::Qemu(None)),
            (HostCheckId::FirmwarePresent, ProbeValue::Bool(false)),
            (
                HostCheckId::DiskSpace,
                ProbeValue::Bytes(40 * 1024 * 1024 * 1024 - 1),
            ),
            (HostCheckId::RebootPending, ProbeValue::Bool(true)),
        ] {
            let report = HostReadiness::inspect(&ready_probe().with(id, value));
            assert_eq!(report.verdict, Verdict::Blocked, "failed for {id:?}");
        }
    }

    #[test]
    fn missing_adb_and_unknown_feature_need_attention() {
        let missing_adb = HostReadiness::inspect(
            &ready_probe().with(HostCheckId::AdbPresent, ProbeValue::Adb(None)),
        );
        assert_eq!(missing_adb.verdict, Verdict::Attention);

        let unknown_feature = HostReadiness::inspect(&ready_probe().with(
            HostCheckId::HypervisorPlatform,
            ProbeValue::Feature(FeatureState::Unknown),
        ));
        assert_eq!(unknown_feature.verdict, Verdict::Attention);
    }

    #[test]
    fn disabled_feature_requires_attention_before_explicit_consent() {
        let report = HostReadiness::inspect(
            &ready_probe()
                .with(
                    HostCheckId::HypervisorPlatform,
                    ProbeValue::Feature(FeatureState::Disabled),
                )
                .with(HostCheckId::WhpxAvailable, ProbeValue::Bool(false)),
        );
        assert_eq!(report.verdict, Verdict::Attention);
        assert!(
            !report
                .rows
                .iter()
                .any(|row| row.status == HostStatus::Blocked)
        );
    }

    #[test]
    fn enabled_feature_with_pending_reboot_is_blocked() {
        let report = HostReadiness::inspect(
            &ready_probe().with(HostCheckId::RebootPending, ProbeValue::Bool(true)),
        );
        assert_eq!(report.verdict, Verdict::Blocked);
        assert_eq!(
            report
                .rows
                .iter()
                .find(|row| row.id == HostCheckId::RebootPending)
                .expect("reboot row")
                .status,
            HostStatus::Blocked
        );
    }
}
