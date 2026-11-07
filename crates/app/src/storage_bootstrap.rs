// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Coordinated configuration and SQLite migration backups.

use crate::api::model::PreparedStorage;
use std::io;
use tallyline_adapter_files::api::{BackupSetStore, ConfigFile, StorageLayout};
use tallyline_adapter_storage::api::{SqliteReferenceCache, StorageError};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;

struct BackupClock(Timestamp);
impl Clock for BackupClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

/// Prepares local storage with a joint backup before any existing config migration.
///
/// The caller holds the writer lease and has validated the layout directories.
/// This blocking operation runs outside the UI and async runtime poll functions.
///
/// # Errors
/// Returns unsupported schemas, inaccessible storage or an incomplete coordinated backup.
pub fn initialise_storage(
    layout: &StorageLayout,
    clock: &impl Clock,
) -> io::Result<PreparedStorage> {
    let path = layout.config().join("settings.json");
    let source = ConfigFile::new(path.clone()).backup_source()?;
    let mut backed_up = None;
    let database = layout.data().join("tallyline.db");
    let backup_layout = layout.clone();
    let backup_clock = BackupClock(clock.now());
    let cache = if database.try_exists()? {
        Some(
            SqliteReferenceCache::open_with_migration_backup(&database, |cache, _| {
                let config = source
                    .as_ref()
                    .map(|source| (source.schema, source.bytes.as_slice()));
                BackupSetStore::new(layout.data().join("backup"))
                    .write(config, &backup_clock, |destination, taken_at| {
                        cache
                            .backup_into(
                                destination,
                                taken_at,
                                source.as_ref().map(|source| source.schema),
                            )
                            .map(Some)
                            .map_err(io::Error::other)
                    })
                    .map_err(StorageError::Filesystem)?;
                backed_up = source.clone();
                Ok(())
            })
            .map_err(io::Error::other)?,
        )
    } else {
        None
    };
    let cache = std::sync::Arc::new(std::sync::Mutex::new(cache));
    let backup_cache = std::sync::Arc::clone(&cache);
    let mut settings = ConfigFile::new(path.clone()).with_migration_backup(move |schema, bytes| {
        if let Some(source) = &backed_up {
            return if source.schema == schema && source.bytes == bytes {
                Ok(())
            } else {
                Err(io::Error::other(
                    "configuration changed after coordinated backup",
                ))
            };
        }
        let mut cache = backup_cache
            .lock()
            .map_err(|_| io::Error::other("storage coordinator lock failed"))?;
        let backups = BackupSetStore::new(backup_layout.data().join("backup"));
        match cache.as_mut() {
            Some(cache) => backups.write(
                Some((schema, bytes)),
                &backup_clock,
                |destination, taken_at| {
                    cache
                        .backup_into(destination, taken_at, Some(schema))
                        .map(Some)
                        .map_err(io::Error::other)
                },
            ),
            None => backups.backup_without_database(&backup_layout, schema, bytes, &backup_clock),
        }
        .map(|_| ())
    });
    settings.migrate()?;
    drop(settings);
    let cache = std::sync::Arc::try_unwrap(cache)
        .map_err(|_| io::Error::other("storage coordinator is still shared"))?
        .into_inner()
        .map_err(|_| io::Error::other("storage coordinator lock failed"))?;
    let cache = match cache {
        Some(cache) => cache,
        None => SqliteReferenceCache::open(&database).map_err(io::Error::other)?,
    };
    Ok(PreparedStorage {
        settings: ConfigFile::new(path),
        cache,
    })
}
