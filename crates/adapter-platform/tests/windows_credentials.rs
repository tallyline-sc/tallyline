// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Windows secure-store contracts using an isolated injected provider.

#![cfg(windows)]

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use keyring_core::api::{CredentialApi, CredentialStoreApi};
use keyring_core::{Entry, Error, Result as KeyringResult};
use tallyline_adapter_platform::api::WindowsCredentialStore;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    AccountState, CredentialCandidate, CredentialFailure, CredentialValues, Persistence,
    SavedCredentials, StorageFailure, ValidatedAccount,
};
use tallyline_submission::api::{CredentialApplication, SecretStore};

const BUNDLE: &[u8] = b"TLCR\x01\x09\x00\x0b\x00\x0c\x00\x39\x30\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00dummy-keydummy-tokendummy-runner";

#[derive(Clone, Copy)]
enum Fault {
    Unavailable,
    Locked,
    BadEncoding,
    BadFormat,
    Ambiguous,
    Unsupported,
    Invalid,
}

impl Fault {
    fn error(self) -> Error {
        match self {
            Self::Unavailable => {
                Error::PlatformFailure(std::io::Error::other("dummy-private-error").into())
            }
            Self::Locked => {
                Error::NoStorageAccess(std::io::Error::other("dummy-private-error").into())
            }
            Self::BadEncoding => Error::BadEncoding(b"dummy-private-error".to_vec()),
            Self::BadFormat => Error::BadDataFormat(
                b"dummy-private-error".to_vec(),
                std::io::Error::other("dummy-private-error").into(),
            ),
            Self::Ambiguous => Error::Ambiguous(Vec::new()),
            Self::Unsupported => Error::NotSupportedByStore("dummy-private-error".to_owned()),
            Self::Invalid => Error::Invalid(
                "dummy-private-error".to_owned(),
                "dummy-private-error".to_owned(),
            ),
        }
    }
}

#[derive(Default)]
struct Provider(Arc<Credential>, Arc<Credential>);

#[derive(Default)]
struct Credential {
    bytes: Mutex<Option<Vec<u8>>>,
    read_fault: Mutex<Option<Fault>>,
    write_fault: Mutex<Option<Fault>>,
    build_fault: Mutex<Option<Fault>>,
    remove_fault: Mutex<Option<Fault>>,
}

impl CredentialStoreApi for Provider {
    fn vendor(&self) -> String {
        "isolated".to_owned()
    }
    fn id(&self) -> String {
        "isolated".to_owned()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        modifiers: Option<&HashMap<&str, &str>>,
    ) -> KeyringResult<Entry> {
        assert_eq!(service, "Tallyline");
        assert!(matches!(user, "uex-credentials" | "uex-token-associations"));
        assert_eq!(modifiers, Some(&HashMap::from([("persistence", "Local")])));
        if let Some(fault) = *self.0.build_fault.lock().map_err(poisoned)? {
            return Err(fault.error());
        }
        Ok(Entry::new_with_credential(if user == "uex-credentials" {
            self.0.clone()
        } else {
            self.1.clone()
        }))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CredentialApi for Credential {
    fn set_secret(&self, secret: &[u8]) -> KeyringResult<()> {
        assert!(!secret.is_empty());
        assert!(secret.len() <= 2560);
        if let Some(fault) = *self.write_fault.lock().map_err(poisoned)? {
            return Err(fault.error());
        }
        *self.bytes.lock().map_err(poisoned)? = Some(secret.to_vec());
        Ok(())
    }
    fn get_secret(&self) -> KeyringResult<Vec<u8>> {
        if let Some(fault) = *self.read_fault.lock().map_err(poisoned)? {
            return Err(fault.error());
        }
        self.bytes
            .lock()
            .map_err(poisoned)?
            .clone()
            .ok_or(Error::NoEntry)
    }
    fn delete_credential(&self) -> KeyringResult<()> {
        if let Some(fault) = *self.remove_fault.lock().map_err(poisoned)? {
            return Err(fault.error());
        }
        self.bytes
            .lock()
            .map_err(poisoned)?
            .take()
            .map(|_| ())
            .ok_or(Error::NoEntry)
    }
    fn get_credential(&self) -> KeyringResult<Option<Arc<keyring_core::Credential>>> {
        Ok(None)
    }
    fn get_specifiers(&self) -> Option<(String, String)> {
        None
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn poisoned<T>(_: std::sync::PoisonError<T>) -> Error {
    Error::PlatformFailure(std::io::Error::other("isolated provider lock poisoned").into())
}

fn candidate(persistence: Persistence) -> Result<CredentialCandidate, CredentialFailure> {
    Ok(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))?,
        persistence,
    ))
}

async fn accept(
    application: &mut CredentialApplication,
    store: &mut WindowsCredentialStore,
    persistence: Persistence,
) -> Result<(), CredentialFailure> {
    let request = application.begin(candidate(persistence)?);
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-runner".to_owned())),
            Timestamp::from_millis(12345),
            store,
        )
        .await
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_4_r_nf_4__owned_local_store_restores_the_validated_account_and_secret_values() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let mut application = CredentialApplication::default();
    assert!(store.load().await.unwrap().is_none());
    accept(&mut application, &mut store, Persistence::Durable)
        .await
        .unwrap();
    assert_eq!(provider.0.bytes.lock().unwrap().as_deref(), Some(BUNDLE));
    let mut reopened = WindowsCredentialStore::with_provider(&provider).unwrap();
    let restored = CredentialApplication::load(&mut reopened).await.unwrap();
    assert_eq!(restored.state(), application.state());
    let saved = reopened.load().await.unwrap().unwrap();
    assert_eq!(saved.values().secret_key(), "dummy-key");
    assert_eq!(saved.values().own_token(), Some("dummy-token"));
    reopened.remove().await.unwrap();
    reopened.remove().await.unwrap();
    assert!(store.load().await.unwrap().is_none());
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__unavailable_provider_read_returns_a_closed_actionable_failure() {
    let provider = Provider::default();
    *provider.0.read_fault.lock().unwrap() = Some(Fault::Unavailable);
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    assert_eq!(store.load().await.err(), Some(StorageFailure::Unavailable));
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__provider_payloads_never_escape_as_application_errors() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    for (fault, expected) in [
        (Fault::Locked, StorageFailure::Locked),
        (Fault::BadEncoding, StorageFailure::Corrupt),
        (Fault::BadFormat, StorageFailure::Corrupt),
        (Fault::Ambiguous, StorageFailure::Corrupt),
        (Fault::Unsupported, StorageFailure::Unavailable),
        (Fault::Invalid, StorageFailure::Unavailable),
    ] {
        *provider.0.read_fault.lock().unwrap() = Some(fault);
        let failure = store.load().await.err().unwrap();
        assert_eq!(failure, expected);
        assert!(!format!("{failure:?}").contains("dummy-private-error"));
    }
    *provider.0.build_fault.lock().unwrap() = Some(Fault::Unavailable);
    assert_eq!(
        WindowsCredentialStore::with_provider(&provider).err(),
        Some(StorageFailure::Unavailable)
    );
    *provider.0.remove_fault.lock().unwrap() = Some(Fault::Locked);
    assert_eq!(store.remove().await, Err(StorageFailure::Locked));
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__failed_durable_choice_preserves_the_accepted_session_choice() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let mut application = CredentialApplication::default();
    accept(&mut application, &mut store, Persistence::Session)
        .await
        .unwrap();
    let previous = application.state().clone();
    for fault in [Fault::Unavailable, Fault::Locked, Fault::Invalid] {
        *provider.0.write_fault.lock().unwrap() = Some(fault);
        assert!(matches!(
            accept(&mut application, &mut store, Persistence::Durable).await,
            Err(CredentialFailure::Storage(_))
        ));
        assert_eq!(application.state(), &previous);
        let request = application.begin_revalidation().unwrap();
        application
            .accept(
                request,
                Ok(ValidatedAccount::new("dummy-runner".to_owned())),
                Timestamp::from_millis(12345),
                &mut store,
            )
            .await
            .unwrap();
        assert_eq!(application.state(), &previous);
        assert!(store.load().await.unwrap().is_none());
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_7_r_nf_4__stored_refusals_survive_restarts_without_keeping_a_release_token() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
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
            Timestamp::from_seconds(u64::MAX),
            refusal,
        )
        .unwrap();
        store.save(&saved).await.unwrap();
        let restored = store.load().await.unwrap().unwrap();
        assert_eq!(restored.validated_at(), Timestamp::from_seconds(u64::MAX));
        assert_eq!(restored.refusal(), refusal);
        assert_eq!(restored.values().own_token(), None);
        let application = CredentialApplication::load(&mut store).await.unwrap();
        match refusal {
            Some(failure) => assert_eq!(application.state(), &AccountState::Rejected(failure)),
            None => assert!(matches!(
                application.state(),
                AccountState::Validated { .. }
            )),
        }
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__invalid_or_oversized_bundles_never_replace_a_previous_record() {
    let provider = Provider::default();
    *provider.0.bytes.lock().unwrap() = Some(BUNDLE.to_vec());
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    for username in [String::new(), "dummy\nrunner".to_owned(), "x".repeat(2560)] {
        let saved = SavedCredentials::from_storage(
            CredentialValues::new("dummy-key".to_owned(), None).unwrap(),
            ValidatedAccount::new(username),
            Timestamp::from_seconds(1),
            None,
        )
        .unwrap();
        assert_eq!(store.save(&saved).await, Err(StorageFailure::WriteFailed));
        assert_eq!(provider.0.bytes.lock().unwrap().as_deref(), Some(BUNDLE));
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__malformed_provider_data_is_never_treated_as_missing_or_valid_credentials() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let mut malformed = vec![Vec::new(), vec![0; 2561], b"TLCR\x02".to_vec()];
    for end in 0..BUNDLE.len() {
        malformed.push(BUNDLE[..end].to_vec());
    }
    let mut trailing = BUNDLE.to_vec();
    trailing.push(0);
    malformed.push(trailing);
    for (offset, byte) in [
        (0, b'X'),
        (5, 255),
        (26, 255),
        (27, 255),
        (28, b'\n'),
        (59, b'\n'),
    ] {
        let mut changed = BUNDLE.to_vec();
        changed[offset] = byte;
        malformed.push(changed);
    }
    for bytes in malformed {
        *provider.0.bytes.lock().unwrap() = Some(bytes);
        assert_eq!(store.load().await.err(), Some(StorageFailure::Corrupt));
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__maximum_native_bundle_is_accepted_without_truncating_fields() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let saved = SavedCredentials::from_storage(
        CredentialValues::new("k".repeat(1024), Some("t".repeat(1024))).unwrap(),
        ValidatedAccount::new("n".repeat(484)),
        Timestamp::from_millis(12345),
        None,
    )
    .unwrap();
    store.save(&saved).await.unwrap();
    assert_eq!(
        provider.0.bytes.lock().unwrap().as_ref().unwrap().len(),
        2560
    );
    let restored = store.load().await.unwrap().unwrap();
    assert_eq!(restored.values().secret_key(), "k".repeat(1024));
    assert_eq!(
        restored.values().own_token(),
        Some("t".repeat(1024).as_str())
    );
    assert_eq!(restored.username(), "n".repeat(484));
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__token_associations_survive_account_removal_and_token_return() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let first = store
        .token_scope("dummy-token-a", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    let second = store
        .token_scope("dummy-token-b", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(first, second);
    store.remove().await.unwrap();
    let mut reopened = WindowsCredentialStore::with_provider(&provider).unwrap();
    assert_eq!(
        reopened
            .token_scope("dummy-token-a", Persistence::Durable)
            .await
            .unwrap(),
        Some(first)
    );
    assert_eq!(
        reopened
            .token_scope("dummy-token-b", Persistence::Session)
            .await
            .unwrap(),
        Some(second)
    );
    assert_eq!(
        reopened
            .token_scope("dummy-session-only", Persistence::Session)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        reopened
            .token_scope("dummy-token-a", Persistence::Session)
            .await
            .unwrap(),
        Some(first)
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__account_key_replacement_keeps_exact_token_quota_identity() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let mut application = CredentialApplication::default();
    accept(&mut application, &mut store, Persistence::Durable)
        .await
        .unwrap();
    application
        .establish_token_scope(&mut store, None, Persistence::Durable)
        .await
        .unwrap();
    let original = application.token_scope(None).unwrap();
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-new-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        Persistence::Durable,
    ));
    assert_eq!(application.token_scope(None), None);
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-second-account".to_owned())),
            Timestamp::from_seconds(200),
            &mut store,
        )
        .await
        .unwrap();
    application
        .establish_token_scope(&mut store, None, Persistence::Durable)
        .await
        .unwrap();
    assert_eq!(application.token_scope(None), Some(original));
    assert_eq!(
        application.token_scope(Some("dummy-other-packaged-token")),
        Some(original)
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__registry_save_failure_publishes_no_new_scope_and_preserves_known_associations()
 {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let first = store
        .token_scope("dummy-a", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    *provider.1.write_fault.lock().unwrap() = Some(Fault::Unavailable);
    assert_eq!(
        store.token_scope("dummy-b", Persistence::Durable).await,
        Err(StorageFailure::WriteFailed)
    );
    let mut reopened = WindowsCredentialStore::with_provider(&provider).unwrap();
    assert_eq!(
        reopened
            .token_scope("dummy-a", Persistence::Session)
            .await
            .unwrap(),
        Some(first)
    );
    assert_eq!(
        reopened
            .token_scope("dummy-b", Persistence::Session)
            .await
            .unwrap(),
        None
    );
    *provider.1.write_fault.lock().unwrap() = None;
    let second = reopened
        .token_scope("dummy-b", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(first, second);
    assert!(first.shares_registry(second));
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__session_input_and_locked_registry_never_publish_a_guessed_durable_scope()
{
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    assert_eq!(
        store
            .token_scope("dummy-session", Persistence::Session)
            .await
            .unwrap(),
        None
    );
    let mut reopened = WindowsCredentialStore::with_provider(&provider).unwrap();
    assert_eq!(
        reopened
            .token_scope("dummy-session", Persistence::Session)
            .await
            .unwrap(),
        None
    );
    *provider.1.read_fault.lock().unwrap() = Some(Fault::Locked);
    assert_eq!(
        reopened
            .token_scope("dummy-session", Persistence::Session)
            .await,
        Err(StorageFailure::Locked)
    );
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__lost_registry_uses_a_distinct_generation_and_future_formats_fail_closed()
{
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let original = store
        .token_scope("dummy-same-token", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    *provider.1.bytes.lock().unwrap() = None;
    let replaced = store
        .token_scope("dummy-same-token", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(original, replaced);
    assert!(!original.shares_registry(replaced));
    *provider.1.bytes.lock().unwrap() = Some(b"TLTR\x02".to_vec());
    assert_eq!(
        store
            .token_scope("dummy-same-token", Persistence::Durable)
            .await,
        Err(StorageFailure::Corrupt)
    );
    assert_eq!(
        store
            .token_scope("dummy-same-token", Persistence::Session)
            .await,
        Err(StorageFailure::Corrupt)
    );
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__full_registry_refuses_growth_without_evicting_existing_token_identity() {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let first = "a".repeat(1024);
    let second = "b".repeat(1024);
    let third = "c".repeat(1024);
    let a = store
        .token_scope(&first, Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    let b = store
        .token_scope(&second, Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store.token_scope(&third, Persistence::Durable).await,
        Err(StorageFailure::WriteFailed)
    );
    let mut reopened = WindowsCredentialStore::with_provider(&provider).unwrap();
    assert_eq!(
        reopened
            .token_scope(&first, Persistence::Session)
            .await
            .unwrap(),
        Some(a)
    );
    assert_eq!(
        reopened
            .token_scope(&second, Persistence::Session)
            .await
            .unwrap(),
        Some(b)
    );
    assert_eq!(
        reopened
            .token_scope(&third, Persistence::Session)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__intact_registry_proves_first_token_replacement_without_saving_the_candidate()
 {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let first = store
        .token_scope("dummy-token-a", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    let same = store.token_admission("dummy-token-a").await.unwrap();
    assert_eq!(same.known_scope(), Some(first));
    assert!(!same.proves_distinct(first));
    let replacement = store.token_admission("dummy-first-token-b").await.unwrap();
    assert_eq!(replacement.known_scope(), None);
    assert!(replacement.proves_distinct(first));
    assert_eq!(
        store
            .token_scope("dummy-first-token-b", Persistence::Session)
            .await
            .unwrap(),
        None
    );
    *provider.1.bytes.lock().unwrap() = None;
    let lost = store.token_admission("dummy-first-token-b").await.unwrap();
    assert_eq!(lost.known_scope(), None);
    assert!(!lost.proves_distinct(first));
}

struct QuotaValidator {
    held: tallyline_submission::api::model::TokenQuotaObservation,
    accounts: std::sync::atomic::AtomicUsize,
    tokens: std::sync::atomic::AtomicUsize,
}
impl tallyline_submission::api::CredentialValidator for QuotaValidator {
    async fn validate(
        &self,
        values: &CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        self.accounts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.held.blocks_admission(values.token_admission()) {
            return Err(CredentialFailure::Unavailable);
        }
        self.tokens
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ValidatedAccount::new("dummy-runner".to_owned()))
    }
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11__first_distinct_token_validates_before_registration_while_same_and_lost_tokens_stay_held()
 {
    let provider = Provider::default();
    let mut store = WindowsCredentialStore::with_provider(&provider).unwrap();
    let a = store
        .token_scope("dummy-token-a", Persistence::Durable)
        .await
        .unwrap()
        .unwrap();
    let validator = QuotaValidator {
        held: tallyline_submission::api::model::TokenQuotaObservation::new(Some(a), None),
        accounts: Default::default(),
        tokens: Default::default(),
    };
    let mut application = CredentialApplication::default();
    for (token, allowed) in [("dummy-token-a", false), ("dummy-first-token-b", true)] {
        let mut request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some(token.to_owned())).unwrap(),
            Persistence::Durable,
        ));
        request
            .lookup_token_admission(&mut store, None)
            .await
            .unwrap();
        let result = request.validate(&validator).await;
        assert_eq!(result.is_ok(), allowed);
        if allowed {
            assert_eq!(
                store
                    .token_scope(token, Persistence::Session)
                    .await
                    .unwrap(),
                None
            );
            application
                .accept(request, result, Timestamp::from_seconds(1000), &mut store)
                .await
                .unwrap();
            application
                .establish_token_scope(&mut store, None, Persistence::Durable)
                .await
                .unwrap();
            let b = application.token_scope(None).unwrap();
            assert_ne!(a, b);
            assert!(
                !validator
                    .held
                    .blocks_admission(application.token_admission(None).unwrap())
            );
        }
    }
    *provider.1.bytes.lock().unwrap() = None;
    let mut lost = application.begin(CredentialCandidate::new(
        CredentialValues::new(
            "dummy-key".to_owned(),
            Some("dummy-first-token-b".to_owned()),
        )
        .unwrap(),
        Persistence::Durable,
    ));
    lost.lookup_token_admission(&mut store, None).await.unwrap();
    assert_eq!(
        lost.validate(&validator).await,
        Err(CredentialFailure::Unavailable)
    );
    assert_eq!(
        validator.accounts.load(std::sync::atomic::Ordering::SeqCst),
        3
    );
    assert_eq!(
        validator.tokens.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}
