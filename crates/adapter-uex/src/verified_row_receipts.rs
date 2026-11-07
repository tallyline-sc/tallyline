// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Positive receipt recovery for immutable partial and unknown attempts.

use crate::api::UexReader;
use crate::receipt_window::ReceiptReadFailure;
use tallyline_submission::api::SubmissionRowReceiptVerifier;
use tallyline_submission::api::model::{
    CredentialValues, SubmissionJob, SubmissionResult, VerifiedUexReport,
};

impl SubmissionRowReceiptVerifier for UexReader {
    async fn verify_row_receipts(
        &self,
        job: &SubmissionJob,
        credentials: &CredentialValues,
        release_token: Option<&str>,
    ) -> Option<Vec<VerifiedUexReport>> {
        if !job.can_check_receipts() {
            return None;
        }
        let date_added = job.attempts().last()?.date_added();
        let token = credentials.own_token().or(release_token)?;
        let request = job.request();
        let verification = async {
            let mut receipts = Vec::new();
            if let Some(SubmissionResult::Partial(ids)) =
                job.result().map(SubmissionResult::final_result)
            {
                for id in ids {
                    match self
                        .read_receipt_window(request, date_added, credentials, token, Some(id.id()))
                        .await
                    {
                        Ok(window) => {
                            if let [proof] = window.receipts.as_slice() {
                                merge(&mut receipts, proof.clone())?;
                            }
                        }
                        Err(ReceiptReadFailure::Ambiguous) => return None,
                        Err(
                            ReceiptReadFailure::Unavailable | ReceiptReadFailure::InvalidWindow,
                        ) => continue,
                    }
                }
            }
            match self
                .read_receipt_window(request, date_added, credentials, token, None)
                .await
            {
                Ok(window) => {
                    for proof in window.receipts {
                        merge(&mut receipts, proof)?;
                    }
                    Some(receipts)
                }
                Err(ReceiptReadFailure::Ambiguous) => None,
                Err(_) if !receipts.is_empty() => Some(receipts),
                Err(_) => None,
            }
        };
        tokio::time::timeout(crate::receipt_window::TIMEOUT, verification)
            .await
            .ok()
            .flatten()
    }
}
fn merge(receipts: &mut Vec<VerifiedUexReport>, proof: VerifiedUexReport) -> Option<()> {
    if receipts.iter().any(|previous| {
        (previous.row() == proof.row() || previous.id() == proof.id()) && previous != &proof
    }) {
        return None;
    }
    if !receipts.contains(&proof) {
        receipts.push(proof);
    }
    Some(())
}
