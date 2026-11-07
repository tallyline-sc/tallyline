// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Complete fresh pre-send review before an atomic write-ahead reservation.

use tallyline_reporting::api::model::{ReleaseBlockReason, SubmissionGate};
use tallyline_shared_kernel::api::model::{Timestamp, UexEnvironment};
use tallyline_submission::api::model::{
    AccountState, PayloadHash, SendHoldReason, SendReadiness, SendReferenceState, SendReturnReason,
    SubmissionAcceptance, SubmissionChange, SubmissionCommand, SubmissionJob,
};
use tallyline_submission::api::{CredentialApplication, SubmissionPreflight};

/// A closed reason preventing write-ahead persistence or dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManualDispatchError {
    /// Independent current account or UEX acceptance facts hold this fixed job.
    Held(SendHoldReason),
    /// Changed observation metadata requires renewed manual release.
    Returned(SendReturnReason),
    /// Complete numerical, history, privacy or payload checks require renewed review.
    Blocked(Vec<ReleaseBlockReason>),
    /// The source job, Report or committed references changed.
    Conflict,
    /// The supplied snapshot or prepared metadata is inconsistent.
    Invalid,
    /// Durable state could not be read or committed.
    Unavailable,
}
/// Secret-free preparation minted from actual this-session credential authority and full review.
#[derive(Clone)]
pub struct ManualDispatch {
    source: SubmissionJob,
    gate: SubmissionGate,
    account: AccountState,
    payload_hash: PayloadHash,
    date_added: Timestamp,
    token_admission: tallyline_submission::api::model::TokenAdmission,
}
impl ManualDispatch {
    /// Checks current session authority and the complete exact queued Report before persistence.
    ///
    /// # Errors
    /// Refuses remembered-only accounts, changed fixed context and every current send block.
    pub fn prepare(
        job: &SubmissionJob,
        gate: &SubmissionGate,
        credentials: &CredentialApplication,
        payload_hash: PayloadHash,
        date_added: Timestamp,
    ) -> Result<Self, ManualDispatchError> {
        if credentials
            .credentials_for(&job.request().header().account)
            .is_none()
        {
            let reason = match credentials.state() {
                AccountState::Validated { username, .. }
                    if username != &job.request().header().account =>
                {
                    SendHoldReason::AccountChanged
                }
                AccountState::Unconfigured
                | AccountState::Rejected(_)
                | AccountState::Validated { .. } => SendHoldReason::AccountUnavailable,
            };
            return Err(ManualDispatchError::Held(reason));
        }
        let binding = gate
            .report()
            .release_binding()
            .ok_or(ManualDispatchError::Invalid)?;
        if binding.job() != Some(job.id())
            || binding.account() != job.request().header().account
            || binding.mode() != job.request().header().mode
            || binding.environment() != job.request().header().environment
        {
            return Err(ManualDispatchError::Invalid);
        }
        let candidate = crate::submission_request::manual_submission_request(
            gate.report(),
            binding.environment(),
            binding.account(),
            binding.mode(),
        )
        .map_err(|_| ManualDispatchError::Invalid)?;
        let mut header = candidate.header().clone();
        header.report_version = job.request().header().report_version;
        if &header != job.request().header()
            || candidate.rows() != job.request().rows()
            || candidate.newer_observation_confirmations()
                != job.request().newer_observation_confirmations()
        {
            return Err(ManualDispatchError::Invalid);
        }
        let plan = Self {
            source: job.clone(),
            gate: gate.clone(),
            account: credentials.state().clone(),
            payload_hash,
            date_added,
            token_admission: credentials
                .token_admission(None)
                .cloned()
                .unwrap_or_default(),
        };
        plan.check(gate)?;
        plan.change(gate.checked_at())?;
        Ok(plan)
    }
    /// Binds the exact selected token through the credential application's protected association.
    #[must_use]
    pub fn with_selected_token(
        mut self,
        credentials: &CredentialApplication,
        release_token: Option<&str>,
    ) -> Self {
        self.token_admission = credentials
            .token_admission(release_token)
            .cloned()
            .unwrap_or_default();
        self
    }
    /// Returns immutable nonsecret token identity for this write-ahead attempt.
    #[must_use]
    pub const fn token_scope(&self) -> Option<tallyline_submission::api::model::TokenScope> {
        self.token_admission.known_scope()
    }
    /// Returns the current protected token distinction for atomic admission only.
    #[must_use]
    pub fn token_admission(&self) -> &tallyline_submission::api::model::TokenAdmission {
        &self.token_admission
    }
    fn check(&self, gate: &SubmissionGate) -> Result<(), ManualDispatchError> {
        match assess_manual_send_context(
            self.source.request(),
            gate.report(),
            gate.snapshot(),
            &self.account,
            gate.age_settings(),
            gate.checked_at(),
        ) {
            SendReadiness::Ready => {}
            SendReadiness::Held(reason) => return Err(ManualDispatchError::Held(reason)),
            SendReadiness::Returned(reason) => return Err(ManualDispatchError::Returned(reason)),
        }
        let blocks = gate.blocks(gate.report());
        if !blocks.is_empty() {
            return Err(ManualDispatchError::Blocked(blocks));
        }
        Ok(())
    }
    /// Repeats actual numerical assessment and all send checks at the transaction's current time.
    ///
    /// # Errors
    /// Refuses newly stale observations, changed decisions or unresolved committed history.
    pub fn recheck(
        &self,
        now: Timestamp,
        history: &[SubmissionJob],
    ) -> Result<SubmissionGate, ManualDispatchError> {
        let gate = self.gate.reassess_at(now);
        let gate =
            crate::submission_history::assess_release_observation_history(&gate, Some(history));
        self.check(&gate)?;
        Ok(gate)
    }
    /// Borrows the exact queued job required by the optimistic transaction.
    #[must_use]
    pub const fn source(&self) -> &SubmissionJob {
        &self.source
    }
    /// Borrows the original complete gate for committed reference and source comparisons.
    #[must_use]
    pub const fn gate(&self) -> &SubmissionGate {
        &self.gate
    }
    /// Builds the write-ahead transition without performing transport work.
    ///
    /// # Errors
    /// Refuses stale phases and unsupported prepared times or payload metadata.
    pub fn change(&self, now: Timestamp) -> Result<SubmissionChange, ManualDispatchError> {
        self.source
            .apply(SubmissionCommand::Begin {
                at: now,
                payload_hash: self.payload_hash,
                date_added: self.date_added,
            })
            .map_err(|_| ManualDispatchError::Invalid)
    }
}
/// Atomic full-review and write-ahead persistence before the credential owner dispatches once.
pub trait ManualDispatchStore {
    /// Atomically returns an unsent refused review while preserving its freshly reassessed decisions.
    ///
    /// # Errors
    /// Refuses ready or stale reviews and leaves both aggregates unchanged when saving fails.
    fn return_manual_review(
        &mut self,
        job: &SubmissionJob,
        gate: &SubmissionGate,
        now: Timestamp,
    ) -> Result<SubmissionJob, ManualDispatchError>;
    /// Rechecks committed Report, references, policy, history, schedule and budget, then saves one attempt.
    ///
    /// # Errors
    /// Returns all closed refusals without speculative state or an HTTP request.
    fn begin_manual_dispatch(
        &mut self,
        plan: &ManualDispatch,
        now: Timestamp,
    ) -> Result<SubmissionJob, ManualDispatchError>;
}

impl ManualDispatch {
    /// Selects a precise return finding from a complete refused review.
    ///
    /// # Errors
    /// Refuses to return a ready report through the review-only transition.
    pub fn review_return_reason(
        gate: &SubmissionGate,
    ) -> Result<SendReturnReason, ManualDispatchError> {
        let blocks = gate.blocks(gate.report());
        if blocks.is_empty() {
            return Err(ManualDispatchError::Invalid);
        }
        for (block, reason) in [
            (
                ReleaseBlockReason::GameVersionChanged,
                SendReturnReason::GameVersionChanged,
            ),
            (
                ReleaseBlockReason::ObservationTooOld,
                SendReturnReason::ObservationTooOld,
            ),
            (
                ReleaseBlockReason::ObservationInFuture,
                SendReturnReason::ObservationInFuture,
            ),
            (
                ReleaseBlockReason::ObservationAgeUnconfirmed,
                SendReturnReason::ObservationAgeUnconfirmed,
            ),
        ] {
            if blocks.contains(&block) {
                return Ok(reason);
            }
        }
        Ok(SendReturnReason::ReviewRequired)
    }
}

/// Assesses independently dated account, target, version, acceptance and age context.
/// Complete numerical/evidence review, session authority and atomic write-ahead remain required.
#[must_use]
pub fn assess_manual_send_context(
    request: &tallyline_submission::api::model::SubmissionRequest,
    report: &tallyline_reporting::api::model::Report,
    snapshot: &tallyline_reference_data::api::model::ReferenceSnapshot,
    account: &AccountState,
    age: tallyline_shared_kernel::api::model::ObservationAgeSettings,
    now: Timestamp,
) -> SendReadiness {
    let references = snapshot
        .game_versions()
        .zip(snapshot.parameters())
        .and_then(|(versions, parameters)| {
            Some(SendReferenceState {
                environment: snapshot.environment(),
                version: match snapshot.environment() {
                    UexEnvironment::Live => Some(versions.live.clone()),
                    UexEnvironment::Ptu => versions.ptu.clone(),
                },
                versions_fetched_at: snapshot
                    .fetched_at(tallyline_reference_data::api::model::Endpoint::GameVersions)?,
                parameters_fetched_at: snapshot
                    .fetched_at(tallyline_reference_data::api::model::Endpoint::Parameters)?,
                acceptance: if parameters.acceptance().allows(snapshot.environment()) {
                    SubmissionAcceptance::Open
                } else {
                    SubmissionAcceptance::Closed
                },
            })
        });
    SubmissionPreflight::evaluate(
        request,
        account,
        references.as_ref(),
        report.age_confirmation(),
        age,
        now,
    )
}
