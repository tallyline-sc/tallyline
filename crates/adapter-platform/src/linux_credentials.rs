// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Encrypted records on an authenticated Linux credential bus.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use secret_service::{Collection, EncryptionType, Error, Item, SecretService};
use tallyline_submission::api::SecretStore;
use tallyline_submission::api::model::{Persistence, SavedCredentials, StorageFailure, TokenScope};
use tracing::instrument::WithSubscriber;
use zbus::zvariant::OwnedObjectPath;
use zeroize::Zeroizing;

use crate::credential_bundle;
use crate::linux_credential_bus::LinuxCredentialBus;

const PHASE_TIMEOUT: Duration = Duration::from_secs(5);
const CONTENT_TYPE: &str = "application/octet-stream";
const SERVICE: &str = "org.freedesktop.secrets";

/// Owns a DH-encrypted session and one fixed application record.
/// Calls run serially on the caller's runtime; credential dependency log targets must be suppressed.
pub struct LinuxCredentialStore {
    service: SecretService<'static>,
    connection: zbus::Connection,
}

impl LinuxCredentialStore {
    /// Consumes an authenticated bus and negotiates DH without unlocking a collection.
    ///
    /// # Errors
    /// Returns a closed failure when negotiation is refused or exceeds five seconds.
    pub async fn connect(bus: LinuxCredentialBus) -> Result<Self, StorageFailure> {
        bounded(
            async {
                let connection = bus.connection;
                let service =
                    SecretService::connect_with_existing(EncryptionType::Dh, connection.clone())
                        .await
                        .map_err(|error| failure(error, StorageFailure::Unavailable))?;
                Ok(Self {
                    service,
                    connection,
                })
            },
            StorageFailure::Unavailable,
        )
        .await
    }

    async fn collection(&self, fallback: StorageFailure) -> Result<Collection<'_>, StorageFailure> {
        let collection = self
            .service
            .get_default_collection()
            .await
            .map_err(|error| failure(error, fallback))?;
        if collection
            .is_locked()
            .await
            .map_err(|error| failure(error, fallback))?
        {
            return Err(StorageFailure::Locked);
        }
        Ok(collection)
    }
}

impl SecretStore for LinuxCredentialStore {
    async fn token_admission(
        &mut self,
        token: &str,
    ) -> Result<tallyline_submission::api::model::TokenAdmission, StorageFailure> {
        let collection = bounded(
            self.collection(StorageFailure::Unavailable),
            StorageFailure::Unavailable,
        )
        .await?;
        read_registry(&collection).await?.admission(token)
    }

    async fn token_scope(
        &mut self,
        token: &str,
        persistence: Persistence,
    ) -> Result<Option<TokenScope>, StorageFailure> {
        const RECORD: &str = "uex-token-associations";
        let collection = bounded(
            self.collection(StorageFailure::Unavailable),
            StorageFailure::Unavailable,
        )
        .await?;
        let mut registry = read_registry(&collection).await?;
        let (scope, changed) = registry.associate(token, persistence)?;
        if changed {
            let bytes = registry.encode()?;
            bounded(
                async {
                    collection
                        .create_item(
                            "Tallyline UEX token associations",
                            HashMap::from([("application", "Tallyline"), ("record", RECORD)]),
                            &bytes,
                            true,
                            CONTENT_TYPE,
                        )
                        .await
                        .map_err(mutation_failure)?;
                    Ok(())
                },
                StorageFailure::OutcomeUnknown,
            )
            .await?;
        }
        Ok(scope)
    }

    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        bounded(
            async {
                let collection = self.collection(StorageFailure::Unavailable).await?;
                let Some(item) = record(&collection, StorageFailure::Unavailable).await? else {
                    return Ok(None);
                };
                let bytes = Zeroizing::new(
                    item.get_secret()
                        .await
                        .map_err(|error| failure(error, StorageFailure::Unavailable))?,
                );
                credential_bundle::decode(&bytes).map(Some)
            },
            StorageFailure::Unavailable,
        )
        .await
    }

    async fn save(&mut self, credentials: &SavedCredentials) -> Result<(), StorageFailure> {
        let bytes = credential_bundle::encode(credentials)?;
        let collection = bounded(
            async {
                let collection = self.collection(StorageFailure::WriteFailed).await?;
                drop(record(&collection, StorageFailure::WriteFailed).await?);
                Ok(collection)
            },
            StorageFailure::WriteFailed,
        )
        .await?;
        bounded(
            async {
                collection
                    .create_item(
                        "Tallyline UEX credentials",
                        attributes(),
                        &bytes,
                        true,
                        CONTENT_TYPE,
                    )
                    .await
                    .map_err(mutation_failure)?;
                Ok(())
            },
            StorageFailure::OutcomeUnknown,
        )
        .await
    }

    async fn save_passive(&mut self, credentials: &SavedCredentials) -> Result<(), StorageFailure> {
        let bytes = credential_bundle::encode(credentials)?;
        let item = bounded(
            async {
                let collection = self.collection(StorageFailure::Unavailable).await?;
                let path = record(&collection, StorageFailure::Unavailable)
                    .await?
                    .map(|item| item.item_path)
                    .ok_or(StorageFailure::Corrupt)?;
                self.service
                    .get_item_by_path(path)
                    .await
                    .map_err(|error| failure(error, StorageFailure::Unavailable))
            },
            StorageFailure::Unavailable,
        )
        .await?;
        bounded(
            async {
                item.set_secret(&bytes, CONTENT_TYPE)
                    .await
                    .map_err(mutation_failure)
            },
            StorageFailure::OutcomeUnknown,
        )
        .await
    }

    async fn remove(&mut self) -> Result<(), StorageFailure> {
        let path = bounded(
            async {
                let collection = self.collection(StorageFailure::Unavailable).await?;
                Ok(record(&collection, StorageFailure::Unavailable)
                    .await?
                    .map(|item| item.item_path))
            },
            StorageFailure::Unavailable,
        )
        .await?;
        let Some(path) = path else {
            return Ok(());
        };
        bounded(
            async {
                let reply = self
                    .connection
                    .call_method(
                        Some(SERVICE),
                        path,
                        Some("org.freedesktop.Secret.Item"),
                        "Delete",
                        &(),
                    )
                    .await
                    .map_err(dispatched_failure)?;
                let prompt = reply
                    .body()
                    .deserialize::<OwnedObjectPath>()
                    .map_err(|_| StorageFailure::OutcomeUnknown)?;
                if prompt.as_str() != "/" {
                    return Err(StorageFailure::Locked);
                }
                Ok(())
            },
            StorageFailure::OutcomeUnknown,
        )
        .await
    }
}

fn attributes() -> HashMap<&'static str, &'static str> {
    HashMap::from([("application", "Tallyline"), ("record", "uex-credentials")])
}

async fn record<'a>(
    collection: &'a Collection<'_>,
    fallback: StorageFailure,
) -> Result<Option<Item<'a>>, StorageFailure> {
    record_named(collection, fallback, "uex-credentials").await
}
async fn record_named<'a>(
    collection: &'a Collection<'_>,
    fallback: StorageFailure,
    name: &'static str,
) -> Result<Option<Item<'a>>, StorageFailure> {
    let mut items = collection
        .search_items(HashMap::from([
            ("application", "Tallyline"),
            ("record", name),
        ]))
        .await
        .map_err(|error| failure(error, fallback))?;
    if items.len() > 1 {
        return Err(StorageFailure::Corrupt);
    }
    let Some(item) = items.pop() else {
        return Ok(None);
    };
    if item
        .is_locked()
        .await
        .map_err(|error| failure(error, fallback))?
    {
        return Err(StorageFailure::Locked);
    }
    Ok(Some(item))
}

async fn bounded<T>(
    operation: impl Future<Output = Result<T, StorageFailure>>,
    timeout: StorageFailure,
) -> Result<T, StorageFailure> {
    tokio::time::timeout(PHASE_TIMEOUT, operation)
        .with_subscriber(tracing::subscriber::NoSubscriber::default())
        .await
        .map_err(|_| timeout)?
}

fn mutation_failure(error: Error) -> StorageFailure {
    match error {
        Error::Locked | Error::Prompt => StorageFailure::Locked,
        Error::Zbus(error) => dispatched_failure(error),
        Error::Crypto(_)
        | Error::Zvariant(_)
        | Error::ZbusFdo(_)
        | Error::NoResult
        | Error::PromptDisconnected
        | Error::Unavailable
        | _ => StorageFailure::OutcomeUnknown,
    }
}

fn dispatched_failure(error: zbus::Error) -> StorageFailure {
    if let zbus::Error::MethodError(name, _, _) = error {
        if locked_error(name.as_str()) {
            StorageFailure::Locked
        } else {
            StorageFailure::WriteFailed
        }
    } else {
        StorageFailure::OutcomeUnknown
    }
}

fn locked_error(name: &str) -> bool {
    matches!(
        name,
        "org.freedesktop.Secret.Error.IsLocked" | "org.freedesktop.DBus.Error.AccessDenied"
    )
}

fn failure(error: Error, fallback: StorageFailure) -> StorageFailure {
    match error {
        Error::Locked | Error::Prompt => StorageFailure::Locked,
        Error::Crypto(_) | Error::Zvariant(_) => StorageFailure::Corrupt,
        Error::Zbus(zbus::Error::MethodError(name, _, _)) if locked_error(name.as_str()) => {
            StorageFailure::Locked
        }
        Error::Zbus(_)
        | Error::ZbusFdo(_)
        | Error::NoResult
        | Error::PromptDisconnected
        | Error::Unavailable
        | _ => fallback,
    }
}

async fn read_registry(
    collection: &Collection<'_>,
) -> Result<crate::token_registry::TokenRegistry, StorageFailure> {
    let bytes = bounded(
        async {
            match record_named(
                collection,
                StorageFailure::Unavailable,
                "uex-token-associations",
            )
            .await?
            {
                Some(item) => item
                    .get_secret()
                    .await
                    .map(Zeroizing::new)
                    .map(Some)
                    .map_err(|error| failure(error, StorageFailure::Unavailable)),
                None => Ok(None),
            }
        },
        StorageFailure::Unavailable,
    )
    .await?;
    crate::token_registry::TokenRegistry::load(bytes.as_deref().map(Vec::as_slice))
}
