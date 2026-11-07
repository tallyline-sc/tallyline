// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Credential application acceptance tests.

use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    AccountState, CredentialCandidate, CredentialFailure, CredentialValues, Persistence,
    SavedCredentials, StorageFailure, ValidatedAccount,
};
use tallyline_submission::api::{CredentialApplication, CredentialValidator, SecretStore};

struct Validator;
impl CredentialValidator for Validator {
    async fn validate(&self, _: &CredentialValues) -> Result<ValidatedAccount, CredentialFailure> {
        Ok(ValidatedAccount::new("dummy-runner".to_owned()))
    }
}

#[derive(Default)]
struct MemoryStore {
    credentials: Option<SavedCredentials>,
    fail_write: bool,
}
impl SecretStore for MemoryStore {
    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        Ok(self.credentials.clone())
    }
    async fn save(&mut self, credentials: &SavedCredentials) -> Result<(), StorageFailure> {
        if self.fail_write {
            return Err(StorageFailure::WriteFailed);
        }
        self.credentials = Some(credentials.clone());
        Ok(())
    }
    async fn save_passive(&mut self, saved: &SavedCredentials) -> Result<(), StorageFailure> {
        self.save(saved).await
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        self.credentials = None;
        Ok(())
    }
}

fn candidate(persistence: Persistence) -> Result<CredentialCandidate, CredentialFailure> {
    Ok(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))?,
        persistence,
    ))
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_4__missing_release_and_own_token_is_actionable_before_validation() {
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
        Persistence::Durable,
    ));
    assert_eq!(
        request.validate(&Validator).await,
        Err(CredentialFailure::MissingToken)
    );
}

struct ReleaseValidator;
impl CredentialValidator for ReleaseValidator {
    async fn validate(
        &self,
        values: &CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        assert_eq!(values.own_token(), Some("dummy-release-token"));
        Ok(ValidatedAccount::new("dummy-release-runner".to_owned()))
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_4__superseded_or_refused_candidates_never_replace_a_saved_identity() {
    let mut application = CredentialApplication::default();
    let mut store = MemoryStore::default();
    let old = application.begin(candidate(Persistence::Durable).unwrap());
    let current = application.begin(candidate(Persistence::Durable).unwrap());
    let result = old.validate(&Validator).await;
    assert_eq!(
        application
            .accept(old, result, Timestamp::from_seconds(10), &mut store)
            .await,
        Err(CredentialFailure::Superseded)
    );
    assert_eq!(
        application
            .accept(
                current,
                Err(CredentialFailure::NotAllowed),
                Timestamp::from_seconds(10),
                &mut store
            )
            .await,
        Err(CredentialFailure::NotAllowed)
    );
    assert_eq!(application.state(), &AccountState::Unconfigured);
    assert!(store.load().await.unwrap().is_none());
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_7__offline_revalidation_keeps_the_last_successful_account() {
    let mut application = CredentialApplication::default();
    let mut store = MemoryStore::default();
    let request = application.begin(candidate(Persistence::Durable).unwrap());
    let result = request.validate(&Validator).await;
    application
        .accept(request, result, Timestamp::from_seconds(10), &mut store)
        .await
        .unwrap();
    let previous = application.state().clone();
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::Unavailable),
                Timestamp::from_seconds(20),
                &mut store
            )
            .await,
        Err(CredentialFailure::Unavailable)
    );
    assert_eq!(application.state(), &previous);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__credential_input_is_bounded() {
    for key in [String::new(), "dummy\nkey".to_owned(), "x".repeat(1025)] {
        assert_eq!(
            CredentialValues::new(key, None).err(),
            Some(CredentialFailure::InvalidInput)
        );
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_4_r_nf_4__release_token_is_used_without_copying_it_into_secure_storage() {
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
        Persistence::Durable,
    ));
    let result = request
        .validate_with_release_token(&ReleaseValidator, Some("dummy-release-token"))
        .await;
    let mut store = MemoryStore::default();
    application
        .accept(request, result, Timestamp::from_seconds(100), &mut store)
        .await
        .unwrap();
    assert_eq!(store.credentials.unwrap().values().own_token(), None);
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_7__known_invalid_current_credentials_remain_blocked_after_offline_revalidation() {
    let mut application = CredentialApplication::default();
    let mut store = MemoryStore::default();
    let request = application.begin(candidate(Persistence::Durable).unwrap());
    let result = request.validate(&Validator).await;
    application
        .accept(request, result, Timestamp::from_seconds(10), &mut store)
        .await
        .unwrap();
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::InvalidKey),
                Timestamp::from_seconds(20),
                &mut store
            )
            .await,
        Err(CredentialFailure::InvalidKey)
    );
    let mut restored = CredentialApplication::load(&mut store).await.unwrap();
    assert_eq!(restored.release_account(), None);
    let request = restored.begin_revalidation().unwrap();
    assert_eq!(
        restored
            .accept(
                request,
                Err(CredentialFailure::Unavailable),
                Timestamp::from_seconds(30),
                &mut store
            )
            .await,
        Err(CredentialFailure::Unavailable)
    );
    assert_eq!(
        restored.state(),
        &AccountState::Rejected(CredentialFailure::InvalidKey)
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__session_credentials_never_become_durable_after_revalidation() {
    let mut application = CredentialApplication::default();
    let mut store = MemoryStore::default();
    let request = application.begin(candidate(Persistence::Session).unwrap());
    let result = request.validate(&Validator).await;
    application
        .accept(request, result, Timestamp::from_seconds(10), &mut store)
        .await
        .unwrap();
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::InvalidKey),
                Timestamp::from_seconds(20),
                &mut store
            )
            .await,
        Err(CredentialFailure::InvalidKey)
    );
    let request = application.begin_revalidation().unwrap();
    let result = request.validate(&Validator).await;
    application
        .accept(request, result, Timestamp::from_seconds(30), &mut store)
        .await
        .unwrap();
    assert!(store.load().await.unwrap().is_none());
    assert!(matches!(
        application.state(),
        AccountState::Validated {
            persistence: Persistence::Session,
            ..
        }
    ));
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_4_r_nf_4__failed_replacement_preserves_the_previous_account_and_store() {
    let mut application = CredentialApplication::default();
    let mut store = MemoryStore::default();
    let request = application.begin(candidate(Persistence::Durable).unwrap());
    let result = request.validate(&Validator).await;
    application
        .accept(request, result, Timestamp::from_seconds(10), &mut store)
        .await
        .unwrap();
    let previous = application.state().clone();
    store.fail_write = true;
    let request = application.begin(candidate(Persistence::Durable).unwrap());
    let result = Ok(ValidatedAccount::new("different-dummy-runner".to_owned()));
    assert_eq!(
        application
            .accept(request, result, Timestamp::from_seconds(20), &mut store)
            .await,
        Err(CredentialFailure::Storage(StorageFailure::WriteFailed))
    );
    assert_eq!(application.state(), &previous);
    let restored = CredentialApplication::load(&mut store).await.unwrap();
    assert_eq!(restored.state(), &previous);
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_4_r_nf_4__validated_credentials_survive_application_reload() {
    let mut application = CredentialApplication::default();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        Persistence::Durable,
    ));
    let validated = request.validate(&Validator).await;
    let mut store = MemoryStore::default();
    assert_eq!(
        application
            .accept(request, validated, Timestamp::from_seconds(100), &mut store)
            .await,
        Ok(())
    );
    let text = format!("{:?}", application.state());
    assert!(!text.contains("dummy-key"));
    assert!(!text.contains("dummy-token"));
    let restored = CredentialApplication::load(&mut store).await.unwrap();
    assert_eq!(
        restored.state(),
        &AccountState::Validated {
            username: "dummy-runner".to_owned(),
            validated_at: Timestamp::from_seconds(100),
            persistence: Persistence::Durable,
        }
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_7_r_nf_4__secure_record_reconstruction_refuses_non_account_failure_states() {
    for refusal in [
        CredentialFailure::InvalidInput,
        CredentialFailure::MissingToken,
        CredentialFailure::ClientVersionMissing,
        CredentialFailure::Unavailable,
        CredentialFailure::Superseded,
        CredentialFailure::Storage(StorageFailure::Locked),
    ] {
        assert_eq!(
            SavedCredentials::from_storage(
                CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
                ValidatedAccount::new("dummy-runner".to_owned()),
                Timestamp::from_seconds(1),
                Some(refusal),
            )
            .err(),
            Some(StorageFailure::Corrupt)
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_7_r_nf_4__secure_record_reconstruction_retains_only_required_account_metadata() {
    for refusal in [
        None,
        Some(CredentialFailure::InvalidKey),
        Some(CredentialFailure::NotAllowed),
        Some(CredentialFailure::InvalidToken),
        Some(CredentialFailure::UpdateRequired),
    ] {
        let saved = SavedCredentials::from_storage(
            CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
            ValidatedAccount::new("dummy-runner".to_owned()),
            Timestamp::from_millis(12345),
            refusal,
        )
        .unwrap();
        assert_eq!(saved.username(), "dummy-runner");
        assert_eq!(saved.validated_at(), Timestamp::from_millis(12345));
        assert_eq!(saved.refusal(), refusal);
        assert_eq!(saved.values().own_token(), None);
    }
}

struct PassiveOnly {
    credentials: Option<SavedCredentials>,
}
impl SecretStore for PassiveOnly {
    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        Ok(self.credentials.clone())
    }
    async fn save(&mut self, _: &SavedCredentials) -> Result<(), StorageFailure> {
        Err(StorageFailure::Locked)
    }
    async fn save_passive(&mut self, saved: &SavedCredentials) -> Result<(), StorageFailure> {
        self.credentials = Some(saved.clone());
        Ok(())
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        Ok(())
    }
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_7_r_ui_16__startup_revalidation_persists_only_without_interaction() {
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let saved = SavedCredentials::from_storage(
        values,
        ValidatedAccount::new("dummy-runner".to_owned()),
        Timestamp::from_seconds(10),
        None,
    )
    .unwrap();
    let mut store = PassiveOnly {
        credentials: Some(saved),
    };
    let mut application = CredentialApplication::load(&mut store).await.unwrap();
    let request = application.begin_revalidation().unwrap();
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_seconds(20),
            &mut store,
        )
        .await
        .unwrap();
    assert!(
        matches!(CredentialApplication::load(&mut store).await.unwrap().state(), AccountState::Validated { validated_at, .. } if *validated_at == Timestamp::from_seconds(20))
    );
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::InvalidKey),
                Timestamp::from_seconds(30),
                &mut store
            )
            .await,
        Err(CredentialFailure::InvalidKey)
    );
    assert_eq!(
        CredentialApplication::load(&mut store)
            .await
            .unwrap()
            .state(),
        &AccountState::Rejected(CredentialFailure::InvalidKey)
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_8_r_sub_12__submission_credentials_are_borrowed_only_for_the_exact_accepted_account()
{
    let mut application = CredentialApplication::default();
    let mut store = MemoryStore::default();
    assert!(application.credentials_for("dummy-a").is_none());
    assert_ne!(application.release_account(), Some("dummy-a"));
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key-a".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        Persistence::Session,
    ));
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-a".to_owned())),
            Timestamp::from_seconds(10),
            &mut store,
        )
        .await
        .unwrap();
    assert_eq!(
        application
            .credentials_for("dummy-a")
            .map(CredentialValues::secret_key),
        Some("dummy-key-a")
    );
    assert!(application.credentials_for("dummy-b").is_none());
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key-b".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        Persistence::Session,
    ));
    assert!(application.credentials_for("dummy-a").is_none());
    assert_ne!(application.release_account(), Some("dummy-a"));
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-b".to_owned())),
            Timestamp::from_seconds(20),
            &mut store,
        )
        .await
        .unwrap();
    assert!(application.credentials_for("dummy-a").is_none());
    assert_ne!(application.release_account(), Some("dummy-a"));
    assert_eq!(
        application
            .credentials_for("dummy-b")
            .map(CredentialValues::secret_key),
        Some("dummy-key-b")
    );
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::InvalidKey),
                Timestamp::from_seconds(30),
                &mut store
            )
            .await,
        Err(CredentialFailure::InvalidKey)
    );
    assert!(application.credentials_for("dummy-b").is_none());
    assert!(store.credentials.is_none());
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_8_r_api_7__restored_identity_needs_session_validation_before_submission() {
    let mut store = MemoryStore::default();
    let mut original = CredentialApplication::default();
    let request = original.begin(candidate(Persistence::Durable).unwrap());
    original
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_seconds(10),
            &mut store,
        )
        .await
        .unwrap();
    let mut application = CredentialApplication::load(&mut store).await.unwrap();
    assert_eq!(application.state(), original.state());
    assert!(application.credentials_for("dummy-runner").is_none());
    let request = application.begin_revalidation().unwrap();
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_seconds(20),
            &mut store,
        )
        .await
        .unwrap();
    assert!(application.credentials_for("dummy-runner").is_some());
    let previous = application.state().clone();
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::Unavailable),
                Timestamp::from_seconds(30),
                &mut store
            )
            .await,
        Err(CredentialFailure::Unavailable)
    );
    assert_eq!(application.state(), &previous);
    assert!(application.credentials_for("dummy-runner").is_none());
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_8_r_api_7__only_current_success_can_restore_send_authority() {
    let mut application = CredentialApplication::default();
    let mut store = MemoryStore::default();
    let request = application.begin(candidate(Persistence::Session).unwrap());
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_seconds(10),
            &mut store,
        )
        .await
        .unwrap();
    let request = application.begin(candidate(Persistence::Session).unwrap());
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::InvalidKey),
                Timestamp::from_seconds(20),
                &mut store
            )
            .await,
        Err(CredentialFailure::InvalidKey)
    );
    assert!(application.credentials_for("dummy-runner").is_none());
    let stale = application.begin_revalidation().unwrap();
    assert!(application.credentials_for("dummy-runner").is_none());
    let current = application.begin(candidate(Persistence::Session).unwrap());
    assert_eq!(
        application
            .accept(
                stale,
                Ok(ValidatedAccount::new("dummy-runner".to_owned())),
                Timestamp::from_seconds(30),
                &mut store
            )
            .await,
        Err(CredentialFailure::Superseded)
    );
    assert!(application.credentials_for("dummy-runner").is_none());
    application
        .accept(
            current,
            Ok(ValidatedAccount::new("dummy-new".to_owned())),
            Timestamp::from_seconds(40),
            &mut store,
        )
        .await
        .unwrap();
    assert!(application.credentials_for("dummy-runner").is_none());
    assert!(application.credentials_for("dummy-new").is_some());
    let stale = application.begin_revalidation().unwrap();
    let current = application.begin(candidate(Persistence::Session).unwrap());
    application
        .accept(
            current,
            Ok(ValidatedAccount::new("dummy-new".to_owned())),
            Timestamp::from_seconds(50),
            &mut store,
        )
        .await
        .unwrap();
    assert_eq!(
        application
            .accept(
                stale,
                Err(CredentialFailure::InvalidKey),
                Timestamp::from_seconds(60),
                &mut store
            )
            .await,
        Err(CredentialFailure::Superseded)
    );
    assert!(application.credentials_for("dummy-new").is_some());
    let request = application.begin_revalidation().unwrap();
    assert_eq!(
        application
            .accept(
                request,
                Err(CredentialFailure::MissingToken),
                Timestamp::from_seconds(70),
                &mut store
            )
            .await,
        Err(CredentialFailure::MissingToken)
    );
    assert!(application.credentials_for("dummy-new").is_none());
    assert!(
        matches!(application.state(), AccountState::Validated { username, .. } if username == "dummy-new")
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_7_r_sub_8_r_sub_12__candidate_failure_keeps_saved_identity_but_cannot_keep_old_send_authority()
 {
    for failure in [
        CredentialFailure::Unavailable,
        CredentialFailure::InvalidKey,
        CredentialFailure::InvalidToken,
        CredentialFailure::NotAllowed,
        CredentialFailure::MissingToken,
    ] {
        let mut application = CredentialApplication::default();
        let mut store = MemoryStore::default();
        let first = application.begin(candidate(Persistence::Durable).unwrap());
        application
            .accept(
                first,
                Ok(ValidatedAccount::new("dummy-runner".to_owned())),
                Timestamp::from_seconds(10),
                &mut store,
            )
            .await
            .unwrap();
        let previous = application.state().clone();
        assert!(application.credentials_for("dummy-runner").is_some());
        assert_eq!(application.release_account(), Some("dummy-runner"));
        let request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key-b".to_owned(), Some("dummy-token".to_owned()))
                .unwrap(),
            Persistence::Durable,
        ));
        assert_eq!(application.state(), &previous);
        assert_eq!(application.release_account(), None);
        assert!(
            application.credentials_for("dummy-runner").is_none(),
            "a pending candidate must close old-account dispatch"
        );
        assert_eq!(
            application
                .accept(
                    request,
                    Err(failure),
                    Timestamp::from_seconds(20),
                    &mut store
                )
                .await,
            Err(failure)
        );
        assert_eq!(application.state(), &previous);
        assert_eq!(application.release_account(), None);
        assert!(application.credentials_for("dummy-runner").is_none());
        let saved = store.load().await.unwrap().unwrap();
        assert_eq!(saved.username(), "dummy-runner");
        assert_eq!(saved.values().secret_key(), "dummy-key");
        let revalidated = application.begin_revalidation().unwrap();
        assert_eq!(application.release_account(), None);
        application
            .accept(
                revalidated,
                Ok(ValidatedAccount::new("dummy-runner".to_owned())),
                Timestamp::from_seconds(30),
                &mut store,
            )
            .await
            .unwrap();
        assert!(application.credentials_for("dummy-runner").is_some());
        assert_eq!(application.release_account(), Some("dummy-runner"));
    }
}
