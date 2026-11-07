// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Account-digest-scoped screenshot requirement memory.

use crate::api::SqliteReferenceCache;
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::ScreenshotPolicyStore;
use tallyline_submission::api::model::StorageFailure;

impl ScreenshotPolicyStore for SqliteReferenceCache {
    fn required_since(&mut self, account: &str) -> Result<Option<Timestamp>, StorageFailure> {
        required_since(&self.connection, account)
    }
    fn remember(&mut self, account: &str, at: Timestamp) -> Result<(), StorageFailure> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| StorageFailure::WriteFailed)?;
        remember_requirement(&transaction, account, at)?;
        transaction
            .commit()
            .map_err(|_| StorageFailure::WriteFailed)
    }
    fn clear(&mut self, account: &str) -> Result<(), StorageFailure> {
        let identity = Sha256::digest(account.as_bytes());
        self.connection
            .execute(
                "DELETE FROM screenshot_requirements WHERE account_digest=?1",
                [identity.as_slice()],
            )
            .map_err(|_| StorageFailure::WriteFailed)?;
        Ok(())
    }
}

pub(crate) fn remember_requirement(
    connection: &rusqlite::Connection,
    account: &str,
    at: Timestamp,
) -> Result<(), StorageFailure> {
    let identity = Sha256::digest(account.as_bytes());
    let previous: Option<String> = connection
        .query_row(
            "SELECT observed_at FROM screenshot_requirements WHERE account_digest=?1",
            [identity.as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| StorageFailure::WriteFailed)?;
    let previous = previous
        .map(|value| crate::reference_cache::timestamp(&value).map_err(|_| StorageFailure::Corrupt))
        .transpose()?;
    let newest = previous.map_or(at, |previous| previous.max(at));
    connection.execute("INSERT INTO screenshot_requirements(account_digest,observed_at) VALUES(?1,?2) ON CONFLICT(account_digest) DO UPDATE SET observed_at=excluded.observed_at", (identity.as_slice(), newest.millis().to_string())).map_err(|_| StorageFailure::WriteFailed)?;
    Ok(())
}

pub(crate) fn required_since(
    connection: &rusqlite::Connection,
    account: &str,
) -> Result<Option<Timestamp>, StorageFailure> {
    let identity = Sha256::digest(account.as_bytes());
    let at: Option<String> = connection
        .query_row(
            "SELECT observed_at FROM screenshot_requirements WHERE account_digest=?1",
            [identity.as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| StorageFailure::Unavailable)?;
    at.map(|at| crate::reference_cache::timestamp(&at).map_err(|_| StorageFailure::Corrupt))
        .transpose()
}
