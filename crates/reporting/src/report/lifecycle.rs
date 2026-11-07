// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable Report release bindings and lifecycle transitions.

mod cooldown_split;
pub use cooldown_split::ReportSplit;

use super::{InvalidReport, Report, ReportId};
use crate::api::model::{ReleaseBlockReason, SubmissionGate};
use tallyline_shared_kernel::api::model::{
    AgeConfirmation, EventId, EventSource, SubmissionJobId, SubmissionMode, Timestamp,
    UexEnvironment,
};

/// Durable lifecycle of one Report, independent of transport job phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReportState {
    /// User-editable values awaiting complete release checks.
    #[default]
    Draft,
    /// Complete checks passed and the fixed release binding was established.
    Released,
    /// An immutable submission job owns the released values.
    Queued,
    /// The complete Report waits for its row cooldowns.
    WaitingForCooldown(Timestamp),
    /// UEX accepted every row, including successful test validation.
    Submitted,
    /// Some rows may have been accepted and require individual resolution.
    PartiallyAccepted,
    /// Processing cannot be excluded and automatic replay is closed.
    OutcomeUnknown,
    /// A permanent response ended this Report's delivery.
    Rejected,
    /// Every accepted row was withdrawn.
    Withdrawn,
    /// The user explicitly discarded the Draft.
    Discarded,
}

/// A durable reason why a Report needs user attention after being returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportFinding {
    /// A concrete Report-owned correction action is required before renewed release.
    ReturnedForFix(super::fixable_reason::FixableReason),
    /// A duplicate provides evidence of an earlier uncertain receipt.
    PossiblyAlreadyReceived,
    /// Restart closed unfinished work that was not sending.
    AppRestart,
    /// Observation age requires renewed explicit approval.
    ObservationAgeUnconfirmed,
    /// The fixed observation-age limit expired.
    ObservationTooOld,
    /// Observation time is ahead of the current clock.
    ObservationInFuture,
    /// The captured version no longer matches the accepted target version.
    GameVersionChanged,
    /// A newer observation already reached or may have reached UEX.
    NewerObservationSent,
    /// The account requires confirmed private screenshot evidence.
    ScreenshotRequired,
    /// UEX refused the attempted observation time.
    InvalidObservationTime,
    /// Current values, decisions or evidence need renewed review.
    ReviewRequired,
}

/// Account, mode and environment fixed by a successful release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportReleaseBinding {
    account: String,
    mode: SubmissionMode,
    environment: UexEnvironment,
    release_version: u64,
    job: Option<SubmissionJobId>,
}
impl ReportReleaseBinding {
    pub(crate) fn new(
        account: String,
        mode: SubmissionMode,
        environment: UexEnvironment,
        release_version: u64,
    ) -> Result<Self, InvalidReport> {
        if account.trim().is_empty() || release_version == 0 {
            return Err(InvalidReport);
        }
        Ok(Self {
            account,
            mode,
            environment,
            release_version,
            job: None,
        })
    }
    /// Borrows the validated account identity selected for this release.
    #[must_use]
    pub fn account(&self) -> &str {
        &self.account
    }
    /// Returns the immutable test or production mode.
    #[must_use]
    pub const fn mode(&self) -> SubmissionMode {
        self.mode
    }
    /// Returns the release-time mapped UEX target.
    #[must_use]
    pub const fn environment(&self) -> UexEnvironment {
        self.environment
    }
    /// Returns the Report version that established this release.
    #[must_use]
    pub const fn release_version(&self) -> u64 {
        self.release_version
    }
    /// Returns the submission identity once enqueue has committed.
    #[must_use]
    pub const fn job(&self) -> Option<SubmissionJobId> {
        self.job
    }
}

/// Persistence-only immutable release metadata without credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredReportReleaseBinding {
    /// Validated account identity fixed at release.
    pub account: String,
    /// Mode fixed at release.
    pub mode: SubmissionMode,
    /// Environment fixed at release.
    pub environment: UexEnvironment,
    /// Version that established the binding.
    pub release_version: u64,
    /// The owning job, absent only before enqueue.
    pub job: Option<SubmissionJobId>,
}

/// Persistence-only Report lifecycle, validated by aggregate reconstruction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoredReportLifecycle {
    /// Current lifecycle phase.
    pub state: ReportState,
    /// Fixed release metadata, retained when an unfinished job returns to Draft.
    pub release: Option<StoredReportReleaseBinding>,
    /// Explicit age decision bound to the Report's observation.
    pub age_confirmation: Option<AgeConfirmation>,
    /// Passive reason for returned or rejected work.
    pub finding: Option<ReportFinding>,
    /// Exact value- and release-scope-bound decisions against newer accepted rows.
    pub newer_observations: Vec<super::StoredNewerObservationApproval>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ReportLifecycle {
    state: ReportState,
    release: Option<ReportReleaseBinding>,
    age_confirmation: Option<AgeConfirmation>,
    finding: Option<ReportFinding>,
    pub(super) newer_observations: Vec<super::NewerObservationApproval>,
}
impl ReportLifecycle {
    pub(super) fn restore(
        stored: StoredReportLifecycle,
        version: u64,
        observed: Timestamp,
        report: &Report,
    ) -> Result<Self, InvalidReport> {
        let release = stored
            .release
            .map(|value| {
                if value.release_version > version || value.job.is_some_and(|job| job.0 == 0) {
                    return Err(InvalidReport);
                }
                let mut binding = ReportReleaseBinding::new(
                    value.account,
                    value.mode,
                    value.environment,
                    value.release_version,
                )?;
                binding.job = value.job;
                Ok(binding)
            })
            .transpose()?;
        let valid = match stored.state {
            ReportState::Draft | ReportState::Discarded => true,
            ReportState::Released => release.as_ref().is_some_and(|value| value.job.is_none()),
            ReportState::Queued
            | ReportState::WaitingForCooldown(_)
            | ReportState::Submitted
            | ReportState::PartiallyAccepted
            | ReportState::OutcomeUnknown
            | ReportState::Rejected
            | ReportState::Withdrawn => release.as_ref().is_some_and(|value| value.job.is_some()),
        };
        if !valid
            || stored
                .age_confirmation
                .is_some_and(|value| value.observed_at() != observed)
        {
            return Err(InvalidReport);
        }
        let mut identities = std::collections::BTreeSet::new();
        let mut newer_observations = Vec::new();
        for approval in stored.newer_observations {
            let scope = (
                approval.row.0,
                approval.evidence.account().to_owned(),
                approval.evidence.mode() == SubmissionMode::Production,
                approval.evidence.environment() == UexEnvironment::Live,
            );
            if !identities.insert(scope) {
                return Err(InvalidReport);
            }
            newer_observations.push(super::NewerObservationApproval::restore(approval, report)?);
        }
        Ok(Self {
            state: stored.state,
            release,
            age_confirmation: stored.age_confirmation,
            finding: stored.finding,
            newer_observations,
        })
    }
    fn stored(&self) -> StoredReportLifecycle {
        StoredReportLifecycle {
            state: self.state,
            release: self
                .release
                .as_ref()
                .map(|value| StoredReportReleaseBinding {
                    account: value.account.clone(),
                    mode: value.mode,
                    environment: value.environment,
                    release_version: value.release_version,
                    job: value.job,
                }),
            age_confirmation: self.age_confirmation,
            finding: self.finding,
            newer_observations: self
                .newer_observations
                .iter()
                .map(super::NewerObservationApproval::stored)
                .collect(),
        }
    }
}

/// A Report lifecycle event persisted with its state transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportEventKind {
    /// Complete release checks established a fixed submission binding.
    Released,
    /// UEX accepted the complete released row set.
    Submitted,
    /// The user explicitly discarded this Draft.
    Discarded,
}
/// An immutable, credential-free event with a stable idempotence identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportEvent {
    id: EventId,
    report: ReportId,
    kind: ReportEventKind,
}
impl ReportEvent {
    /// Returns the durable outbox identity.
    #[must_use]
    pub const fn id(self) -> EventId {
        self.id
    }
    /// Returns the owning Report.
    #[must_use]
    pub const fn report(self) -> ReportId {
        self.report
    }
    /// Returns the event's closed domain meaning.
    #[must_use]
    pub const fn kind(self) -> ReportEventKind {
        self.kind
    }
}
/// An immutable lifecycle change saved atomically with its events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportChange {
    expected_version: u64,
    state: Report,
    events: Vec<ReportEvent>,
}
impl ReportChange {
    pub(super) fn new(
        expected_version: u64,
        state: Report,
        kind: Option<ReportEventKind>,
    ) -> Result<Self, InvalidReport> {
        let events = kind
            .map(|kind| {
                EventId::new(EventSource::Reporting, state.id.0, state.version)
                    .map(|id| ReportEvent {
                        id,
                        report: state.id,
                        kind,
                    })
                    .ok_or(InvalidReport)
            })
            .transpose()?
            .into_iter()
            .collect();
        Ok(Self {
            expected_version,
            state,
            events,
        })
    }
    /// Returns the version against which the first transition was prepared.
    #[must_use]
    pub const fn expected_version(&self) -> u64 {
        self.expected_version
    }
    /// Borrows the new immutable Report.
    #[must_use]
    pub const fn state(&self) -> &Report {
        &self.state
    }
    /// Borrows events that must share the state transaction.
    #[must_use]
    pub fn events(&self) -> &[ReportEvent] {
        &self.events
    }
    /// Takes the state after its transaction has committed.
    #[must_use]
    pub fn into_state(self) -> Report {
        self.state
    }
}
impl Report {
    /// Returns the durable Report phase.
    #[must_use]
    pub const fn state(&self) -> ReportState {
        self.lifecycle.state
    }
    /// Borrows the last fixed release binding without exposing credentials.
    #[must_use]
    pub const fn release_binding(&self) -> Option<&ReportReleaseBinding> {
        self.lifecycle.release.as_ref()
    }
    /// Returns the explicit age decision for this observation.
    #[must_use]
    pub const fn age_confirmation(&self) -> Option<AgeConfirmation> {
        self.lifecycle.age_confirmation
    }
    /// Returns the passive reason for returned work.
    #[must_use]
    pub const fn finding(&self) -> Option<ReportFinding> {
        self.lifecycle.finding
    }
    /// Exports validated lifecycle values for the persistence mapper.
    #[must_use]
    pub fn stored_lifecycle(&self) -> StoredReportLifecycle {
        self.lifecycle.stored()
    }
    pub(super) fn ensure_draft(&self) -> Result<(), InvalidReport> {
        if self.state() != ReportState::Draft {
            return Err(InvalidReport);
        }
        Ok(())
    }
    pub(super) fn advanced(&self) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        next.version = self.version.checked_add(1).ok_or(InvalidReport)?;
        Ok(next)
    }
    /// Releases only the exact fresh Report approved by the complete gate.
    ///
    /// # Errors
    /// Returns every release block and refuses another release of non-Draft work.
    pub fn release(&self, gate: &SubmissionGate) -> Result<ReportChange, Vec<ReleaseBlockReason>> {
        let mut blocks = gate.blocks(self);
        if self.state() != ReportState::Draft {
            blocks.push(ReleaseBlockReason::InvalidState);
        }
        if !blocks.is_empty() {
            return Err(blocks);
        }
        let mut next = self
            .advanced()
            .map_err(|_| vec![ReleaseBlockReason::InvalidState])?;
        next.lifecycle.release = Some(
            gate.binding(next.version)
                .map_err(|_| vec![ReleaseBlockReason::AccountUnavailable])?,
        );
        next.lifecycle.state = ReportState::Released;
        next.lifecycle.finding = None;
        ReportChange::new(self.version, next, Some(ReportEventKind::Released))
            .map_err(|_| vec![ReleaseBlockReason::InvalidState])
    }
    /// Binds exactly one queued job without changing any released value.
    ///
    /// # Errors
    /// Refuses missing identities, another job binding or an invalid Report phase.
    pub fn enqueue(&self, job: SubmissionJobId) -> Result<ReportChange, InvalidReport> {
        if job.0 == 0 {
            return Err(InvalidReport);
        }
        if self.state() == ReportState::Queued
            && self.release_binding().and_then(ReportReleaseBinding::job) == Some(job)
        {
            return ReportChange::new(self.version, self.clone(), None);
        }
        if self.state() != ReportState::Released {
            return Err(InvalidReport);
        }
        let mut next = self.advanced()?;
        next.lifecycle.release.as_mut().ok_or(InvalidReport)?.job = Some(job);
        next.lifecycle.state = ReportState::Queued;
        ReportChange::new(self.version, next, None)
    }
}

/// A delivery fact supplied by the coordinating workflow after a job transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportDelivery {
    /// Every released row was accepted, with optional later receipt enrichment.
    FullAcceptance,
    /// Some released rows may have been accepted.
    PartialAcceptance,
    /// Processing cannot safely be excluded.
    Unknown,
    /// A permanent refusal ended the released delivery.
    Rejected,
    /// Unsent work requires a renewed manual release.
    Return(ReportFinding),
    /// The immutable release waits until its latest row cooldown.
    Cooldown(Timestamp),
}
impl Report {
    /// Applies a committed delivery fact for this Report's exact owning job.
    ///
    /// # Errors
    /// Refuses another job, editable work or a conflicting terminal outcome.
    pub fn apply_delivery(
        &self,
        job: SubmissionJobId,
        delivery: ReportDelivery,
    ) -> Result<ReportChange, InvalidReport> {
        if self.release_binding().and_then(ReportReleaseBinding::job) != Some(job) {
            return Err(InvalidReport);
        }
        let (state, finding) = match delivery {
            ReportDelivery::FullAcceptance => (ReportState::Submitted, None),
            ReportDelivery::PartialAcceptance => (ReportState::PartiallyAccepted, None),
            ReportDelivery::Unknown => (ReportState::OutcomeUnknown, None),
            ReportDelivery::Rejected => (ReportState::Rejected, None),
            ReportDelivery::Return(reason) => (ReportState::Draft, Some(reason)),
            ReportDelivery::Cooldown(until) => (ReportState::WaitingForCooldown(until), None),
        };
        if self.state() == state && self.finding() == finding {
            return ReportChange::new(self.version, self.clone(), None);
        }
        if !matches!(
            self.state(),
            ReportState::Queued | ReportState::WaitingForCooldown(_)
        ) {
            return Err(InvalidReport);
        }
        let mut next = self.advanced()?;
        next.lifecycle.state = state;
        next.lifecycle.finding = finding;
        if matches!(delivery, ReportDelivery::Return(_)) {
            next.lifecycle.age_confirmation = None;
        }
        let event = (state == ReportState::Submitted).then_some(ReportEventKind::Submitted);
        ReportChange::new(self.version, next, event)
    }
}

impl Report {
    /// Records an explicit age decision bound to this Draft's unchanged observation.
    ///
    /// # Errors
    /// Refuses released work, future observations and observations beyond the fixed hard limit.
    pub fn confirm_observation_age(&self, now: Timestamp) -> Result<ReportChange, InvalidReport> {
        self.ensure_draft()?;
        if matches!(
            tallyline_shared_kernel::api::model::ObservationAgeSettings::default().assess(
                self.observed_at,
                None,
                now
            ),
            tallyline_shared_kernel::api::model::AgeValidity::Future
                | tallyline_shared_kernel::api::model::AgeValidity::TooOld
        ) {
            return Err(InvalidReport);
        }
        let confirmation =
            AgeConfirmation::new(self.observed_at, now).map_err(|_| InvalidReport)?;
        if self.age_confirmation() == Some(confirmation) {
            return ReportChange::new(self.version, self.clone(), None);
        }
        let mut next = self.advanced()?;
        next.lifecycle.age_confirmation = Some(confirmation);
        ReportChange::new(self.version, next, None)
    }
    pub(super) fn revoke_observation_age(&mut self) {
        self.lifecycle.age_confirmation = None;
    }
}

impl Report {
    /// Applies explicit receipt resolution for the exact immutable owning job.
    ///
    /// # Errors
    /// Refuses another job, ordinary delivery events and conflicting final outcomes.
    pub fn resolve_delivery(
        &self,
        job: SubmissionJobId,
        delivery: ReportDelivery,
    ) -> Result<ReportChange, InvalidReport> {
        if self.release_binding().and_then(ReportReleaseBinding::job) != Some(job) {
            return Err(InvalidReport);
        }
        let state = match delivery {
            ReportDelivery::FullAcceptance => ReportState::Submitted,
            ReportDelivery::PartialAcceptance => ReportState::PartiallyAccepted,
            ReportDelivery::Unknown
            | ReportDelivery::Rejected
            | ReportDelivery::Return(_)
            | ReportDelivery::Cooldown(_) => return Err(InvalidReport),
        };
        if self.state() == state {
            return ReportChange::new(self.version, self.clone(), None);
        }
        if !matches!(
            self.state(),
            ReportState::OutcomeUnknown | ReportState::PartiallyAccepted
        ) {
            return Err(InvalidReport);
        }
        let mut next = self.advanced()?;
        next.lifecycle.state = state;
        ReportChange::new(
            self.version,
            next,
            (state == ReportState::Submitted).then_some(ReportEventKind::Submitted),
        )
    }
}
