// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Storage handles prepared before application workers start normal processing.

use tallyline_adapter_files::api::ConfigFile;
use tallyline_adapter_storage::api::SqliteReferenceCache;

/// Settings and cache after coordinated bootstrap migrations.
pub struct PreparedStorage {
    /// The single settings writer, containing only the current schema.
    pub settings: ConfigFile,
    /// The SQLite connection to move to the owning reference worker.
    pub cache: SqliteReferenceCache,
}

pub use crate::build_info::{BuildInformation, BuildInformationError, BuildKind, BuildTarget};

pub use crate::receipt_lookup::{HttpAdmissionFailure, ReceiptLookupOutcome};
