// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Version-bound diagnostics for committed permanent technical refusals.

use tallyline_submission::api::model::{SubmissionCorrection, SubmissionJob, SubmissionResult};

pub(super) fn record(job: &SubmissionJob) {
    let Some(SubmissionResult::Rejected(refusal)) =
        job.result().map(SubmissionResult::final_result)
    else {
        return;
    };
    if refusal.correction() == SubmissionCorrection::NewCapture {
        return;
    }
    let code = refusal.code();
    match crate::build_info::compiled_build_information() {
        Ok(build) => tracing::warn!(
            uex_code = code.as_str(),
            app_version = build.version(),
            "UEX permanently refused a submission; review as a suspected client defect"
        ),
        Err(error) => {
            tracing::warn!(uex_code = code.as_str(), build_error = ?error, "UEX permanently refused a submission; compiled identity is unavailable")
        }
    }
}
