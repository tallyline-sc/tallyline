// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform credential composition on the bounded credential worker.

use std::sync::mpsc::{Receiver, SyncSender};
#[cfg(target_os = "linux")]
use tallyline_submission::api::model::StorageFailure;
use tallyline_ui::api::model::{BackgroundEvent, CredentialCommand};
pub(crate) fn start(
    credential_directory: std::path::PathBuf,
    incoming: Receiver<CredentialCommand>,
    outgoing: SyncSender<BackgroundEvent>,
    context: egui::Context,
    client: tallyline_app::api::HttpClient,
    queues: Receiver<Box<dyn tallyline_app::api::CredentialQueue>>,
) -> std::io::Result<tallyline_app::api::CredentialWorker> {
    tallyline_app::api::CredentialWorker::start_with_queue(
        move || initialise(credential_directory),
        client,
        crate::update_wiring::SystemClock,
        None,
        (incoming, outgoing),
        queues,
        move || tallyline_ui::api::wake_up(&context),
    )
}
fn release_token() -> Option<tallyline_submission::api::model::ReleaseToken> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;
    tallyline_adapter_platform::api::ReleaseTokenFile::new(directory.to_owned())
        .read()
        .ok()
        .flatten()
}
#[cfg(windows)]
async fn initialise(
    _: std::path::PathBuf,
) -> tallyline_app::api::CredentialStores<tallyline_adapter_platform::api::WindowsCredentialStore> {
    tallyline_app::api::CredentialStores::new(
        tallyline_adapter_platform::api::WindowsCredentialStore::new(),
        None,
    )
    .with_release_token(release_token())
}
#[cfg(target_os = "linux")]
async fn initialise(
    directory: std::path::PathBuf,
) -> tallyline_app::api::CredentialStores<LinuxStore> {
    use std::os::unix::fs::DirBuilderExt;
    let primary = async {
        let address =
            std::env::var("DBUS_SESSION_BUS_ADDRESS").map_err(|_| StorageFailure::Unavailable)?;
        let bus = tallyline_adapter_platform::api::LinuxCredentialBus::connect(&address).await?;
        tallyline_adapter_platform::api::LinuxCredentialStore::connect(bus)
            .await
            .map(LinuxStore::Service)
    }
    .await;
    let created = std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory);
    let fallback = if created.is_ok()
        || created
            .as_ref()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists)
    {
        tallyline_adapter_platform::api::LinuxFallbackStore::open(&directory)
            .ok()
            .map(LinuxStore::File)
    } else {
        None
    };
    tallyline_app::api::CredentialStores::new(primary, fallback).with_release_token(release_token())
}
#[cfg(target_os = "linux")]
enum LinuxStore {
    Service(tallyline_adapter_platform::api::LinuxCredentialStore),
    File(tallyline_adapter_platform::api::LinuxFallbackStore),
}
#[cfg(target_os = "linux")]
impl tallyline_submission::api::SecretStore for LinuxStore {
    async fn token_admission(
        &mut self,
        token: &str,
    ) -> Result<tallyline_submission::api::model::TokenAdmission, StorageFailure> {
        match self {
            Self::Service(store) => store.token_admission(token).await,
            Self::File(store) => store.token_admission(token).await,
        }
    }

    async fn token_scope(
        &mut self,
        token: &str,
        persistence: tallyline_submission::api::model::Persistence,
    ) -> Result<Option<tallyline_submission::api::model::TokenScope>, StorageFailure> {
        match self {
            Self::Service(store) => store.token_scope(token, persistence).await,
            Self::File(store) => store.token_scope(token, persistence).await,
        }
    }

    async fn load(
        &mut self,
    ) -> Result<Option<tallyline_submission::api::model::SavedCredentials>, StorageFailure> {
        match self {
            Self::Service(store) => store.load().await,
            Self::File(store) => store.load().await,
        }
    }
    async fn save(
        &mut self,
        credentials: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), StorageFailure> {
        match self {
            Self::Service(store) => store.save(credentials).await,
            Self::File(store) => store.save(credentials).await,
        }
    }
    async fn save_passive(
        &mut self,
        credentials: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), StorageFailure> {
        match self {
            Self::Service(store) => store.save_passive(credentials).await,
            Self::File(store) => store.save_passive(credentials).await,
        }
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        match self {
            Self::Service(store) => store.remove().await,
            Self::File(store) => store.remove().await,
        }
    }
}
