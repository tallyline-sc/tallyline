// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Credential values and closed account outcomes.

use tallyline_shared_kernel::api::model::Timestamp;

const MAX_CREDENTIAL_BYTES: usize = 1024;

/// The installation token, borrowed only for authenticated validation requests.
pub struct ReleaseToken(String);

impl ReleaseToken {
    /// Accepts a bounded non-empty printable ASCII installation token.
    ///
    /// # Errors
    /// Returns an input failure for malformed token data.
    pub fn new(value: String) -> Result<Self, CredentialFailure> {
        if !valid_secret(&value) {
            return Err(CredentialFailure::InvalidInput);
        }
        Ok(Self(value))
    }

    /// Borrows installation data for a credential validation boundary.
    #[must_use]
    pub fn for_validation(&self) -> &str {
        &self.0
    }
}

/// A safe failure to read installation token data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseTokenFailure {
    /// Installation data could not be read.
    Unreadable,
    /// The installation data is not a bounded token.
    Malformed,
}

/// Credentials retained only at the secure storage and validation boundaries.
#[derive(Clone, PartialEq, Eq)]
pub struct CredentialValues {
    key: String,
    own_token: Option<String>,
    pub(crate) token_admission: TokenAdmission,
}

impl CredentialValues {
    /// Accepts bounded non-empty printable ASCII credentials.
    ///
    /// # Errors
    /// Returns an input failure without retaining malformed credentials.
    pub fn new(key: String, own_token: Option<String>) -> Result<Self, CredentialFailure> {
        if !valid_secret(&key)
            || own_token
                .as_deref()
                .is_some_and(|token| !valid_secret(token))
        {
            return Err(CredentialFailure::InvalidInput);
        }
        Ok(Self {
            key,
            own_token,
            token_admission: TokenAdmission::default(),
        })
    }

    /// Borrows exact protected token inequality evidence without exposing token values.
    #[must_use]
    pub const fn token_admission(&self) -> &TokenAdmission {
        &self.token_admission
    }
    /// Returns nonsecret scope metadata minted by the protected credential boundary.
    #[must_use]
    pub const fn token_scope(&self) -> Option<TokenScope> {
        self.token_admission.known_scope()
    }
    /// Borrows the key exclusively for a credential boundary operation.
    #[must_use]
    pub fn secret_key(&self) -> &str {
        &self.key
    }

    /// Borrows an own token exclusively for a credential boundary operation.
    #[must_use]
    pub fn own_token(&self) -> Option<&str> {
        self.own_token.as_deref()
    }
}

fn valid_secret(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CREDENTIAL_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}

/// The explicitly requested storage lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Persistence {
    /// Retain credentials securely across restarts.
    Durable,
    /// Retain credentials in memory until application exit.
    Session,
}

/// A proposed credential replacement which has not yet been validated.
pub struct CredentialCandidate {
    pub(crate) values: CredentialValues,
    pub(crate) persistence: Persistence,
}

impl CredentialCandidate {
    /// Returns the explicitly requested storage lifetime.
    #[must_use]
    pub const fn persistence(&self) -> Persistence {
        self.persistence
    }
    /// Creates a candidate with an explicit storage lifetime.
    #[must_use]
    pub const fn new(values: CredentialValues, persistence: Persistence) -> Self {
        Self {
            values,
            persistence,
        }
    }
}

/// A username established by the account and token validation calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedAccount {
    pub(crate) username: String,
}

impl ValidatedAccount {
    /// Creates the reduced account result after validating DataRunner standing.
    #[must_use]
    pub const fn new(username: String) -> Self {
        Self { username }
    }
}

/// A versioned secure-store record containing only required account data.
#[derive(Clone, PartialEq, Eq)]
pub struct SavedCredentials {
    pub(crate) values: CredentialValues,
    pub(crate) account: ValidatedAccount,
    pub(crate) validated_at: Timestamp,
    pub(crate) refusal: Option<CredentialFailure>,
}

impl SavedCredentials {
    /// Reconstructs a required account record at the secure storage boundary.
    ///
    /// # Errors
    /// Returns corruption when a persisted refusal is not an account refusal.
    pub fn from_storage(
        values: CredentialValues,
        account: ValidatedAccount,
        validated_at: Timestamp,
        refusal: Option<CredentialFailure>,
    ) -> Result<Self, StorageFailure> {
        if refusal.is_some_and(|failure| !super::known_refusal(failure)) {
            return Err(StorageFailure::Corrupt);
        }
        Ok(Self {
            values,
            account,
            validated_at,
            refusal,
        })
    }

    /// Borrows only the account identity for secure-store encoding.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.account.username
    }

    /// Returns the last successful account validation time.
    #[must_use]
    pub const fn validated_at(&self) -> Timestamp {
        self.validated_at
    }

    /// Returns the persisted account refusal, if any.
    #[must_use]
    pub const fn refusal(&self) -> Option<CredentialFailure> {
        self.refusal
    }

    /// Borrows credential values only for secure-store encoding.
    #[must_use]
    pub const fn values(&self) -> &CredentialValues {
        &self.values
    }
}

/// Safe account information that can be shown without revealing credentials.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AccountState {
    /// No successfully validated credential identity is available.
    #[default]
    Unconfigured,
    /// The current identity is known to be refused and cannot authorise release.
    Rejected(CredentialFailure),
    /// The current identity passed both read-only checks.
    Validated {
        /// The validated UEX account identity.
        username: String,
        /// The last successful validation time.
        validated_at: Timestamp,
        /// The accepted credential lifetime.
        persistence: Persistence,
    },
}

/// A closed validation outcome containing no remote message or secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialFailure {
    /// Credential input is empty, excessive or unsuitable for an HTTP header.
    InvalidInput,
    /// The user's key was refused.
    InvalidKey,
    /// The account may not submit DataRunner reports.
    NotAllowed,
    /// The selected app token was refused.
    InvalidToken,
    /// No release or own app token is available.
    MissingToken,
    /// This client must be updated before validation can complete.
    UpdateRequired,
    /// UEX reported that the required client-version header was absent.
    ClientVersionMissing,
    /// UEX could not be reached or returned an unusable response.
    Unavailable,
    /// A newer request superseded this result.
    Superseded,
    /// The requested secure persistence failed.
    Storage(StorageFailure),
}

/// A safe secure-store outcome without dependency error payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageFailure {
    /// No supported secure store is available.
    Unavailable,
    /// The store is locked or access was declined.
    Locked,
    /// The stored record is malformed or ambiguous.
    Corrupt,
    /// A dispatched mutation may have committed; reconcile storage before another mutation.
    OutcomeUnknown,
    /// The requested write could not be committed.
    WriteFailed,
}

/// Account evidence learned only from an actual documented submission answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenshotRequirement {
    /// UEX answered `screenshot_required` for this validated account.
    Required,
}
/// Current account-scoped knowledge; absence never invents a requirement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScreenshotPolicy {
    /// No verified requirement is known; a manual Report may omit its image.
    #[default]
    Unknown,
    /// A documented actual response established this account's requirement.
    Required {
        /// Date of the newest observed `screenshot_required` answer.
        observed_at: Timestamp,
    },
}

pub use crate::budget::{AccountAttempt, RowBudgetDecision};
pub use crate::job::NotProcessed;
pub use crate::job::SubmissionResult;
pub use crate::job::SubmissionReturnReason;
pub use crate::job::{
    AttemptOutcome, JobId, JobPhase, PayloadHash, QueueRefusal, SubmissionAttempt, SubmissionJob,
};
pub use crate::preflight::{
    SendHoldReason, SendReadiness, SendReferenceState, SendReturnReason, SubmissionAcceptance,
};
pub use crate::receipt::VerifiedUexReport;
pub use crate::request::{InvalidSubmission, SubmissionRow};
pub use crate::request::{SubmissionHeader, SubmissionRequest};

/// A preparation refusal before any network dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationFailure {
    /// The immutable request cannot be represented as the documented payload.
    InvalidPayload,
}
pub use crate::application::{SubmissionApplicationError, SubmissionRepositoryError};
pub use crate::events::{CancellationReason, SubmissionReturn};
pub use crate::events::{
    SubmissionChange, SubmissionCommand, SubmissionEvent, SubmissionEventKind,
};
pub use crate::job::{StoredSubmissionAttempt, StoredSubmissionJob};
pub use crate::request::NewerObservationConfirmation;
pub use crate::schedule::{CooldownPartition, RowCooldown, ScheduleDecision};
pub use crate::server_time::ServerTimeEstimate;

pub use crate::receipt_mapping::VerifiedReceiptMapping;

pub use crate::observation_order::ObservationOrderBlock;

pub use crate::queue_wait::{SubmissionWait, SubmissionWaitReason};

pub use crate::job::SubmissionPauseReason;

pub use crate::job::SubmissionHoldReason;
pub use crate::retry::SubmissionRetrySettings;
pub use crate::service_fence::{SubmissionServiceCause, SubmissionServiceFence};
pub use crate::uex_error_code::{RowFailureKind, RowFieldConflict, UexErrorCode};

pub use crate::submission_account_evidence::{SubmissionAccountCheck, SubmissionAccountEvidence};
pub use crate::submission_rejection::{SubmissionCorrection, SubmissionRejection};

pub use crate::submission_resolution_action::{ReceiptDecision, SubmissionResolutionAction};

pub use crate::submission_row_result::SubmissionRowResult;

pub use crate::row_failure_source::RowFailureSource;

pub use crate::token_scope::TokenScope;

pub use crate::token_quota_reset::TokenQuotaReset;

pub use crate::token_quota_observation::{TokenQuotaObservation, TokenQuotaObservations};

pub use crate::token_admission::TokenAdmission;

pub use crate::submission_lifecycle::{
    LifecycleReadClaim, StoredLifecycleRowAdmission, StoredSubmissionLifecycle, SubmissionLifecycle,
};
pub use crate::uex_lifecycle_observation::{
    LifecycleRefresh, RowEditability, UexLifecycleObservation, UexLifecycleRow, UexReportStatus,
};

pub use crate::submission_lifecycle_settings::SubmissionLifecycleSettings;

pub use crate::lifecycle_read_batch::LifecycleReadBatch;

pub use crate::lifecycle_read_response::{
    CompletedLifecycleClaim, CompletedLifecycleRead, LifecycleReadFailure, LifecycleReadResponse,
};
