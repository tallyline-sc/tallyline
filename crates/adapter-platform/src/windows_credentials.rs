// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned Windows local-machine credential storage.

use std::collections::HashMap;

use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error};
use tallyline_submission::api::SecretStore;
use tallyline_submission::api::model::{
    Persistence, SavedCredentials, StorageFailure, TokenAdmission, TokenScope,
};
use zeroize::{Zeroize, Zeroizing};

use crate::credential_bundle;

/// Stores one bounded credential bundle in Windows Credential Manager.
/// Calls run serially on an application-owned blocking worker.
pub struct WindowsCredentialStore {
    entry: Entry,
    registry: Entry,
}

impl WindowsCredentialStore {
    /// Builds an owned native store without reading or writing credentials.
    ///
    /// # Errors
    /// Returns a closed failure if the native store cannot be constructed.
    pub fn new() -> Result<Self, StorageFailure> {
        let provider = windows_native_keyring_store::Store::new()
            .map_err(|error| failure(error, StorageFailure::Unavailable))?;
        Self::with_provider(provider.as_ref())
    }

    /// Builds the fixed local-machine entry with an injected credential provider.
    ///
    /// # Errors
    /// Returns a closed failure without formatting provider errors.
    pub fn with_provider(provider: &impl CredentialStoreApi) -> Result<Self, StorageFailure> {
        let modifiers = HashMap::from([("persistence", "Local")]);
        let entry = provider
            .build("Tallyline", "uex-credentials", Some(&modifiers))
            .map_err(|error| failure(error, StorageFailure::Unavailable))?;
        let registry = provider
            .build("Tallyline", "uex-token-associations", Some(&modifiers))
            .map_err(|error| failure(error, StorageFailure::Unavailable))?;
        Ok(Self { entry, registry })
    }
    fn read_registry(&self) -> Result<crate::token_registry::TokenRegistry, StorageFailure> {
        let bytes = match self.registry.get_secret() {
            Ok(bytes) => Some(Zeroizing::new(bytes)),
            Err(Error::NoEntry) => None,
            Err(error) => return Err(failure(error, StorageFailure::Unavailable)),
        };
        crate::token_registry::TokenRegistry::load(bytes.as_deref().map(Vec::as_slice))
    }
}

impl SecretStore for WindowsCredentialStore {
    async fn token_admission(&mut self, token: &str) -> Result<TokenAdmission, StorageFailure> {
        self.read_registry()?.admission(token)
    }

    async fn token_scope(
        &mut self,
        token: &str,
        persistence: Persistence,
    ) -> Result<Option<TokenScope>, StorageFailure> {
        let mut registry = self.read_registry()?;
        let (scope, changed) = registry.associate(token, persistence)?;
        if changed {
            self.registry
                .set_secret(&registry.encode()?)
                .map_err(|error| failure(error, StorageFailure::WriteFailed))?;
        }
        Ok(scope)
    }

    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        match self.entry.get_secret() {
            Ok(bytes) => credential_bundle::decode(&Zeroizing::new(bytes)).map(Some),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(failure(error, StorageFailure::Unavailable)),
        }
    }

    async fn save(&mut self, credentials: &SavedCredentials) -> Result<(), StorageFailure> {
        let bytes = credential_bundle::encode(credentials)?;
        self.entry
            .set_secret(&bytes)
            .map_err(|error| failure(error, StorageFailure::WriteFailed))
    }

    async fn save_passive(&mut self, saved: &SavedCredentials) -> Result<(), StorageFailure> {
        self.save(saved).await
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        match self.entry.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(error) => Err(failure(error, StorageFailure::Unavailable)),
        }
    }
}

fn failure(error: Error, fallback: StorageFailure) -> StorageFailure {
    match error {
        Error::NoStorageAccess(_) => StorageFailure::Locked,
        Error::BadEncoding(mut bytes) | Error::BadDataFormat(mut bytes, _) => {
            bytes.zeroize();
            StorageFailure::Corrupt
        }
        Error::Ambiguous(_) | Error::BadStoreFormat(_) => StorageFailure::Corrupt,
        Error::NoDefaultStore | Error::NotSupportedByStore(_) => StorageFailure::Unavailable,
        Error::PlatformFailure(_)
        | Error::NoEntry
        | Error::TooLong(..)
        | Error::Invalid(..)
        | _ => fallback,
    }
}
