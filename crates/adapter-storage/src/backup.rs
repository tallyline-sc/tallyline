// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Consistent SQLite snapshots and public restore-marker inspection.

use crate::api::StorageError;
use crate::api::model::BackupMetadata;
use crate::reference_cache::{SqliteReferenceCache, timestamp};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::fs::OpenOptions;
use std::path::Path;
use tallyline_shared_kernel::api::model::Timestamp;

impl SqliteReferenceCache {
    /// Writes a new consistent backup, including committed WAL content and its marker.
    ///
    /// The application owns the single-writer lease and a pending backup-set directory.
    /// It supplies the same sampled time and configuration schema to the set writer.
    ///
    /// # Errors
    /// Returns an existing destination, filesystem failure, SQLite failure or invalid snapshot.
    pub fn backup_into(
        &mut self,
        destination: &Path,
        taken_at: Timestamp,
        configuration_schema: Option<u32>,
    ) -> Result<u32, StorageError> {
        let filename = destination.to_str().ok_or_else(|| {
            StorageError::Filesystem(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "backup path is not valid Unicode",
            ))
        })?;
        let reserved = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(StorageError::Filesystem)?;
        drop(reserved);
        let from_schema = self.connection.pragma_query_one(None, "user_version")?;
        self.connection.pragma_update(None, "synchronous", "FULL")?;
        self.connection.execute("VACUUM INTO ?1", [filename])?;
        let mut backup = Connection::open_with_flags(
            destination,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        backup.pragma_update(None, "journal_mode", "DELETE")?;
        backup.pragma_update(None, "synchronous", "FULL")?;
        verify(&backup)?;
        let transaction = backup.transaction()?;
        transaction.execute_batch("CREATE TABLE IF NOT EXISTS backup_meta(taken_at TEXT NOT NULL, from_version INTEGER NOT NULL, config_schema INTEGER); DELETE FROM backup_meta;")?;
        transaction.execute(
            "INSERT INTO backup_meta(taken_at,from_version,config_schema) VALUES(?1,?2,?3)",
            (
                taken_at.millis().to_string(),
                from_schema,
                configuration_schema,
            ),
        )?;
        transaction.commit()?;
        verify(&backup)?;
        drop(backup);
        OpenOptions::new()
            .write(true)
            .open(destination)
            .map_err(StorageError::Filesystem)?
            .sync_all()
            .map_err(StorageError::Filesystem)?;
        Ok(from_schema)
    }

    /// Reads a restore marker without clearing it or inventing network observations.
    ///
    /// # Errors
    /// Returns a malformed marker or database read failure.
    pub fn backup_metadata(&self) -> Result<Option<BackupMetadata>, StorageError> {
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='backup_meta')",
            [],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(None);
        }
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM backup_meta", [], |row| row.get(0))?;
        if count > 1 {
            return Err(StorageError::InvalidBackup);
        }
        let record: Option<(String, u32, Option<u32>)> = self
            .connection
            .query_row(
                "SELECT taken_at,from_version,config_schema FROM backup_meta",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        record
            .map(|(taken_at, from_schema, configuration_schema)| {
                Ok(BackupMetadata {
                    taken_at: timestamp(&taken_at).map_err(|_| StorageError::InvalidBackup)?,
                    from_schema,
                    configuration_schema,
                })
            })
            .transpose()
    }
}

fn verify(connection: &Connection) -> Result<(), StorageError> {
    let integrity: String = connection.pragma_query_one(None, "integrity_check")?;
    if integrity == "ok" {
        Ok(())
    } else {
        Err(StorageError::InvalidBackup)
    }
}
