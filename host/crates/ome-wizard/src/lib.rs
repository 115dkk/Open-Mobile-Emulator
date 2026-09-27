// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Pure first-run wizard state machine and its permission helpers.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The eight first-run wizard states, covering seven user-facing steps plus completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    /// Read-only host inspection.
    HostCheck,
    /// Explicit consent before enabling the Windows feature.
    WhpxConsent,
    /// Waiting for the user to restart Windows.
    RebootPending,
    /// Verified guest artifact download.
    ArtifactDownload,
    /// Interactive guest disk installation.
    GuestInstall,
    /// First persistent guest boot.
    FirstBoot,
    /// Optional initial app installation.
    AppInstall,
    /// Terminal successful state.
    Done,
}

/// Persistable wizard state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardState {
    /// Current state.
    pub step: Step,
    /// States successfully completed or explicitly skipped.
    pub completed: BTreeSet<Step>,
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            step: Step::HostCheck,
            completed: BTreeSet::new(),
        }
    }
}

/// Outcome supplied after the current state's work or user intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Current work succeeded and normal progression may continue.
    Continue,
    /// User asks to skip the current state.
    Skip,
    /// Clear progress and return to host inspection.
    Restart,
    /// Feature enablement succeeded and Windows requires restart.
    WhpxEnabledNeedsReboot,
    /// Host inspection confirmed the feature is already enabled.
    WhpxAlreadyEnabled,
    /// Host inspection contains a blocking result.
    HostBlocked,
}

/// Facts used to decide whether the primary action is currently enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// Host inspection has completed with no blocking row.
    pub host_ready: bool,
    /// User has explicitly consented to the system change.
    pub whpx_consent: bool,
    /// Artifact is present and verified.
    pub artifact_verified: bool,
    /// Guest installation has finished.
    pub guest_installed: bool,
    /// Guest reports first boot complete.
    pub guest_booted: bool,
    /// App-install choice has been resolved.
    pub app_install_resolved: bool,
}

/// Applies one outcome without side effects.
///
/// Invalid outcomes leave the state unchanged. Consent cannot be bypassed with [`Outcome::Skip`],
/// optional app installation can be skipped, [`Step::Done`] is terminal, and [`Outcome::Restart`]
/// always clears completion history.
pub fn advance(mut state: WizardState, outcome: Outcome) -> WizardState {
    if outcome == Outcome::Restart {
        return WizardState::default();
    }
    if state.step == Step::Done {
        return state;
    }
    let next = match (state.step, outcome) {
        (Step::HostCheck, Outcome::Continue) => Some(Step::WhpxConsent),
        (Step::HostCheck, Outcome::WhpxAlreadyEnabled) => Some(Step::ArtifactDownload),
        (Step::HostCheck, Outcome::HostBlocked) => None,
        (Step::WhpxConsent, Outcome::WhpxEnabledNeedsReboot) => Some(Step::RebootPending),
        (Step::RebootPending, Outcome::Continue) => Some(Step::ArtifactDownload),
        (Step::ArtifactDownload, Outcome::Continue) => Some(Step::GuestInstall),
        (Step::GuestInstall, Outcome::Continue) => Some(Step::FirstBoot),
        (Step::FirstBoot, Outcome::Continue) => Some(Step::AppInstall),
        (Step::AppInstall, Outcome::Continue | Outcome::Skip) => Some(Step::Done),
        _ => None,
    };
    if let Some(next) = next {
        state.completed.insert(state.step);
        state.step = next;
    }
    state
}

/// Returns true only for optional initial app installation.
pub const fn can_skip(step: Step) -> bool {
    matches!(step, Step::AppInstall)
}

/// Returns whether the primary action may run for the current state and observed facts.
///
/// Consent is never inferred from host state. Reboot pending and done are actionable without an
/// additional fact; each work state requires its corresponding observed completion fact.
pub const fn can_continue(step: Step, facts: Facts) -> bool {
    match step {
        Step::HostCheck => facts.host_ready,
        Step::WhpxConsent => facts.whpx_consent,
        Step::RebootPending => true,
        Step::ArtifactDownload => facts.artifact_verified,
        Step::GuestInstall => facts.guest_installed,
        Step::FirstBoot => facts.guest_booted,
        Step::AppInstall => facts.app_install_resolved,
        Step::Done => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_success_sequence_reaches_terminal_state() {
        let mut state = advance(WizardState::default(), Outcome::Continue);
        assert_eq!(state.step, Step::WhpxConsent);
        state = advance(state, Outcome::WhpxEnabledNeedsReboot);
        assert_eq!(state.step, Step::RebootPending);
        for expected in [
            Step::ArtifactDownload,
            Step::GuestInstall,
            Step::FirstBoot,
            Step::AppInstall,
            Step::Done,
        ] {
            state = advance(state, Outcome::Continue);
            assert_eq!(state.step, expected);
        }
        assert_eq!(state.completed.len(), 7);
        assert_eq!(advance(state.clone(), Outcome::Continue), state);
    }

    #[test]
    fn already_enabled_skips_consent_and_reboot() {
        let state = advance(WizardState::default(), Outcome::WhpxAlreadyEnabled);
        assert_eq!(state.step, Step::ArtifactDownload);
        assert_eq!(state.completed, BTreeSet::from([Step::HostCheck]));
    }

    #[test]
    fn consent_cannot_be_skipped_but_optional_states_can() {
        let consent = WizardState {
            step: Step::WhpxConsent,
            completed: BTreeSet::new(),
        };
        assert_eq!(advance(consent.clone(), Outcome::Skip), consent);
        assert_eq!(advance(consent.clone(), Outcome::Continue), consent);

        let install = WizardState {
            step: Step::AppInstall,
            completed: BTreeSet::new(),
        };
        assert_eq!(advance(install, Outcome::Skip).step, Step::Done);
    }

    #[test]
    fn reboot_outcome_and_restart_have_explicit_behavior() {
        let consent = WizardState {
            step: Step::WhpxConsent,
            completed: BTreeSet::from([Step::HostCheck]),
        };
        let pending = advance(consent, Outcome::WhpxEnabledNeedsReboot);
        assert_eq!(pending.step, Step::RebootPending);
        assert_eq!(advance(pending, Outcome::Restart), WizardState::default());
    }

    #[test]
    fn blocked_and_invalid_outcomes_do_not_advance() {
        let initial = WizardState::default();
        assert_eq!(advance(initial.clone(), Outcome::HostBlocked), initial);
        assert_eq!(advance(initial.clone(), Outcome::Skip), initial);
    }

    #[test]
    fn skip_and_continue_helpers_cover_every_step() {
        let facts = Facts {
            host_ready: true,
            whpx_consent: true,
            artifact_verified: true,
            guest_installed: true,
            guest_booted: true,
            app_install_resolved: true,
        };
        for step in [
            Step::HostCheck,
            Step::WhpxConsent,
            Step::RebootPending,
            Step::ArtifactDownload,
            Step::GuestInstall,
            Step::FirstBoot,
            Step::AppInstall,
        ] {
            assert!(can_continue(step, facts), "{step:?}");
        }
        assert!(!can_continue(Step::Done, facts));
        assert!(can_skip(Step::AppInstall));
        assert!(!can_skip(Step::WhpxConsent));
    }
}
