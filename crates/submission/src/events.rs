// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Submission commands and durable outcome envelopes.

use crate::api::model::SubmissionResolutionAction;
use crate::api::model::{JobPhase, PayloadHash, QueueRefusal, SubmissionJob, SubmissionResult};
use tallyline_shared_kernel::api::model::{EventId, EventSource, ReportId, Timestamp};

/// A single immutable job command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionCommand {
    /// Commits a write-ahead attempt before dispatch.
    Begin {
        /// The injected wall-clock attempt start.
        at: Timestamp,
        /// The digest of the exact prepared bytes.
        payload_hash: PayloadHash,
        /// The exact whole-second time encoded in those bytes.
        date_added: Timestamp,
    },
    /// Cancels an unsent job at the user's request.
    Cancel,
    /// Cancels a queued origin only as part of an atomic released-row split.
    CancelForSplit {
        /// Fresh Report identity for the ready rows; the cooling part retains this Report.
        report: ReportId,
        /// The injected explicit split time.
        at: Timestamp,
    },
    /// Returns an eligible held request to the complete send-time gate.
    Retry,
    /// Returns proven unprocessed quota work through the full gate after its reset.
    ResumeQuota {
        /// The injected expiry observation time.
        at: Timestamp,
    },
    /// Retries or acknowledges a version lockout only from a strictly newer compiled release.
    RetryAfterUpdate {
        /// Immutable current release identity supplied by the application owner.
        version: tallyline_shared_kernel::api::model::ReleaseVersion,
    },
    /// Returns an unsent job to manual review without inventing an attempt.
    Return(crate::api::model::SendReturnReason),
    /// Closes interrupted work without replay.
    Recover {
        /// The injected recovery time.
        at: Timestamp,
    },
    /// Records a read-only account check without attesting receipt certainty.
    ResolveAccountCheck(crate::api::model::SubmissionPauseReason),
    /// Resolves a held refusal through a later read-only receipt check.
    ResolveOutcomeCheck {
        /// The confirmed refusal or partial-acceptance outcome.
        result: SubmissionResult,
    },
    /// Records explicit evidence without replaying an uncertain attempt.
    ResolveDelivery(SubmissionResolutionAction),
    /// Records the result of exactly one dispatched attempt.
    Complete {
        /// The closed adapter result.
        result: SubmissionResult,
        /// The injected completion time.
        at: Timestamp,
    },
}

/// An outcome published by Submission and applied to Reporting by workflows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionEventKind {
    /// A successful test or production attempt; the mode remains fixed in its job.
    Succeeded(SubmissionResult),
    /// A permanent refusal ended the released delivery.
    Rejected,
    /// Some rows may have been processed and automatic replay is closed.
    PartiallyAccepted(SubmissionResult),
    /// Processing cannot be excluded.
    OutcomeUnknown,
    /// A known unprocessed attempt is held for an explicit scheduling decision.
    Deferred(Timestamp),
    /// The Report requires renewed release, including unsent startup recovery.
    Returned(SubmissionReturn),
    /// The user cancelled before sending began.
    Cancelled(CancellationReason),
}

/// A credential-free immutable outcome with an idempotent outbox identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionEvent {
    id: EventId,
    report: ReportId,
    kind: SubmissionEventKind,
}
impl SubmissionEvent {
    /// Returns the stable identity used to deduplicate at-least-once delivery.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }
    /// Returns the Report to which workflows applies the event.
    #[must_use]
    pub const fn report(&self) -> ReportId {
        self.report
    }
    /// Borrows the outcome without speculative identities.
    #[must_use]
    pub const fn kind(&self) -> &SubmissionEventKind {
        &self.kind
    }
}

/// A new aggregate state and its event, saved in the same transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionChange {
    expected_revision: u64,
    state: SubmissionJob,
    event: Option<SubmissionEvent>,
}
impl SubmissionChange {
    /// Borrows the new state for atomic persistence.
    #[must_use]
    pub const fn state(&self) -> &SubmissionJob {
        &self.state
    }
    /// Returns the version the transaction must compare before writing.
    #[must_use]
    pub const fn expected_revision(&self) -> u64 {
        self.expected_revision
    }
    /// Borrows the outcome to insert in the same transaction, if any.
    #[must_use]
    pub const fn event(&self) -> Option<&SubmissionEvent> {
        self.event.as_ref()
    }
    /// Takes the committed state after persistence succeeds.
    #[must_use]
    pub fn into_state(self) -> SubmissionJob {
        self.state
    }
}
impl SubmissionJob {
    /// Produces an immutable state and outcome for one atomic repository operation.
    ///
    /// # Errors
    /// Refuses invalid phase transitions, payloads or exhausted persistence versions.
    pub fn apply(&self, command: SubmissionCommand) -> Result<SubmissionChange, QueueRefusal> {
        let account_check = matches!(&command, SubmissionCommand::ResolveAccountCheck(_));
        let split = matches!(command, SubmissionCommand::CancelForSplit { .. });
        let state = match command {
            SubmissionCommand::ResolveDelivery(action) => self.resolve_delivery(action)?,
            SubmissionCommand::ResolveAccountCheck(reason) => self.resolve_account_check(reason)?,
            SubmissionCommand::Begin {
                at,
                payload_hash,
                date_added,
            } => self.begin_prepared(at, payload_hash, date_added)?,
            SubmissionCommand::Cancel => self.cancel()?,
            SubmissionCommand::CancelForSplit { report, at } => {
                self.cancel_for_split(report, at)?
            }
            SubmissionCommand::Retry => self.retry()?,
            SubmissionCommand::ResumeQuota { at } => self.resume_quota(at)?,
            SubmissionCommand::RetryAfterUpdate { version } => self.retry_after_update(version)?,
            SubmissionCommand::ResolveOutcomeCheck { result } => {
                self.resolve_outcome_check(result)?
            }
            SubmissionCommand::Return(reason) => self.return_for_review(reason)?,
            SubmissionCommand::Recover { at } => self.recover(at)?,
            SubmissionCommand::Complete { result, at } => self.complete(result, at)?,
        };
        let event = if account_check
            || state.revision() == self.revision()
            || (state.service_pause_acknowledged() && !self.service_pause_acknowledged())
        {
            None
        } else {
            let result = state.result().map(SubmissionResult::final_result).cloned();
            let kind = match state.phase() {
                JobPhase::Queued | JobPhase::Sending => None,
                JobPhase::Succeeded => Some(SubmissionEventKind::Succeeded(
                    result.ok_or(QueueRefusal::InvalidTransition)?,
                )),
                JobPhase::PartiallyAccepted => Some(SubmissionEventKind::PartiallyAccepted(
                    result.ok_or(QueueRefusal::InvalidTransition)?,
                )),
                JobPhase::OutcomeUnknown => Some(SubmissionEventKind::OutcomeUnknown),
                JobPhase::Rejected => Some(SubmissionEventKind::Rejected),
                JobPhase::Held | JobPhase::Paused => None,
                JobPhase::Returned => Some(SubmissionEventKind::Returned(
                    match (state.review_reason(), result) {
                        (Some(reason), _) => SubmissionReturn::Preflight(reason),
                        (None, Some(SubmissionResult::DuplicateEvidence(_))) => {
                            SubmissionReturn::Response(
                                crate::api::model::SubmissionReturnReason::PossiblyAlreadyReceived,
                            )
                        }
                        (None, Some(SubmissionResult::Returned(reason))) => {
                            SubmissionReturn::Response(reason)
                        }
                        (None, _) => SubmissionReturn::Restart,
                    },
                )),
                JobPhase::Cancelled => Some(SubmissionEventKind::Cancelled(if split {
                    CancellationReason::Split
                } else {
                    CancellationReason::User
                })),
            };
            if let Some(kind) = kind {
                let id = EventId::new(EventSource::Submission, state.id().0, state.revision())
                    .ok_or(QueueRefusal::InvalidTransition)?;
                Some(SubmissionEvent {
                    id,
                    report: state.request().header().report,
                    kind,
                })
            } else {
                None
            }
        };
        Ok(SubmissionChange {
            expected_revision: self.revision(),
            state,
            event,
        })
    }
}

/// Why a cancelled job returns to its owning workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationReason {
    /// Explicit user cancellation before sending began.
    User,
    /// The exact released rows are atomically partitioned into replacement jobs.
    Split,
}

/// Why an unfinished Report requires renewed manual release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionReturn {
    /// Startup closed an unfinished job that was never in flight.
    Restart,
    /// Fresh preflight facts require renewed release before any further attempt.
    Preflight(crate::api::model::SendReturnReason),
    /// A documented report-fixable response ended the attempt.
    Response(crate::api::model::SubmissionReturnReason),
}
