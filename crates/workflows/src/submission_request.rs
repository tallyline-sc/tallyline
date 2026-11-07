// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact manual Report translation at the Reporting and Submission boundary.

use tallyline_reporting::api::model::Report;
use tallyline_shared_kernel::api::model::{SubmissionMode, UexEnvironment};
use tallyline_submission::api::model::{
    InvalidSubmission, SubmissionHeader, SubmissionRequest, SubmissionRow,
};

/// Captures entered values and explicit account, mode and mapped environment without changing the Report.
/// The release workflow must establish complete readiness before committing this snapshot.
///
/// # Errors
/// Refuses absent mandatory values, unsupported payload values, missing context and excessive rows.
pub fn manual_submission_request(
    report: &Report,
    environment: UexEnvironment,
    account: &str,
    mode: SubmissionMode,
) -> Result<SubmissionRequest, InvalidSubmission> {
    let rows = report
        .rows()
        .iter()
        .filter(|row| row.entered())
        .map(|row| {
            SubmissionRow::new(
                row.id(),
                row.commodity(),
                row.price().ok_or(InvalidSubmission)?,
                row.stock().ok_or(InvalidSubmission)?,
                row.status().ok_or(InvalidSubmission)?.get(),
            )
        })
        .collect::<Result<Vec<_>, InvalidSubmission>>()?;
    let mut request = SubmissionRequest::new(
        SubmissionHeader {
            report: report.id(),
            report_version: report.version(),
            account: account.to_owned(),
            mode,
            terminal: report.key().terminal(),
            side: report.key().side(),
            environment,
            game_version: report.game_version().ok_or(InvalidSubmission)?.to_owned(),
            faction_affinity: report.faction_affinity(),
            observed_at: report.observed_at(),
        },
        rows,
    )?;
    for approval in report
        .newer_observation_confirmations()
        .iter()
        .filter(|approval| {
            approval.evidence().account() == account
                && approval.evidence().mode() == mode
                && approval.evidence().environment() == environment
        })
    {
        request =
            request.with_confirmed_newer_observation(approval.row(), approval.observed_at())?;
    }
    Ok(request)
}
