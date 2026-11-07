// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Complete manual release checks tied to real numeric review and exact context.

use crate::api::model::ObservationHistoryFinding;
use crate::api::model::{
    AttachmentBinding, AttachmentTime, ImageHash, ManualReview, NumericBlock, Report,
    ReportReleaseBinding, ReportState,
};
use crate::api::{ManualReviewer, RepositoryError};
use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_shared_kernel::api::model::SubmissionJobId;
use tallyline_shared_kernel::api::model::{
    AgeValidity, ObservationAgeSettings, RowId, SubmissionMode, Timestamp, TradeSide,
    UexEnvironment,
};

/// Accepted account identity supplied by the credential-owning workflow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseAccountStanding {
    /// No accepted identity can bind this release.
    Unavailable,
    /// Previously accepted account identity fixes an offline release target.
    Accepted(String),
}
/// Current account-scoped screenshot policy, independent of release mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseScreenshotPolicy {
    /// The policy record could not be read safely.
    Unavailable,
    /// No actual response established a screenshot requirement.
    Unknown,
    /// An actual response established that this account requires evidence.
    Required,
}
/// Result of the workflow's private working-copy integrity check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseWorkingCopy {
    /// The Report has no selected attachment.
    Absent,
    /// Reading or verifying the selected private copy failed.
    Unavailable,
    /// The exact binding and stored private content passed integrity verification.
    Verified {
        /// Hash and geometry of the confirmed source selection.
        binding: AttachmentBinding,
        /// Hash of the verified persisted private region.
        digest: ImageHash,
    },
}
/// Current release facts, without credentials, original images or paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseContext {
    account: ReleaseAccountStanding,
    mode: SubmissionMode,
    screenshot: ReleaseScreenshotPolicy,
    working_copy: ReleaseWorkingCopy,
    running_channel: Option<GameEnvironment>,
}
impl ReleaseContext {
    /// Captures current account standing, chosen mode, policy and verified private-copy facts.
    #[must_use]
    pub const fn new(
        account: ReleaseAccountStanding,
        mode: SubmissionMode,
        screenshot: ReleaseScreenshotPolicy,
        working_copy: ReleaseWorkingCopy,
    ) -> Self {
        Self {
            account,
            mode,
            screenshot,
            working_copy,
            running_channel: None,
        }
    }
    /// Adds a channel observed from the running game without guessing when it is unknown.
    #[must_use]
    pub fn with_running_channel(&self, channel: GameEnvironment) -> Self {
        let mut next = self.clone();
        next.running_channel = Some(channel);
        next
    }
}
/// An actionable reason preventing manual release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseBlockReason {
    /// Positive original receipt evidence prevents releasing this existing duplicate.
    ResolvedDuplicateOrigin {
        /// The original uncertain submission with positive evidence.
        submission: SubmissionJobId,
    },
    /// The gate belongs to another exact Report snapshot.
    StaleReview,
    /// The Report phase cannot perform this command.
    InvalidState,
    /// A mandatory numeric value or explicit decision is unresolved.
    Numeric(NumericBlock),
    /// The entered row count exceeds the protocol limit; no rows may be trimmed.
    TooManyRows,
    /// The selected mapping and reference environment differ.
    ReferenceMismatch,
    /// Exact numeric assessment could not complete.
    AssessmentUnavailable,
    /// No accepted account identity can bind the release.
    AccountUnavailable,
    /// The account's screenshot policy could not be read.
    ScreenshotPolicyUnavailable,
    /// The current account requires an attachment.
    ImageRequired,
    /// The exact full-size private preview has not been confirmed.
    PreviewUnconfirmed,
    /// Attachment time remains uncertain.
    ObservationTimeUncertain,
    /// The exact persisted private region could not be verified.
    WorkingCopyUnavailable,
    /// The terminal is absent from the selected environment's vocabulary.
    UnknownTerminal,
    /// An entered commodity is absent from the selected vocabulary.
    UnknownCommodity(RowId),
    /// The observed quantity is fractional or exceeds the supported whole-unit range.
    UnsupportedStock(RowId),
    /// No current status vocabulary is available.
    StatusVocabularyUnavailable,
    /// An entered status is absent from its side's vocabulary.
    UnknownStatus(RowId),
    /// The capture or selected environment has no known version.
    GameVersionUnavailable,
    /// Historical version evidence is unavailable for an attachment older than the observed version fetch.
    VersionHistoryUnavailable,
    /// The chosen capture version differs from the newest observed target version.
    GameVersionChanged,
    /// Observation age requires a fresh explicit decision.
    ObservationAgeUnconfirmed,
    /// The fixed observation-age hard limit expired.
    ObservationTooOld,
    /// The observation is ahead of the current wall clock.
    ObservationInFuture,
    /// The running game's mapped environment differs without a confirmation.
    EnvironmentUnconfirmed,
    /// Complete committed observation history has not been assessed for this exact scope.
    ObservationHistoryUnavailable,
    /// This exact older row needs a decision against the newest accepted observation.
    NewerObservationSent {
        /// Candidate local row.
        row: RowId,
        /// Newest accepted original observation.
        observed_at: Timestamp,
    },
    /// A newer uncertain row cannot be bypassed by confirmation.
    UnresolvedNewerObservation {
        /// Candidate local row.
        row: RowId,
        /// Newer uncertain submission.
        submission: tallyline_shared_kernel::api::model::SubmissionJobId,
        /// Newer original observation.
        observed_at: Timestamp,
    },
    /// A decision no longer has matching committed accepted evidence.
    StaleObservationDecision(RowId),
}
/// Fresh real numeric review and complete release facts for one exact Report.
#[derive(Clone)]
pub struct SubmissionGate {
    reviewer: ManualReviewer,
    source: Report,
    report: Report,
    review: Option<ManualReview>,
    error: Option<RepositoryError>,
    snapshot: ReferenceSnapshot,
    context: ReleaseContext,
    age_settings: ObservationAgeSettings,
    now: Timestamp,
    running_mismatch: bool,
    history: Option<crate::api::model::ObservationHistoryAssessment>,
}
impl ManualReviewer {
    /// Builds a complete gate from actual reassessment, preserving all independent blocks.
    #[must_use]
    pub fn submission_gate(
        &self,
        report: Report,
        snapshot: &ReferenceSnapshot,
        context: ReleaseContext,
        age_settings: ObservationAgeSettings,
        now: Timestamp,
    ) -> SubmissionGate {
        let running_mismatch = context
            .running_channel
            .is_some_and(|channel| self.environment(channel) != snapshot.environment());
        let (reviewed, review, error) = match self.reassess(report.clone(), snapshot, now) {
            Ok(review) => (review.report().clone(), Some(review), None),
            Err(error) => (report.clone(), None, Some(error)),
        };
        SubmissionGate {
            reviewer: self.clone(),
            source: report,
            report: reviewed,
            review,
            error,
            snapshot: snapshot.clone(),
            context,
            age_settings,
            now,
            running_mismatch,
            history: None,
        }
    }
}
impl SubmissionGate {
    /// Repeats actual review at a new current time; history must be freshly assessed by workflows.
    #[must_use]
    pub fn reassess_at(&self, now: Timestamp) -> Self {
        self.reviewer.submission_gate(
            self.source.clone(),
            &self.snapshot,
            self.context.clone(),
            self.age_settings,
            now,
        )
    }
    /// Returns the exact time used for this complete assessment.
    #[must_use]
    pub const fn checked_at(&self) -> Timestamp {
        self.now
    }
    /// Returns the tighten-only observation-age policy used for this assessment.
    #[must_use]
    pub const fn age_settings(&self) -> ObservationAgeSettings {
        self.age_settings
    }
    /// Borrows the exact source that must still match at the atomic save.
    #[must_use]
    pub const fn source(&self) -> &Report {
        &self.source
    }
    /// Borrows the reassessed Report authorised by this gate only when every block is absent.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }
    /// Borrows real numeric results when exact reassessment completed.
    #[must_use]
    pub const fn review(&self) -> Option<&ManualReview> {
        self.review.as_ref()
    }
    /// Borrows the reference values that must remain current at persistence.
    #[must_use]
    pub const fn snapshot(&self) -> &ReferenceSnapshot {
        &self.snapshot
    }
    /// Returns every independent block in stable order without changing values or approvals.
    #[must_use]
    pub fn blocks(&self, report: &Report) -> Vec<ReleaseBlockReason> {
        if report != &self.report {
            return vec![ReleaseBlockReason::StaleReview];
        }
        let mut blocks = crate::numeric_readiness::blocks(report)
            .into_iter()
            .map(ReleaseBlockReason::Numeric)
            .collect::<Vec<_>>();
        if report.rows().iter().filter(|row| row.entered()).count()
            > tallyline_shared_kernel::api::model::MAX_SUBMISSION_ROWS
        {
            blocks.push(ReleaseBlockReason::TooManyRows);
        }
        match &self.history {
            Some(history)
                if history.matches(
                    report,
                    self.accepted_account(),
                    self.mode(),
                    self.snapshot.environment(),
                ) =>
            {
                for finding in history.findings() {
                    blocks.push(match *finding {
                        ObservationHistoryFinding::ResolvedDuplicateOrigin { submission } => {
                            ReleaseBlockReason::ResolvedDuplicateOrigin { submission }
                        }
                        crate::api::model::ObservationHistoryFinding::NewerAccepted {
                            row,
                            observed_at,
                        } => ReleaseBlockReason::NewerObservationSent { row, observed_at },
                        crate::api::model::ObservationHistoryFinding::Unresolved {
                            row,
                            submission,
                            observed_at,
                        } => ReleaseBlockReason::UnresolvedNewerObservation {
                            row,
                            submission,
                            observed_at,
                        },
                        crate::api::model::ObservationHistoryFinding::StaleDecision { row } => {
                            ReleaseBlockReason::StaleObservationDecision(row)
                        }
                    });
                }
            }
            _ => blocks.push(ReleaseBlockReason::ObservationHistoryUnavailable),
        }
        if let Some(error) = self.error {
            blocks.push(match error {
                RepositoryError::Invalid => ReleaseBlockReason::ReferenceMismatch,
                RepositoryError::AssessmentUnavailable
                | RepositoryError::Conflict
                | RepositoryError::Unavailable => ReleaseBlockReason::AssessmentUnavailable,
            });
        }
        if !matches!(&self.context.account, ReleaseAccountStanding::Accepted(account) if !account.trim().is_empty())
        {
            blocks.push(ReleaseBlockReason::AccountUnavailable);
        }
        match self.context.screenshot {
            ReleaseScreenshotPolicy::Unavailable => {
                blocks.push(ReleaseBlockReason::ScreenshotPolicyUnavailable)
            }
            ReleaseScreenshotPolicy::Required if report.attachment().is_none() => {
                blocks.push(ReleaseBlockReason::ImageRequired)
            }
            ReleaseScreenshotPolicy::Required | ReleaseScreenshotPolicy::Unknown => {
                if report.attachment().is_none()
                    && self.context.working_copy != ReleaseWorkingCopy::Absent
                {
                    blocks.push(ReleaseBlockReason::WorkingCopyUnavailable);
                }
            }
        }
        if let Some(attachment) = report.attachment() {
            if self
                .snapshot
                .fetched_at(tallyline_reference_data::api::model::Endpoint::GameVersions)
                .is_some_and(|fetched| report.observed_at() < fetched)
            {
                blocks.push(ReleaseBlockReason::VersionHistoryUnavailable);
            }
            if !attachment.preview_confirmed() {
                blocks.push(ReleaseBlockReason::PreviewUnconfirmed);
            }
            if matches!(attachment.time(), AttachmentTime::ImportedFile(_)) {
                blocks.push(ReleaseBlockReason::ObservationTimeUncertain);
            }
            if !matches!(self.context.working_copy, ReleaseWorkingCopy::Verified { binding, digest } if binding == attachment.binding() && Some(digest) == attachment.working_copy())
            {
                blocks.push(ReleaseBlockReason::WorkingCopyUnavailable);
            }
        }
        if !self
            .snapshot
            .terminals("")
            .iter()
            .any(|terminal| terminal.id() == report.key().terminal())
        {
            blocks.push(ReleaseBlockReason::UnknownTerminal);
        }
        let commodities = self.snapshot.commodities("");
        let statuses = self
            .snapshot
            .status_levels()
            .map(|levels| match report.key().side() {
                TradeSide::Buy => &levels.buy,
                TradeSide::Sell => &levels.sell,
            });
        if statuses.is_none() {
            blocks.push(ReleaseBlockReason::StatusVocabularyUnavailable);
        }
        for row in report.rows().iter().filter(|row| row.entered()) {
            if row
                .stock()
                .is_some_and(|stock| stock.whole_units().is_none())
            {
                blocks.push(ReleaseBlockReason::UnsupportedStock(row.id()));
            }
            if !commodities
                .iter()
                .any(|commodity| commodity.id() == row.commodity())
            {
                blocks.push(ReleaseBlockReason::UnknownCommodity(row.id()));
            }
            if let Some(status) = row.status()
                && statuses.is_some_and(|levels| !levels.iter().any(|level| level.code == status))
            {
                blocks.push(ReleaseBlockReason::UnknownStatus(row.id()));
            }
        }
        let version =
            self.snapshot
                .game_versions()
                .and_then(|versions| match self.snapshot.environment() {
                    UexEnvironment::Live => Some(versions.live.as_str()),
                    UexEnvironment::Ptu => versions.ptu.as_deref(),
                });
        let version_block = match (report.game_version(), version) {
            (Some(chosen), Some(current)) if chosen == current && !current.trim().is_empty() => {
                None
            }
            (Some(_), Some(current)) if !current.trim().is_empty() => {
                Some(ReleaseBlockReason::GameVersionChanged)
            }
            _ => Some(ReleaseBlockReason::GameVersionUnavailable),
        };
        blocks.extend(version_block);
        let age_block = match self.age_settings.assess(
            report.observed_at(),
            report.age_confirmation(),
            self.now,
        ) {
            AgeValidity::Fresh | AgeValidity::Confirmed => None,
            AgeValidity::Unconfirmed => Some(ReleaseBlockReason::ObservationAgeUnconfirmed),
            AgeValidity::TooOld => Some(ReleaseBlockReason::ObservationTooOld),
            AgeValidity::Future => Some(ReleaseBlockReason::ObservationInFuture),
        };
        blocks.extend(age_block);
        if self.running_mismatch {
            blocks.push(ReleaseBlockReason::EnvironmentUnconfirmed);
        }
        if !matches!(
            report.state(),
            ReportState::Draft
                | ReportState::Released
                | ReportState::Queued
                | ReportState::WaitingForCooldown(_)
        ) {
            blocks.push(ReleaseBlockReason::InvalidState);
        }
        blocks
    }
    pub(crate) fn binding(
        &self,
        version: u64,
    ) -> Result<ReportReleaseBinding, crate::api::model::InvalidReport> {
        let ReleaseAccountStanding::Accepted(account) = &self.context.account else {
            return Err(crate::api::model::InvalidReport);
        };
        ReportReleaseBinding::new(
            account.clone(),
            self.context.mode,
            self.snapshot.environment(),
            version,
        )
    }
}

impl SubmissionGate {
    /// Borrows the accepted fixed identity without implying this-session HTTP authority.
    #[must_use]
    pub fn accepted_account(&self) -> Option<&str> {
        match &self.context.account {
            ReleaseAccountStanding::Accepted(account) if !account.trim().is_empty() => {
                Some(account)
            }
            ReleaseAccountStanding::Unavailable | ReleaseAccountStanding::Accepted(_) => None,
        }
    }
    /// Returns the chosen release mode.
    #[must_use]
    pub const fn mode(&self) -> SubmissionMode {
        self.context.mode
    }
    /// Binds a workflow's assessed history to this immutable gate; absent or stale facts stay closed.
    #[must_use]
    pub fn with_observation_history(
        &self,
        history: Option<crate::api::model::ObservationHistoryAssessment>,
    ) -> Self {
        let mut next = self.clone();
        next.history = history;
        next
    }
}
