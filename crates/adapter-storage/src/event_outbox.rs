// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact SQLite notification identities and idempotent post-delivery acknowledgements.

use crate::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::{EventId, EventSource};
use tallyline_workflows::api::{EventOutbox, OutboxError, OutboxEvent, OutboxKind};

fn meaning(source: &str, kind: &str) -> Result<(EventSource, OutboxKind), OutboxError> {
    match (source, kind) {
        ("submission_lifecycle", "status_changed") => Ok((
            EventSource::SubmissionLifecycle,
            OutboxKind::UexReportStatusChanged,
        )),
        ("reporting", "released") => Ok((EventSource::Reporting, OutboxKind::ReportReleased)),
        ("reporting", "submitted") => Ok((EventSource::Reporting, OutboxKind::ReportSubmitted)),
        ("reporting", "discarded") => Ok((EventSource::Reporting, OutboxKind::ReportDiscarded)),
        ("submission", "succeeded") => {
            Ok((EventSource::Submission, OutboxKind::SubmissionSucceeded))
        }
        ("submission", "partially_accepted") => Ok((
            EventSource::Submission,
            OutboxKind::SubmissionPartiallyAccepted,
        )),
        ("submission", "outcome_unknown") => Ok((
            EventSource::Submission,
            OutboxKind::SubmissionOutcomeUnknown,
        )),
        ("submission", "deferred") => Ok((EventSource::Submission, OutboxKind::SubmissionDeferred)),
        ("submission", "rejected") => Ok((EventSource::Submission, OutboxKind::SubmissionRejected)),
        ("submission", "returned") => Ok((EventSource::Submission, OutboxKind::SubmissionReturned)),
        ("submission", "cancelled_split") => {
            Ok((EventSource::Submission, OutboxKind::SubmissionSplit))
        }
        ("submission", "cancelled") => {
            Ok((EventSource::Submission, OutboxKind::SubmissionCancelled))
        }
        _ => Err(OutboxError::Invalid),
    }
}
fn names(kind: OutboxKind) -> (&'static str, &'static str) {
    match kind {
        OutboxKind::UexReportStatusChanged => ("submission_lifecycle", "status_changed"),
        OutboxKind::ReportReleased => ("reporting", "released"),
        OutboxKind::ReportSubmitted => ("reporting", "submitted"),
        OutboxKind::ReportDiscarded => ("reporting", "discarded"),
        OutboxKind::SubmissionSucceeded => ("submission", "succeeded"),
        OutboxKind::SubmissionPartiallyAccepted => ("submission", "partially_accepted"),
        OutboxKind::SubmissionOutcomeUnknown => ("submission", "outcome_unknown"),
        OutboxKind::SubmissionDeferred => ("submission", "deferred"),
        OutboxKind::SubmissionRejected => ("submission", "rejected"),
        OutboxKind::SubmissionReturned => ("submission", "returned"),
        OutboxKind::SubmissionCancelled => ("submission", "cancelled"),
        OutboxKind::SubmissionSplit => ("submission", "cancelled_split"),
    }
}
impl EventOutbox for SqliteReferenceCache {
    fn pending_events(&mut self) -> Result<Vec<OutboxEvent>, OutboxError> {
        let mut query=self.connection.prepare("SELECT event_source,aggregate_id,revision,event_kind FROM domain_outbox WHERE delivered=0 ORDER BY rowid LIMIT ?1")
            .map_err(|_|OutboxError::Unavailable)?;
        let limit = i64::try_from(tallyline_workflows::api::OUTBOX_BATCH_LIMIT)
            .map_err(|_| OutboxError::Invalid)?;
        let rows = query
            .query_map([limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|_| OutboxError::Unavailable)?;
        rows.map(|row| {
            let (source, aggregate, version, kind) = row.map_err(|_| OutboxError::Invalid)?;
            let (source, kind) = meaning(&source, &kind)?;
            let identity = EventId::new(
                source,
                aggregate.try_into().map_err(|_| OutboxError::Invalid)?,
                version.try_into().map_err(|_| OutboxError::Invalid)?,
            )
            .ok_or(OutboxError::Invalid)?;
            OutboxEvent::new(identity, kind).ok_or(OutboxError::Invalid)
        })
        .collect()
    }
    fn mark_delivered(&mut self, event: OutboxEvent) -> Result<(), OutboxError> {
        let (source, kind) = names(event.kind());
        let aggregate: i64 = event
            .id()
            .aggregate_id()
            .try_into()
            .map_err(|_| OutboxError::Invalid)?;
        let version: i64 = event
            .id()
            .version()
            .try_into()
            .map_err(|_| OutboxError::Invalid)?;
        let changed=self.connection.execute("UPDATE domain_outbox SET delivered=1 WHERE event_source=?1 AND aggregate_id=?2 AND revision=?3 AND event_kind=?4",(source,aggregate,version,kind))
            .map_err(|_|OutboxError::Unavailable)?;
        if changed != 1 {
            return Err(OutboxError::Invalid);
        }
        Ok(())
    }
}
