// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact credential-free Report lifecycle document mapping.

use crate::report_fixable_reason::FixableDocument;
use serde::{Deserialize, Serialize};
use tallyline_reporting::api::RepositoryError;
use tallyline_reporting::api::model::{
    ReportFinding, ReportState, StoredReportLifecycle, StoredReportReleaseBinding,
};
use tallyline_shared_kernel::api::model::{
    AgeConfirmation, SubmissionJobId, SubmissionMode, UexEnvironment,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LifecycleDocument {
    state: StateDocument,
    release: Option<ReleaseDocument>,
    age: Option<[String; 2]>,
    finding: Option<FindingDocument>,
    newer_observations: Vec<NewerDocument>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StateDocument {
    Draft,
    Released,
    Queued,
    Submitted,
    PartiallyAccepted,
    OutcomeUnknown,
    Rejected,
    Withdrawn,
    Discarded,
    WaitingForCooldown(String),
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FindingDocument {
    ReturnedForFix(FixableDocument),
    PossiblyAlreadyReceived,
    AppRestart,
    ObservationAgeUnconfirmed,
    ObservationTooOld,
    ObservationInFuture,
    GameVersionChanged,
    NewerObservationSent,
    ScreenshotRequired,
    InvalidObservationTime,
    ReviewRequired,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseDocument {
    account: String,
    mode: ModeDocument,
    environment: EnvironmentDocument,
    release_version: u64,
    job: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ModeDocument {
    DryRun,
    Production,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EnvironmentDocument {
    Live,
    Ptu,
}
impl From<StoredReportLifecycle> for LifecycleDocument {
    fn from(value: StoredReportLifecycle) -> Self {
        Self {
            newer_observations: value
                .newer_observations
                .into_iter()
                .map(NewerDocument::from)
                .collect(),
            state: match value.state {
                ReportState::Draft => StateDocument::Draft,
                ReportState::Released => StateDocument::Released,
                ReportState::Queued => StateDocument::Queued,
                ReportState::Submitted => StateDocument::Submitted,
                ReportState::PartiallyAccepted => StateDocument::PartiallyAccepted,
                ReportState::OutcomeUnknown => StateDocument::OutcomeUnknown,
                ReportState::Rejected => StateDocument::Rejected,
                ReportState::Withdrawn => StateDocument::Withdrawn,
                ReportState::Discarded => StateDocument::Discarded,
                ReportState::WaitingForCooldown(until) => {
                    StateDocument::WaitingForCooldown(until.millis().to_string())
                }
            },
            release: value.release.map(|release| ReleaseDocument {
                account: release.account,
                mode: match release.mode {
                    SubmissionMode::DryRun => ModeDocument::DryRun,
                    SubmissionMode::Production => ModeDocument::Production,
                },
                environment: match release.environment {
                    UexEnvironment::Live => EnvironmentDocument::Live,
                    UexEnvironment::Ptu => EnvironmentDocument::Ptu,
                },
                release_version: release.release_version,
                job: release.job.map(|job| job.0),
            }),
            age: value.age_confirmation.map(|age| {
                [
                    age.observed_at().millis().to_string(),
                    age.confirmed_at().millis().to_string(),
                ]
            }),
            finding: value.finding.map(|finding| match finding {
                ReportFinding::PossiblyAlreadyReceived => FindingDocument::PossiblyAlreadyReceived,
                ReportFinding::AppRestart => FindingDocument::AppRestart,
                ReportFinding::ObservationAgeUnconfirmed => {
                    FindingDocument::ObservationAgeUnconfirmed
                }
                ReportFinding::ObservationTooOld => FindingDocument::ObservationTooOld,
                ReportFinding::ObservationInFuture => FindingDocument::ObservationInFuture,
                ReportFinding::GameVersionChanged => FindingDocument::GameVersionChanged,
                ReportFinding::NewerObservationSent => FindingDocument::NewerObservationSent,
                ReportFinding::ScreenshotRequired => FindingDocument::ScreenshotRequired,
                ReportFinding::InvalidObservationTime => FindingDocument::InvalidObservationTime,
                ReportFinding::ReturnedForFix(reason) => {
                    FindingDocument::ReturnedForFix(reason.into())
                }
                ReportFinding::ReviewRequired => FindingDocument::ReviewRequired,
            }),
        }
    }
}
impl LifecycleDocument {
    pub(crate) fn restore(self) -> Result<StoredReportLifecycle, RepositoryError> {
        Ok(StoredReportLifecycle {
            newer_observations: self
                .newer_observations
                .into_iter()
                .map(NewerDocument::restore)
                .collect::<Result<_, _>>()?,
            state: match self.state {
                StateDocument::Draft => ReportState::Draft,
                StateDocument::Released => ReportState::Released,
                StateDocument::Queued => ReportState::Queued,
                StateDocument::Submitted => ReportState::Submitted,
                StateDocument::PartiallyAccepted => ReportState::PartiallyAccepted,
                StateDocument::OutcomeUnknown => ReportState::OutcomeUnknown,
                StateDocument::Rejected => ReportState::Rejected,
                StateDocument::Withdrawn => ReportState::Withdrawn,
                StateDocument::Discarded => ReportState::Discarded,
                StateDocument::WaitingForCooldown(until) => ReportState::WaitingForCooldown(
                    crate::reference_cache::timestamp(&until)
                        .map_err(|_| RepositoryError::Invalid)?,
                ),
            },
            release: self.release.map(|release| StoredReportReleaseBinding {
                account: release.account,
                mode: match release.mode {
                    ModeDocument::DryRun => SubmissionMode::DryRun,
                    ModeDocument::Production => SubmissionMode::Production,
                },
                environment: match release.environment {
                    EnvironmentDocument::Live => UexEnvironment::Live,
                    EnvironmentDocument::Ptu => UexEnvironment::Ptu,
                },
                release_version: release.release_version,
                job: release.job.map(SubmissionJobId),
            }),
            age_confirmation: self
                .age
                .map(|[observed, confirmed]| {
                    AgeConfirmation::new(
                        crate::reference_cache::timestamp(&observed)
                            .map_err(|_| RepositoryError::Invalid)?,
                        crate::reference_cache::timestamp(&confirmed)
                            .map_err(|_| RepositoryError::Invalid)?,
                    )
                    .map_err(|_| RepositoryError::Invalid)
                })
                .transpose()?,
            finding: self.finding.map(|finding| match finding {
                FindingDocument::PossiblyAlreadyReceived => ReportFinding::PossiblyAlreadyReceived,
                FindingDocument::AppRestart => ReportFinding::AppRestart,
                FindingDocument::ObservationAgeUnconfirmed => {
                    ReportFinding::ObservationAgeUnconfirmed
                }
                FindingDocument::ObservationTooOld => ReportFinding::ObservationTooOld,
                FindingDocument::ObservationInFuture => ReportFinding::ObservationInFuture,
                FindingDocument::GameVersionChanged => ReportFinding::GameVersionChanged,
                FindingDocument::NewerObservationSent => ReportFinding::NewerObservationSent,
                FindingDocument::ScreenshotRequired => ReportFinding::ScreenshotRequired,
                FindingDocument::InvalidObservationTime => ReportFinding::InvalidObservationTime,
                FindingDocument::ReturnedForFix(reason) => {
                    ReportFinding::ReturnedForFix(reason.into())
                }
                FindingDocument::ReviewRequired => ReportFinding::ReviewRequired,
            }),
        })
    }
}
pub(crate) const fn phase_name(state: ReportState) -> &'static str {
    match state {
        ReportState::Draft => "draft",
        ReportState::Released => "released",
        ReportState::Queued => "queued",
        ReportState::Submitted => "submitted",
        ReportState::PartiallyAccepted => "partially_accepted",
        ReportState::OutcomeUnknown => "outcome_unknown",
        ReportState::Rejected => "rejected",
        ReportState::Withdrawn => "withdrawn",
        ReportState::Discarded => "discarded",
        ReportState::WaitingForCooldown(_) => "waiting_for_cooldown",
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NewerDocument {
    row: u64,
    terminal: u32,
    side: crate::queue_document::SideDocument,
    environment: String,
    observed_at: String,
    commodity: u32,
    price: String,
    stock: String,
    status: std::num::NonZeroU8,
    account: String,
    mode: ModeDocument,
    target: EnvironmentDocument,
    newer_observed_at: String,
    confirmed_at: String,
}
impl From<tallyline_reporting::api::model::StoredNewerObservationApproval> for NewerDocument {
    fn from(value: tallyline_reporting::api::model::StoredNewerObservationApproval) -> Self {
        Self {
            row: value.row.0,
            terminal: value.key.terminal().0,
            side: value.key.side().into(),
            environment: crate::reports::environment_name(value.key.environment()).to_owned(),
            observed_at: value.observed_at.millis().to_string(),
            commodity: value.commodity.0,
            price: value.price.to_string(),
            stock: value.stock.to_string(),
            status: value.status,
            account: value.evidence.account().to_owned(),
            mode: match value.evidence.mode() {
                SubmissionMode::DryRun => ModeDocument::DryRun,
                SubmissionMode::Production => ModeDocument::Production,
            },
            target: match value.evidence.environment() {
                UexEnvironment::Live => EnvironmentDocument::Live,
                UexEnvironment::Ptu => EnvironmentDocument::Ptu,
            },
            newer_observed_at: value.evidence.observed_at().millis().to_string(),
            confirmed_at: value.confirmed_at.millis().to_string(),
        }
    }
}
impl NewerDocument {
    fn restore(
        self,
    ) -> Result<tallyline_reporting::api::model::StoredNewerObservationApproval, RepositoryError>
    {
        use tallyline_reporting::api::model::{
            NewerObservationEvidence, ReportKey, StoredNewerObservationApproval,
        };
        use tallyline_shared_kernel::api::model::{CommodityId, RowId, TerminalId};
        let timestamp = |value: &str| {
            crate::reference_cache::timestamp(value).map_err(|_| RepositoryError::Invalid)
        };
        Ok(StoredNewerObservationApproval {
            row: RowId(self.row),
            key: ReportKey::new(
                TerminalId(self.terminal),
                self.side.into(),
                crate::reports::parse_environment(&self.environment)?,
            )
            .map_err(|_| RepositoryError::Invalid)?,
            observed_at: timestamp(&self.observed_at)?,
            commodity: CommodityId(self.commodity),
            price: self.price.parse().map_err(|_| RepositoryError::Invalid)?,
            stock: self.stock.parse().map_err(|_| RepositoryError::Invalid)?,
            status: self.status,
            evidence: NewerObservationEvidence::new(
                &self.account,
                match self.mode {
                    ModeDocument::DryRun => SubmissionMode::DryRun,
                    ModeDocument::Production => SubmissionMode::Production,
                },
                match self.target {
                    EnvironmentDocument::Live => UexEnvironment::Live,
                    EnvironmentDocument::Ptu => UexEnvironment::Ptu,
                },
                timestamp(&self.newer_observed_at)?,
            )
            .map_err(|_| RepositoryError::Invalid)?,
            confirmed_at: timestamp(&self.confirmed_at)?,
        })
    }
}
