// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic explicit new-Draft creation from eligible immutable submission history.

use crate::api::SqliteReferenceCache;
use rusqlite::TransactionBehavior;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{JobId, SubmissionRepositoryError};
use tallyline_workflows::api::{ResolutionDraft, ResolutionDraftStore};

impl ResolutionDraftStore for SqliteReferenceCache {
    fn duplicate_resolved(
        &mut self,
        id: JobId,
        expected_revision: u64,
        at: Timestamp,
    ) -> Result<ResolutionDraft, SubmissionRepositoryError> {
        use tallyline_shared_kernel::api::model::ReportId;
        use tallyline_submission::api::model::{SubmissionCommand, SubmissionResolutionAction};
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| SubmissionRepositoryError::Unavailable)?;
        let origin =
            crate::queue::load_job(&transaction, id)?.ok_or(SubmissionRepositoryError::Invalid)?;
        if origin.revision() != expected_revision {
            return Err(SubmissionRepositoryError::Conflict);
        }
        let partial_rows = origin
            .partial_copy_rows()
            .map(|rows| rows.collect::<Vec<_>>());
        if !origin.can_duplicate_unknown()
            && !origin.can_duplicate_rejected()
            && partial_rows.is_none()
        {
            return Err(SubmissionRepositoryError::Invalid);
        }
        let report = crate::queue::load_report(&transaction, origin.request().header().report)?;
        transaction
            .execute("INSERT INTO reports(version,payload) VALUES(0,'')", [])
            .map_err(|_| SubmissionRepositoryError::Unavailable)?;
        let identity = ReportId(
            transaction
                .last_insert_rowid()
                .try_into()
                .map_err(|_| SubmissionRepositoryError::Invalid)?,
        );
        let draft = match &partial_rows {
            Some(rows) => report.duplicate_partial_rows_as_draft(identity, rows),
            None => report.duplicate_as_draft(identity),
        }
        .map_err(|_| SubmissionRepositoryError::Invalid)?;
        crate::queue::save_report(&transaction, &draft, 0)?;
        transaction.execute("INSERT INTO report_recents(report_id,changed_order) VALUES(?1,(SELECT COALESCE(MAX(changed_order),0)+1 FROM report_recents))", [transaction.last_insert_rowid()]).map_err(|_| SubmissionRepositoryError::Unavailable)?;
        let action = match partial_rows {
            Some(rows) => SubmissionResolutionAction::RowsDuplicated {
                report: identity,
                rows,
                at,
            },
            None => SubmissionResolutionAction::Duplicated {
                report: identity,
                at,
            },
        };
        let change = origin
            .apply(SubmissionCommand::ResolveDelivery(action))
            .map_err(|_| SubmissionRepositoryError::Invalid)?;
        crate::queue::save_change(&transaction, &change)?;
        transaction
            .commit()
            .map_err(|_| SubmissionRepositoryError::Unavailable)?;
        Ok(ResolutionDraft {
            origin: change.into_state(),
            draft,
        })
    }
}
