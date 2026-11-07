// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public SQLite backup and restore-marker records.

use tallyline_shared_kernel::api::model::Timestamp;

/// The marker retained inside a consistent database backup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupMetadata {
    /// The single sampled time shared with the configuration backup.
    pub taken_at: Timestamp,
    /// The source database schema, unchanged in the backup.
    pub from_schema: u32,
    /// The accompanying configuration schema, absent when no file existed.
    pub configuration_schema: Option<u32>,
}
