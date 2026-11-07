// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared external HTTP-port responses for public receipt-controller contracts.

use super::recovery::RecoveryBackend;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::HttpBackend;
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::TerminalId;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SubmissionJob, SubmissionResult, ValidatedAccount,
    VerifiedUexReport,
};
#[derive(Clone, Copy)]
pub(super) enum EvidenceResponse {
    Positive,
    Empty,
}
pub(super) struct EvidenceBackend {
    pub(super) response: EvidenceResponse,
    pub(super) base: RecoveryBackend,
    pub(super) reads: Arc<AtomicUsize>,
    pub(super) release: Arc<AtomicBool>,
}
impl HttpBackend for EvidenceBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        self.base.fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        ids: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        self.base.fetch_terminal_priors(ids).await
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        self.base.validate(values).await
    }
    async fn verify_receipts(
        &self,
        _: SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        None
    }
    async fn verify_row_receipts(
        &self,
        job: SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        while !self.release.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        if matches!(self.response, EvidenceResponse::Empty) {
            return Some(Vec::new());
        }
        let at = job.attempts().last()?.date_added();
        Some(vec![
            VerifiedUexReport::from_lookup_at(
                718,
                job.request().header().terminal,
                job.request().rows().first()?.commodity(),
                at,
                job.request(),
                at,
            )
            .ok()?,
        ])
    }
    async fn submit(
        &self,
        prepared: PreparedUexSubmission,
        values: CredentialValues,
        token: Option<String>,
    ) -> SubmissionResult {
        self.base.submit(prepared, values, token).await
    }
}
