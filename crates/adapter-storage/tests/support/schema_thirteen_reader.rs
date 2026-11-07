// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Source-bound schema-thirteen reader admission fixture.

use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use tallyline_adapter_storage::api::StorageError;

const SCHEMA_VERSION: u32 = 13;

pub(super) fn open(path: impl AsRef<Path>) -> Result<(), StorageError> {
    let path = path.as_ref();
    if path.try_exists().map_err(StorageError::Filesystem)? {
        let readonly = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        let found: u32 = readonly.pragma_query_one(None, "user_version")?;
        if found > SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchema {
                found,
                supported: SCHEMA_VERSION,
            });
        }
    }
    Ok(())
}
