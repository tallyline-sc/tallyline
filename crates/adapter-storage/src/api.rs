// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! SQLite repository operations and failures.

pub mod model;

pub use crate::reference_cache::SqliteReferenceCache;

/// A reference database failure.
#[derive(Debug)]
pub enum StorageError {
    /// SQLite could not complete the operation.
    Database(rusqlite::Error),
    /// Persisted cache data or its schema is invalid.
    InvalidCache,
    /// A backup did not pass integrity or restore-marker validation.
    InvalidBackup,
    /// The database file could not be inspected.
    Filesystem(std::io::Error),
    /// An existing schema cannot migrate without a coordinated backup.
    MigrationBackupRequired,
    /// A newer application schema cannot be opened for writing.
    UnsupportedSchema {
        /// The persisted schema version.
        found: u32,
        /// The newest supported schema version.
        supported: u32,
    },
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("reference database operation failed"),
            Self::InvalidBackup => formatter.write_str("database backup verification failed"),
            Self::MigrationBackupRequired => {
                formatter.write_str("database migration requires a coordinated backup")
            }
            Self::InvalidCache => formatter.write_str("reference cache data or schema is invalid"),
            Self::Filesystem(_) => formatter.write_str("reference database file is inaccessible"),
            Self::UnsupportedSchema { found, supported } => write!(
                formatter,
                "database schema {found} is newer than supported schema {supported}"
            ),
        }
    }
}
impl std::error::Error for StorageError {}
impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}
