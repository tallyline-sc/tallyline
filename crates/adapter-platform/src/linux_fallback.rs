// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Descriptor-anchored owner-only Linux credential fallback.

use rustix::fs::{AtFlags, Mode, OFlags};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use tallyline_submission::api::SecretStore;
use tallyline_submission::api::model::{Persistence, SavedCredentials, StorageFailure, TokenScope};
use zeroize::Zeroizing;

use crate::credential_bundle;
const RECORD: &str = "uex-credentials";
const PRIVATE_MODE: u32 = 0o600;
const MAX_TEMP_ATTEMPTS: usize = 32;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Retains a checked private directory for explicitly warned Linux fallback use.
/// The caller offers this store only after Secret Service is unavailable and the user confirms.
pub struct LinuxFallbackStore {
    directory: File,
}
impl LinuxFallbackStore {
    fn read_registry(&self) -> Result<crate::token_registry::TokenRegistry, StorageFailure> {
        let bytes = match self.record_named("uex-token-associations")? {
            Some(file) => {
                let mut bytes = Zeroizing::new(Vec::new());
                file.take((crate::token_registry::MAX_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|_| StorageFailure::Corrupt)?;
                Some(bytes)
            }
            None => None,
        };
        crate::token_registry::TokenRegistry::load(bytes.as_deref().map(Vec::as_slice))
    }

    /// Checks a pre-existing owner-only directory and its actual filesystem.
    /// No stored credential is read or permission repaired.
    ///
    /// # Errors
    /// Returns unavailable for unsupported filesystems, paths, ownership or permissions.
    pub fn open(path: &Path) -> Result<Self, StorageFailure> {
        if !path.is_absolute() {
            return Err(StorageFailure::Unavailable);
        }
        let mut directory = File::open("/").map_err(|_| StorageFailure::Unavailable)?;
        for component in path.components() {
            match component {
                Component::RootDir => (),
                Component::Normal(name) => {
                    directory = File::from(
                        rustix::fs::openat(
                            &directory,
                            name,
                            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                            Mode::empty(),
                        )
                        .map_err(|_| StorageFailure::Unavailable)?,
                    );
                }
                Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                    return Err(StorageFailure::Unavailable);
                }
            }
        }
        let metadata = directory
            .metadata()
            .map_err(|_| StorageFailure::Unavailable)?;
        if !metadata.is_dir()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o077 != 0
            || metadata.mode() & 0o700 != 0o700
        {
            return Err(StorageFailure::Unavailable);
        }
        let filesystem =
            rustix::fs::fstatfs(&directory).map_err(|_| StorageFailure::Unavailable)?;
        if !matches!(
            filesystem.f_type,
            0xef53 | 0x5846_5342 | 0x9123_683e | 0x0102_1994 | 0x794c_7630
        ) {
            return Err(StorageFailure::Unavailable);
        }
        let store = Self { directory };
        let (name, probe) = store.temporary()?;
        drop(probe);
        rustix::fs::unlinkat(&store.directory, name.as_str(), AtFlags::empty())
            .map_err(|_| StorageFailure::Unavailable)?;
        Ok(store)
    }
    fn record(&self) -> Result<Option<File>, StorageFailure> {
        self.record_named(RECORD)
    }
    fn record_named(&self, record: &str) -> Result<Option<File>, StorageFailure> {
        let metadata = self
            .directory
            .metadata()
            .map_err(|_| StorageFailure::Unavailable)?;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(StorageFailure::Unavailable);
        }
        let descriptor = match rustix::fs::openat(
            &self.directory,
            record,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(_) => return Err(StorageFailure::Corrupt),
        };
        let file = File::from(descriptor);
        check_file(&file)?;
        Ok(Some(file))
    }
    fn replace_record(&self, record: &str, bytes: &[u8]) -> Result<(), StorageFailure> {
        drop(self.record_named(record)?);
        let (name, mut file) = self.temporary()?;
        let prepared = file.write_all(bytes).and_then(|()| file.sync_all());
        if prepared.is_err() {
            rustix::fs::unlinkat(&self.directory, name.as_str(), AtFlags::empty())
                .map_err(|_| StorageFailure::WriteFailed)?;
            return Err(StorageFailure::WriteFailed);
        }
        if rustix::fs::renameat(&self.directory, name.as_str(), &self.directory, record).is_err() {
            rustix::fs::unlinkat(&self.directory, name.as_str(), AtFlags::empty())
                .map_err(|_| StorageFailure::WriteFailed)?;
            return Err(StorageFailure::WriteFailed);
        }
        self.directory
            .sync_all()
            .map_err(|_| StorageFailure::OutcomeUnknown)
    }

    fn temporary(&self) -> Result<(String, File), StorageFailure> {
        for _ in 0..MAX_TEMP_ATTEMPTS {
            let name = format!(
                ".uex-credentials-{}-{}",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            );
            match rustix::fs::openat(
                &self.directory,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(PRIVATE_MODE),
            ) {
                Ok(descriptor) => {
                    let file = File::from(descriptor);
                    if let Err(failure) = check_file(&file) {
                        rustix::fs::unlinkat(&self.directory, name.as_str(), AtFlags::empty())
                            .map_err(|_| StorageFailure::WriteFailed)?;
                        return Err(failure);
                    }
                    return Ok((name, file));
                }
                Err(rustix::io::Errno::EXIST) => continue,
                Err(_) => return Err(StorageFailure::WriteFailed),
            }
        }
        Err(StorageFailure::WriteFailed)
    }
}
fn check_file(file: &File) -> Result<(), StorageFailure> {
    let metadata = file.metadata().map_err(|_| StorageFailure::Corrupt)?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != PRIVATE_MODE
        || metadata.nlink() != 1
    {
        return Err(StorageFailure::Corrupt);
    }
    Ok(())
}
impl SecretStore for LinuxFallbackStore {
    async fn token_admission(
        &mut self,
        token: &str,
    ) -> Result<tallyline_submission::api::model::TokenAdmission, StorageFailure> {
        self.read_registry()?.admission(token)
    }

    async fn token_scope(
        &mut self,
        token: &str,
        persistence: Persistence,
    ) -> Result<Option<TokenScope>, StorageFailure> {
        const REGISTRY: &str = "uex-token-associations";
        let mut registry = self.read_registry()?;
        let (scope, changed) = registry.associate(token, persistence)?;
        if changed {
            self.replace_record(REGISTRY, &registry.encode()?)?;
        }
        Ok(scope)
    }

    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        let Some(file) = self.record()? else {
            return Ok(None);
        };
        let mut bytes = Zeroizing::new(Vec::new());
        file.take(2561)
            .read_to_end(&mut bytes)
            .map_err(|_| StorageFailure::Corrupt)?;
        credential_bundle::decode(&bytes).map(Some)
    }
    async fn save(&mut self, credentials: &SavedCredentials) -> Result<(), StorageFailure> {
        let bytes = credential_bundle::encode(credentials)?;
        self.replace_record(RECORD, &bytes)
    }
    async fn save_passive(&mut self, saved: &SavedCredentials) -> Result<(), StorageFailure> {
        self.save(saved).await
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        if self.record()?.is_none() {
            return Ok(());
        }
        rustix::fs::unlinkat(&self.directory, RECORD, AtFlags::empty())
            .map_err(|_| StorageFailure::WriteFailed)?;
        self.directory
            .sync_all()
            .map_err(|_| StorageFailure::OutcomeUnknown)
    }
}
