// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Allowlisted diagnostic previews with explicit publication acknowledgement.

mod masking;
mod model;
mod privacy;

pub use model::{
    ConfirmedDiagnosticExport, DiagnosticEntry, DiagnosticExportFailure, DiagnosticInput,
    DiagnosticPreview, DiagnosticSection,
};
pub use privacy::DiagnosticPrivacy;

/// Writes only the exact already-previewed and acknowledged export.
pub trait DiagnosticExportSink {
    /// Persists the supplied immutable export without collecting new evidence.
    ///
    /// # Errors
    /// Returns a closed storage failure without paths or diagnostic content.
    fn write(&mut self, export: &ConfirmedDiagnosticExport) -> Result<(), DiagnosticExportFailure>;
}

/// Prepares masked support evidence before any export write is authorised.
pub struct DiagnosticExportApplication;

impl DiagnosticExportApplication {
    /// Masks the fixed permitted sections before exposing a preview.
    ///
    /// # Errors
    /// Refuses unusable or oversized diagnostic input.
    pub fn prepare(
        mut inputs: Vec<DiagnosticInput>,
        privacy: &DiagnosticPrivacy,
    ) -> Result<DiagnosticPreview, DiagnosticExportFailure> {
        const MAX_SECTIONS: usize = 4;
        if inputs.is_empty() || inputs.len() > MAX_SECTIONS {
            return Err(DiagnosticExportFailure::InvalidSections);
        }
        inputs.sort_by_key(|input| input.section);
        if inputs
            .windows(2)
            .any(|pair| matches!(pair, [left, right] if left.section == right.section))
        {
            return Err(DiagnosticExportFailure::InvalidSections);
        }
        let entries = inputs
            .into_iter()
            .map(|input| {
                let text = privacy.redact(&input.text);
                if text.len() > model::MAX_SECTION_BYTES {
                    return Err(DiagnosticExportFailure::InputTooLarge);
                }
                Ok(DiagnosticEntry {
                    section: input.section,
                    text,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(DiagnosticPreview { entries })
    }

    /// Writes the exact confirmed preview once, without recollecting or retrying.
    ///
    /// # Errors
    /// Returns the sink's closed failure without claiming a successful export.
    pub fn save(
        export: &ConfirmedDiagnosticExport,
        sink: &mut impl DiagnosticExportSink,
    ) -> Result<(), DiagnosticExportFailure> {
        sink.write(export)
    }
}
