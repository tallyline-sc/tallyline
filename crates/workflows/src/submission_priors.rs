// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Accepted production outcomes mapped to explicit local prior records.

use std::{num::NonZeroU8, sync::Arc};
use tallyline_reference_data::api::model::{
    AcceptedLocalPrior, AcceptedPriorContext, InvalidPrior, PriorValues,
};
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::model::{JobPhase, SubmissionJob, SubmissionResult};

/// Produces only accepted production records for the workflow's atomic outcome transaction.
///
/// Full-count success establishes acceptance without verified identities.
/// Partial and unknown rows require verified identities; statements create no priors.
/// PTU records remain history; reference lookups never promote them to usable prices.
///
/// # Errors
/// Refuses inconsistent success metadata instead of omitting a supposedly accepted row.
pub fn accepted_submission_priors(
    job: &SubmissionJob,
) -> Result<Vec<AcceptedLocalPrior>, InvalidPrior> {
    let request = job.request();
    let header = request.header();
    if header.mode != SubmissionMode::Production {
        return Ok(Vec::new());
    }
    let full = job.phase() == JobPhase::Succeeded;
    if full
        && !matches!(
            job.result(),
            Some(
                SubmissionResult::ProductionSucceeded(_) | SubmissionResult::ProductionAccepted(_)
            )
        )
    {
        return Err(InvalidPrior);
    }
    if !full && job.verified_row_receipts().next().is_none() {
        return Ok(Vec::new());
    }
    let accepted_at = job
        .attempts()
        .last()
        .and_then(|attempt| attempt.ended_at())
        .ok_or(InvalidPrior)?;
    let context = Arc::new(AcceptedPriorContext {
        report: header.report,
        report_version: header.report_version,
        submission: job.id(),
        environment: header.environment,
        terminal: header.terminal,
        side: header.side,
        game_version: header.game_version.clone(),
        observed_at: header.observed_at,
        accepted_at,
    });
    request
        .rows()
        .iter()
        .filter(|row| {
            full || job
                .verified_row_receipts()
                .any(|receipt| receipt.row() == row.id())
        })
        .map(|row| {
            AcceptedLocalPrior::new(
                Arc::clone(&context),
                row.commodity(),
                PriorValues {
                    price: row.price(),
                    stock: row.stock(),
                    status: Some(NonZeroU8::new(row.status()).ok_or(InvalidPrior)?),
                    container_sizes: Vec::new(),
                },
            )
        })
        .collect()
}
