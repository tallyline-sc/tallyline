// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exclusive JSON exports of already-previewed support evidence.

use std::path::PathBuf;

use serde::Serialize;
use tallyline_workflows::api::{
    ConfirmedDiagnosticExport, DiagnosticExportFailure, DiagnosticExportSink,
};

const FORMAT_VERSION: u8 = 1;
const MAX_EXPORT_BYTES: usize = 16 * 1024 * 1024;

/// An explicitly selected output file that is never replaced by an export.
pub struct DiagnosticFileExport {
    destination: PathBuf,
}

impl DiagnosticFileExport {
    /// Selects an output file without creating it or collecting diagnostic sources.
    #[must_use]
    pub const fn new(destination: PathBuf) -> Self {
        Self { destination }
    }
}

#[derive(Serialize)]
struct Document<'a> {
    format_version: u8,
    files: Vec<IncludedFile<'a>>,
}

#[derive(Serialize)]
struct IncludedFile<'a> {
    name: &'static str,
    text: &'a str,
}

impl DiagnosticExportSink for DiagnosticFileExport {
    fn write(&mut self, export: &ConfirmedDiagnosticExport) -> Result<(), DiagnosticExportFailure> {
        let files = export
            .entries()
            .iter()
            .map(|entry| IncludedFile {
                name: entry.name(),
                text: entry.text(),
            })
            .collect();
        let document = Document {
            format_version: FORMAT_VERSION,
            files,
        };
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|_| DiagnosticExportFailure::StorageUnavailable)?;
        if bytes.len() > MAX_EXPORT_BYTES {
            return Err(DiagnosticExportFailure::InputTooLarge);
        }
        crate::private_artifact::write_private_artifact(&self.destination, &bytes)
            .map_err(|_| DiagnosticExportFailure::StorageUnavailable)
    }
}
