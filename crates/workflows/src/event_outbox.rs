// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded at-least-once dispatch of committed aggregate-change notifications.

use tallyline_shared_kernel::api::model::{EventId, EventSource};

/// Maximum notifications handled by one synchronous dispatcher step.
pub const OUTBOX_BATCH_LIMIT: usize = 32;
/// Closed meanings of durable aggregate-change notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboxKind {
    /// A Report acquired its fixed release binding.
    ReportReleased,
    /// A Report was accepted completely.
    ReportSubmitted,
    /// A Report was discarded explicitly.
    ReportDiscarded,
    /// A submission succeeded; its committed job owns the exact outcome.
    SubmissionSucceeded,
    /// A submission may have accepted a partial row set.
    SubmissionPartiallyAccepted,
    /// A submission outcome is unknown and replay remains closed.
    SubmissionOutcomeUnknown,
    /// A known unprocessed submission was deferred.
    SubmissionDeferred,
    /// A submission returned to review.
    SubmissionReturned,
    /// A submission was permanently refused.
    SubmissionRejected,
    /// An unsent submission was cancelled.
    SubmissionCancelled,
    /// A cancelled origin was atomically replaced by two released row groups.
    SubmissionSplit,
    /// Committed lifecycle evidence and its accepted-prior projection changed.
    UexReportStatusChanged,
}
/// An immutable notification whose handler reads the current committed aggregate projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutboxEvent {
    id: EventId,
    kind: OutboxKind,
}
impl OutboxEvent {
    /// Binds a closed notification meaning to its owning context identity.
    #[must_use]
    pub fn new(id: EventId, kind: OutboxKind) -> Option<Self> {
        let source = match kind {
            OutboxKind::UexReportStatusChanged => EventSource::SubmissionLifecycle,
            OutboxKind::ReportReleased
            | OutboxKind::ReportSubmitted
            | OutboxKind::ReportDiscarded => EventSource::Reporting,
            OutboxKind::SubmissionSucceeded
            | OutboxKind::SubmissionPartiallyAccepted
            | OutboxKind::SubmissionOutcomeUnknown
            | OutboxKind::SubmissionDeferred
            | OutboxKind::SubmissionRejected
            | OutboxKind::SubmissionReturned
            | OutboxKind::SubmissionCancelled
            | OutboxKind::SubmissionSplit => EventSource::Submission,
        };
        (id.source() == source).then_some(Self { id, kind })
    }
    /// Returns the stable idempotence identity across retries and restarts.
    #[must_use]
    pub const fn id(self) -> EventId {
        self.id
    }
    /// Returns the committed notification meaning without account or report values.
    #[must_use]
    pub const fn kind(self) -> OutboxKind {
        self.kind
    }
}
/// Closed notification-delivery failures without provider diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboxError {
    /// A stored event or bounded batch violates its contract.
    Invalid,
    /// Storage or the receiver cannot complete this delivery step.
    Unavailable,
}
/// Reads committed notifications and acknowledges only successful in-process delivery.
pub trait EventOutbox {
    /// Loads at most one bounded batch, preserving undelivered identity and ordering.
    ///
    /// # Errors
    /// Returns invalid stored events or unavailable storage rather than an empty substitute.
    fn pending_events(&mut self) -> Result<Vec<OutboxEvent>, OutboxError>;
    /// Idempotently acknowledges the exact committed notification after handler success.
    ///
    /// # Errors
    /// Refuses missing or mismatched events and failed saves.
    fn mark_delivered(&mut self, event: OutboxEvent) -> Result<(), OutboxError>;
}
/// Dispatches one bounded batch, retaining events after failed handlers or acknowledgements.
/// Handlers must be idempotent; dispatch never releases or sends a submission.
///
/// # Errors
/// Stops at the first malformed batch, handler failure or acknowledgement failure.
pub fn dispatch_pending_events(
    outbox: &mut impl EventOutbox,
    mut handle: impl FnMut(OutboxEvent) -> Result<(), OutboxError>,
) -> Result<usize, OutboxError> {
    let events = outbox.pending_events()?;
    if events.len() > OUTBOX_BATCH_LIMIT {
        return Err(OutboxError::Invalid);
    }
    let mut identities = std::collections::BTreeSet::new();
    if events.iter().any(|event| !identities.insert(event.id())) {
        return Err(OutboxError::Invalid);
    }
    let mut delivered = 0;
    for event in events {
        handle(event)?;
        outbox.mark_delivered(event)?;
        delivered += 1;
    }
    Ok(delivered)
}
