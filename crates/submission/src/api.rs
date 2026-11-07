// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Credential validation requests and save-before-publish application operations.

pub mod model;

use model::SubmissionJob;
use model::VerifiedUexReport;
use model::{
    AccountState, CredentialCandidate, CredentialFailure, CredentialValues, Persistence,
    SavedCredentials, StorageFailure, ValidatedAccount,
};
use std::future::Future;
use tallyline_shared_kernel::api::model::Timestamp;

/// Performs read-only account and selected-token validation.
pub trait CredentialValidator {
    /// Reports whether restored service admission is ready for credential checks.
    fn is_available(&self) -> bool {
        true
    }

    /// Returns only the account identity after both checks succeed.
    fn validate(
        &self,
        values: &CredentialValues,
    ) -> impl std::future::Future<Output = Result<ValidatedAccount, CredentialFailure>> + Send;
}

/// Persists one complete credential record atomically.
pub trait SecretStore {
    /// Compares a candidate read-only against protected token associations without saving input.
    fn token_admission(
        &mut self,
        token: &str,
    ) -> impl std::future::Future<Output = Result<model::TokenAdmission, StorageFailure>> + Send
    {
        let scope = self.token_scope(token, Persistence::Session);
        async move { scope.await.map(model::TokenAdmission::from_scope) }
    }

    /// Resolves an exact token inside protected storage without persisting session-only input.
    fn token_scope(
        &mut self,
        _token: &str,
        _persistence: Persistence,
    ) -> impl std::future::Future<Output = Result<Option<model::TokenScope>, StorageFailure>> + Send
    {
        async { Ok(None) }
    }

    /// Reads the required account record without revealing it to a view.
    fn load(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<SavedCredentials>, StorageFailure>> + Send;
    /// Replaces one complete record; an uncertain mutation requires load reconciliation before retry.
    fn save(
        &mut self,
        credentials: &SavedCredentials,
    ) -> impl std::future::Future<Output = Result<(), StorageFailure>> + Send;
    /// Saves revalidation metadata without unlocking or prompting.
    fn save_passive(
        &mut self,
        credentials: &SavedCredentials,
    ) -> impl std::future::Future<Output = Result<(), StorageFailure>> + Send;
    /// Removes only the application's credential record.
    fn remove(&mut self) -> impl std::future::Future<Output = Result<(), StorageFailure>> + Send;
}

/// Owns a candidate until validation and persistence complete.
pub struct ValidationRequest {
    identity: u64,
    candidate: CredentialCandidate,
    revalidation: bool,
    scope_token: Option<String>,
}

impl ValidationRequest {
    /// Compares a candidate with protected token associations without saving or unlocking.
    ///
    /// # Errors
    /// Returns a closed provider failure while leaving the candidate identity unproven.
    pub async fn lookup_token_admission(
        &mut self,
        store: &mut impl SecretStore,
        release_token: Option<&str>,
    ) -> Result<(), StorageFailure> {
        self.candidate.values.token_admission = model::TokenAdmission::default();
        self.scope_token = None;
        let Some(token) = self.candidate.values.own_token().or(release_token) else {
            return Ok(());
        };
        let proof = store.token_admission(token).await?;
        self.scope_token = Some(token.to_owned());
        self.candidate.values.token_admission = proof;
        Ok(())
    }
    /// Runs both read-only checks without publishing or saving the candidate.
    ///
    /// # Errors
    /// Returns a missing-token, input or closed remote-validation failure.
    pub async fn validate(
        &self,
        validator: &impl CredentialValidator,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        self.validate_with_release_token(validator, None).await
    }

    /// Uses the own-token override or borrows release installation data for validation.
    ///
    /// # Errors
    /// Returns a missing-token, input or closed remote-validation failure.
    pub async fn validate_with_release_token(
        &self,
        validator: &impl CredentialValidator,
        release_token: Option<&str>,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        let token = self
            .candidate
            .values
            .own_token()
            .or(release_token)
            .ok_or(CredentialFailure::MissingToken)?;
        let mut selected = CredentialValues::new(
            self.candidate.values.secret_key().to_owned(),
            Some(token.to_owned()),
        )?;
        if self.scope_token.as_deref() == Some(token) {
            selected.token_admission = self.candidate.values.token_admission.clone();
        }
        validator.validate(&selected).await
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ReleaseEligibility {
    Remembered,
    #[default]
    Closed,
}

/// Retains the last accepted credential identity independently of a new candidate.
#[derive(Default)]
pub struct CredentialApplication {
    generation: u64,
    accepted: Option<SavedCredentials>,
    state: AccountState,
    persistence: Option<Persistence>,
    send_authorised: bool,
    release_eligibility: ReleaseEligibility,
    token_binding: Option<(String, model::TokenAdmission)>,
}

impl CredentialApplication {
    /// Restores the last successful account identity from secure storage.
    ///
    /// # Errors
    /// Returns a safe storage failure without exposing a stored record.
    pub async fn load(store: &mut impl SecretStore) -> Result<Self, StorageFailure> {
        let accepted = store.load().await?;
        let state = accepted
            .as_ref()
            .map_or(AccountState::Unconfigured, |saved| {
                account_state(saved, Persistence::Durable)
            });
        let persistence = accepted.as_ref().map(|_| Persistence::Durable);
        Ok(Self {
            generation: 0,
            accepted,
            state,
            persistence,
            send_authorised: false,
            release_eligibility: ReleaseEligibility::Remembered,
            token_binding: None,
        })
    }

    /// Publishes a durable scope only after protected persistence or retains a session-only comparison.
    ///
    /// # Errors
    /// Returns a closed provider failure without publishing a new token identity.
    pub async fn establish_token_scope(
        &mut self,
        store: &mut impl SecretStore,
        release_token: Option<&str>,
        persistence: Persistence,
    ) -> Result<(), StorageFailure> {
        if self.token_scope(release_token).is_some() {
            return Ok(());
        }
        self.token_binding = None;
        if let Some(saved) = &mut self.accepted {
            saved.values.token_admission = model::TokenAdmission::default();
        }
        if !self.send_authorised {
            return Ok(());
        }
        let Some(token) = self
            .accepted
            .as_ref()
            .and_then(|saved| saved.values.own_token().or(release_token))
        else {
            return Ok(());
        };
        let proof = match persistence {
            Persistence::Durable => {
                model::TokenAdmission::from_scope(store.token_scope(token, persistence).await?)
            }
            Persistence::Session => store.token_admission(token).await?,
        };
        self.token_binding = Some((token.to_owned(), proof.clone()));
        if let Some(saved) = &mut self.accepted {
            saved.values.token_admission = proof;
        }
        Ok(())
    }
    /// Returns protected inequality evidence only for the exact currently selected validated token.
    #[must_use]
    pub fn token_admission(&self, release_token: Option<&str>) -> Option<&model::TokenAdmission> {
        if !self.send_authorised {
            return None;
        }
        let token = self
            .accepted
            .as_ref()?
            .values
            .own_token()
            .or(release_token)?;
        self.token_binding
            .as_ref()
            .filter(|(bound, _)| bound == token)
            .map(|(_, proof)| proof)
    }
    /// Returns durable token identity only for the exact currently selected token.
    #[must_use]
    pub fn token_scope(&self, release_token: Option<&str>) -> Option<model::TokenScope> {
        self.token_admission(release_token)
            .and_then(model::TokenAdmission::known_scope)
    }
    /// Returns safe account state without exposing saved credential values.
    #[must_use]
    pub const fn state(&self) -> &AccountState {
        &self.state
    }

    /// Borrows the last validated account eligible for release without granting send authority.
    #[must_use]
    pub fn release_account(&self) -> Option<&str> {
        if self.release_eligibility != ReleaseEligibility::Remembered {
            return None;
        }
        match &self.state {
            AccountState::Validated { username, .. } => Some(username),
            AccountState::Unconfigured | AccountState::Rejected(_) => None,
        }
    }

    /// Borrows accepted values only after successful validation in this session.
    #[must_use]
    pub fn credentials_for(&self, account: &str) -> Option<&CredentialValues> {
        if !self.send_authorised {
            return None;
        }
        match &self.state {
            AccountState::Validated { username, .. } if username == account => {
                let saved = self.accepted.as_ref()?;
                if saved.account.username == account && saved.refusal.is_none() {
                    Some(&saved.values)
                } else {
                    None
                }
            }
            AccountState::Unconfigured
            | AccountState::Rejected(_)
            | AccountState::Validated { .. } => None,
        }
    }

    /// Starts a candidate, closes release and sending, and supersedes earlier validation requests.
    /// The accepted identity remains visible until a current successful candidate replaces it.
    pub fn begin(&mut self, candidate: CredentialCandidate) -> ValidationRequest {
        self.token_binding = None;
        self.send_authorised = false;
        self.release_eligibility = ReleaseEligibility::Closed;
        self.generation = self.generation.wrapping_add(1);
        ValidationRequest {
            identity: self.generation,
            candidate,
            revalidation: false,
            scope_token: None,
        }
    }

    /// Rechecks the accepted identity at startup or before its first send.
    /// Retains existing release eligibility without reopening a replacement candidate.
    pub fn begin_revalidation(&mut self) -> Option<ValidationRequest> {
        let saved = self.accepted.as_ref()?;
        let persistence = self.persistence?;
        let candidate = CredentialCandidate::new(saved.values.clone(), persistence);
        let eligibility = self.release_eligibility;
        let mut request = self.begin(candidate);
        self.release_eligibility = eligibility;
        request.revalidation = true;
        Some(request)
    }

    /// Saves a successful current candidate before publishing its username.
    ///
    /// # Errors
    /// Returns validation, supersession or safe storage failure and retains the accepted record.
    pub async fn accept(
        &mut self,
        request: ValidationRequest,
        result: Result<ValidatedAccount, CredentialFailure>,
        now: Timestamp,
        store: &mut impl SecretStore,
    ) -> Result<(), CredentialFailure> {
        if request.identity != self.generation {
            return Err(CredentialFailure::Superseded);
        }
        let account = match result {
            Ok(account) => account,
            Err(failure) => {
                if request.revalidation && known_refusal(failure) {
                    self.state = AccountState::Rejected(failure);
                    if let Some(saved) = &mut self.accepted {
                        saved.refusal = Some(failure);
                        if request.candidate.persistence == Persistence::Durable {
                            store
                                .save_passive(saved)
                                .await
                                .map_err(CredentialFailure::Storage)?;
                        }
                    }
                }
                return Err(failure);
            }
        };
        let saved = SavedCredentials {
            values: request.candidate.values,
            account,
            validated_at: now,
            refusal: None,
        };
        if request.candidate.persistence == Persistence::Durable {
            let result = if request.revalidation {
                store.save_passive(&saved).await
            } else {
                store.save(&saved).await
            };
            result.map_err(CredentialFailure::Storage)?;
        }
        self.token_binding = request
            .scope_token
            .map(|token| (token, saved.values.token_admission.clone()));
        self.state = account_state(&saved, request.candidate.persistence);
        self.persistence = Some(request.candidate.persistence);
        self.accepted = Some(saved);
        self.send_authorised = true;
        self.release_eligibility = ReleaseEligibility::Remembered;
        Ok(())
    }
}

fn account_state(saved: &SavedCredentials, persistence: Persistence) -> AccountState {
    if let Some(failure) = saved.refusal {
        return AccountState::Rejected(failure);
    }
    AccountState::Validated {
        username: saved.account.username.clone(),
        validated_at: saved.validated_at,
        persistence,
    }
}

fn known_refusal(failure: CredentialFailure) -> bool {
    match failure {
        CredentialFailure::InvalidKey
        | CredentialFailure::NotAllowed
        | CredentialFailure::InvalidToken
        | CredentialFailure::UpdateRequired => true,
        CredentialFailure::InvalidInput
        | CredentialFailure::MissingToken
        | CredentialFailure::Unavailable
        | CredentialFailure::ClientVersionMissing
        | CredentialFailure::Superseded
        | CredentialFailure::Storage(_) => false,
    }
}

pub use crate::screenshot_policy::{ScreenshotPolicyApplication, ScreenshotPolicyStore};

/// Sends one prepared released snapshot through the application's shared client.
pub trait SubmissionGateway {
    /// An adapter-owned body prepared outside asynchronous polling.
    type Prepared: Send;
    /// Serialises and hashes one immutable request on a bounded application worker.
    ///
    /// # Errors
    /// Refuses unsupported payload values without dispatching an attempt.
    fn prepare(
        &self,
        request: &model::SubmissionRequest,
    ) -> Result<(Self::Prepared, model::PayloadHash), model::PreparationFailure>;
    /// Performs exactly one prepared attempt without retrying uncertain writes.
    fn submit(
        &self,
        prepared: Self::Prepared,
        credentials: &model::CredentialValues,
        release_token: Option<&str>,
    ) -> impl std::future::Future<Output = model::SubmissionResult> + Send;
}
pub use crate::budget::SubmissionBudget;
pub use crate::preflight::SubmissionPreflight;

/// Supplies both wall time and one stable monotonic origin for submission metadata.
pub trait SubmissionClock: tallyline_shared_kernel::api::Clock + Send + Sync {
    /// Returns elapsed monotonic time independently of wall-clock adjustments.
    fn monotonic(&self) -> std::time::Duration;
}
pub use crate::application::{SubmissionApplication, SubmissionJobRepository};
pub use crate::schedule::SubmissionSchedule;

/// Performs bounded identity reads only after production acceptance is durable.
pub trait SubmissionReceiptVerifier {
    /// Verifies every unmapped identity against its exact released row and attempted wire date.
    /// A failed or incomplete lookup returns absence without changing known acceptance.
    fn verify_receipts(
        &self,
        job: &model::SubmissionJob,
        credentials: &model::CredentialValues,
        release_token: Option<&str>,
    ) -> impl std::future::Future<Output = Option<Vec<model::VerifiedUexReport>>> + Send;
}

pub use crate::duplicate_evidence::SubmissionEvidence;
pub use crate::observation_order::ObservationOrder;
pub use crate::service_fence::SubmissionServiceFenceRepository;

/// Reads positive identities for an uncertain production attempt without inferring non-receipt.
pub trait SubmissionRowReceiptVerifier {
    /// Returns positively matched rows; empty or unavailable windows establish no negative proof.
    fn verify_row_receipts(
        &self,
        job: &SubmissionJob,
        credentials: &CredentialValues,
        release_token: Option<&str>,
    ) -> impl Future<Output = Option<Vec<VerifiedUexReport>>> + Send;
}

/// Atomically expires independently persisted token quota evidence.
pub trait SubmissionQuotaRepository {
    /// Retains one exact authenticated-read quota answer idempotently without a POST attempt.
    ///
    /// # Errors
    /// Returns unavailable or invalid persistence without publishing a weaker hold.
    fn record_authenticated_quota(
        &mut self,
        observation: model::TokenQuotaObservation,
    ) -> Result<(), model::SubmissionRepositoryError>;
    /// Loads active authenticated-read quota evidence without treating a failed read as empty.
    ///
    /// # Errors
    /// Returns unavailable or corrupt persisted evidence.
    fn authenticated_quotas(
        &mut self,
    ) -> Result<Vec<model::TokenQuotaObservation>, model::SubmissionRepositoryError>;
    /// Checks independent global and exact-token service evidence without granting send authority.
    ///
    /// # Errors
    /// Refuses unavailable or corrupt service history.
    fn quota_allows(
        &mut self,
        scope: Option<model::TokenScope>,
    ) -> Result<bool, model::SubmissionRepositoryError>;

    /// Checks a bounded protected token distinction without persisting its proof.
    ///
    /// # Errors
    /// Refuses unavailable or corrupt service history.
    fn quota_allows_admission(
        &mut self,
        proof: &model::TokenAdmission,
    ) -> Result<bool, model::SubmissionRepositoryError> {
        self.quota_allows(proof.known_scope())
    }

    /// Commits due quota expiry and eligible in-session resumption without dispatching HTTP.
    ///
    /// # Errors
    /// Retains every fence and job if an exact-source transaction cannot commit.
    fn expire_quotas(
        &mut self,
        now: Timestamp,
    ) -> Result<Vec<SubmissionJob>, model::SubmissionRepositoryError>;
}

/// Reads a bounded account batch without changing delivery or lifecycle persistence.
pub trait SubmissionLifecycleReader {
    /// Returns positive exact row values while retaining ownership of the admitted batch.
    ///
    /// # Errors
    /// Returns `Unavailable` when admission, credentials, transport, quota or processing
    /// prevents a usable answer, or `InvalidResponse` for malformed, ambiguous or
    /// undocumented answer data; neither failure supplies lifecycle evidence.
    fn read_lifecycle(
        &self,
        batch: model::LifecycleReadBatch,
        credentials: &CredentialValues,
        release_token: Option<&str>,
    ) -> impl Future<Output = Result<model::LifecycleReadResponse, model::LifecycleReadFailure>> + Send;
}
