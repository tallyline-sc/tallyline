// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Transaction-scoped comparison of cached references and accepted local priors.

use rusqlite::Transaction;
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_submission::api::model::SubmissionRepositoryError;

pub(crate) fn check(
    transaction: &Transaction<'_>,
    snapshot: &ReferenceSnapshot,
) -> Result<(), SubmissionRepositoryError> {
    let references = crate::reference_cache::load_references(transaction)
        .map_err(|_| SubmissionRepositoryError::Unavailable)?;
    if !snapshot.matches_cached_references(&references) {
        return Err(SubmissionRepositoryError::Conflict);
    }
    let accepted = crate::accepted_priors::load(transaction)
        .map_err(|_| SubmissionRepositoryError::Unavailable)?;
    let projected = snapshot
        .clone()
        .with_local_priors(&accepted.active(snapshot.environment()))
        .map_err(|_| SubmissionRepositoryError::Invalid)?;
    if &projected != snapshot {
        return Err(SubmissionRepositoryError::Conflict);
    }
    Ok(())
}
