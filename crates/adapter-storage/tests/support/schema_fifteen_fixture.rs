// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restores exact synthetic historical schema and rows for reader-admission contracts.

#[path = "exported_sqlite.rs"]
mod exported_sqlite;

/// Restores the frozen historical SQLite export into an empty path.
///
/// # Errors
/// Returns malformed fixture data or a failed SQLite operation.
pub fn restore(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    exported_sqlite::restore(path, include_str!("../fixtures/schema-fifteen.json"), 15)
}
