// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed private HTTP routing without diagnostics for credential-bearing commands.

use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_adapter_uex::api::{ReadError, UexReader};
use tallyline_reference_data::api::ReferenceDataFetcher;
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::TerminalId;
use tallyline_submission::api::SubmissionRowReceiptVerifier;
use tallyline_submission::api::model::SubmissionJob;
use tallyline_submission::api::model::VerifiedUexReport;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SubmissionResult, ValidatedAccount,
};
use tallyline_submission::api::{
    CredentialValidator, SubmissionGateway, SubmissionReceiptVerifier,
};
use tokio::sync::oneshot::Sender;

/// The existing adapter operations driven only by the owned shared I/O runtime.
pub trait HttpBackend: Send + Sync {
    /// Installs fresh admission for token-bearing reads inside compound adapter operations.
    fn with_authenticated_read_gate(
        self,
        _gate: tallyline_adapter_uex::api::AuthenticatedReadGate,
    ) -> Self
    where
        Self: Sized,
    {
        self
    }
    /// Checks account standing without issuing a token-bearing validation request.
    fn check_account_while_token_held(
        &self,
        _values: CredentialValues,
    ) -> impl Future<Output = CredentialFailure> + Send {
        async { CredentialFailure::Unavailable }
    }

    /// Transfers authenticated-read quota evidence without re-entering shared admission.
    fn take_quota_observations(
        &self,
    ) -> Vec<tallyline_submission::api::model::TokenQuotaObservation> {
        Vec::new()
    }

    /// Fetches one public endpoint through the shared client.
    fn fetch(
        &self,
        endpoint: Endpoint,
    ) -> impl Future<Output = Result<EndpointData, ReadError>> + Send;
    /// Fetches bounded public terminal priors through the same client.
    fn fetch_terminal_priors(
        &self,
        terminals: Vec<TerminalId>,
    ) -> impl Future<Output = Result<Vec<TerminalPriors>, ReadError>> + Send;
    /// Performs read-only validation without exposing credential values to diagnostics.
    fn validate(
        &self,
        values: CredentialValues,
    ) -> impl Future<Output = Result<ValidatedAccount, CredentialFailure>> + Send;
    /// Performs bounded receipt reads independently of the submission permit.
    fn verify_receipts(
        &self,
        job: tallyline_submission::api::model::SubmissionJob,
        values: CredentialValues,
        release_token: Option<String>,
    ) -> impl Future<Output = Option<Vec<tallyline_submission::api::model::VerifiedUexReport>>> + Send;
    /// Reads positive partial or unknown identities independently of the submission permit.
    fn verify_row_receipts(
        &self,
        _job: SubmissionJob,
        _values: CredentialValues,
        _release_token: Option<String>,
    ) -> impl Future<Output = Option<Vec<VerifiedUexReport>>> + Send {
        async { None }
    }
    /// Reads positive lifecycle values for one already committed account batch.
    ///
    /// # Errors
    /// Returns `Unavailable` when admission, credentials, transport, quota or processing
    /// prevents a usable answer, or `InvalidResponse` for malformed, ambiguous or
    /// undocumented answer data; neither failure supplies lifecycle evidence.
    fn read_lifecycle(
        &self,
        _batch: tallyline_submission::api::model::LifecycleReadBatch,
        _values: CredentialValues,
        _release_token: Option<String>,
    ) -> impl Future<
        Output = Result<
            tallyline_submission::api::model::LifecycleReadResponse,
            tallyline_submission::api::model::LifecycleReadFailure,
        >,
    > + Send {
        async { Err(tallyline_submission::api::model::LifecycleReadFailure::Unavailable) }
    }
    /// Repeats only a held refusal's read-only receipt safety check.
    fn recheck_refusal(
        &self,
        _job: tallyline_submission::api::model::SubmissionJob,
        _values: CredentialValues,
        _release_token: Option<String>,
    ) -> impl Future<Output = Option<SubmissionResult>> + Send {
        async { None }
    }
    /// Dispatches one already encoded body without retrying uncertain writes.
    fn submit(
        &self,
        prepared: PreparedUexSubmission,
        values: CredentialValues,
        release_token: Option<String>,
    ) -> impl Future<Output = SubmissionResult> + Send;
}
impl HttpBackend for UexReader {
    fn with_authenticated_read_gate(
        self,
        gate: tallyline_adapter_uex::api::AuthenticatedReadGate,
    ) -> Self {
        UexReader::with_authenticated_read_gate(self, gate)
    }
    async fn check_account_while_token_held(&self, values: CredentialValues) -> CredentialFailure {
        UexReader::check_account_while_token_held(self, &values).await
    }

    fn take_quota_observations(
        &self,
    ) -> Vec<tallyline_submission::api::model::TokenQuotaObservation> {
        UexReader::take_quota_observations(self)
    }

    async fn read_lifecycle(
        &self,
        batch: tallyline_submission::api::model::LifecycleReadBatch,
        values: CredentialValues,
        release_token: Option<String>,
    ) -> Result<
        tallyline_submission::api::model::LifecycleReadResponse,
        tallyline_submission::api::model::LifecycleReadFailure,
    > {
        tallyline_submission::api::SubmissionLifecycleReader::read_lifecycle(
            self,
            batch,
            &values,
            release_token.as_deref(),
        )
        .await
    }
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        ReferenceDataFetcher::fetch(self, endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        terminals: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        ReferenceDataFetcher::fetch_terminal_priors(self, &terminals).await
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        CredentialValidator::validate(self, &values).await
    }
    async fn verify_receipts(
        &self,
        job: tallyline_submission::api::model::SubmissionJob,
        values: CredentialValues,
        release_token: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        SubmissionReceiptVerifier::verify_receipts(self, &job, &values, release_token.as_deref())
            .await
    }
    async fn verify_row_receipts(
        &self,
        job: SubmissionJob,
        values: CredentialValues,
        release_token: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        SubmissionRowReceiptVerifier::verify_row_receipts(
            self,
            &job,
            &values,
            release_token.as_deref(),
        )
        .await
    }
    async fn recheck_refusal(
        &self,
        job: tallyline_submission::api::model::SubmissionJob,
        values: CredentialValues,
        release_token: Option<String>,
    ) -> Option<SubmissionResult> {
        self.recheck_submission_refusal(&job, &values, release_token.as_deref())
            .await
    }
    async fn submit(
        &self,
        prepared: PreparedUexSubmission,
        values: CredentialValues,
        release_token: Option<String>,
    ) -> SubmissionResult {
        SubmissionGateway::submit(self, prepared, &values, release_token.as_deref()).await
    }
}

pub(crate) enum Command {
    ReadLifecycle(
        tallyline_submission::api::model::LifecycleReadBatch,
        CredentialValues,
        Option<String>,
        Sender<
            Result<
                tallyline_submission::api::model::LifecycleReadResponse,
                tallyline_submission::api::model::LifecycleReadFailure,
            >,
        >,
    ),
    VerifyRowReceipts(
        SubmissionJob,
        CredentialValues,
        Option<String>,
        Sender<Option<Vec<VerifiedUexReport>>>,
    ),
    RecheckRefusal(
        tallyline_submission::api::model::SubmissionJob,
        CredentialValues,
        Option<String>,
        Sender<Option<SubmissionResult>>,
    ),
    ReserveSubmission(Sender<crate::api::SubmissionSlot>),
    VerifyReceipts(
        tallyline_submission::api::model::SubmissionJob,
        CredentialValues,
        Option<String>,
        Sender<Option<Vec<tallyline_submission::api::model::VerifiedUexReport>>>,
    ),
    Fetch(Endpoint, Sender<Result<EndpointData, ReadError>>),
    Priors(
        Vec<TerminalId>,
        Sender<Result<Vec<TerminalPriors>, ReadError>>,
    ),
    Validate(
        CredentialValues,
        Sender<Result<ValidatedAccount, CredentialFailure>>,
    ),
    Submit(
        PreparedUexSubmission,
        CredentialValues,
        Option<String>,
        Sender<SubmissionResult>,
    ),
}
