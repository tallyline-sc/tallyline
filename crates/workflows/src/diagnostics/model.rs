// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed support-export sections and immutable privacy decisions.

use std::fmt;

pub(super) const MAX_SECTION_BYTES: usize = 512 * 1024;

/// A permitted text section; raw captures and game logs have no section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSection {
    /// Compiled application/runtime and operating-system versions.
    Build,
    /// Explicitly selected non-credential application preferences.
    Preferences,
    /// The latest connection-test metadata.
    Connection,
    /// Bounded application-log tails.
    Logs,
}

impl DiagnosticSection {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Build => "build.txt",
            Self::Preferences => "settings.txt",
            Self::Connection => "connection.txt",
            Self::Logs => "logs.txt",
        }
    }
}

/// A closed support-export refusal without source content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticExportFailure {
    /// A required source could not be collected safely.
    SourceUnavailable,
    /// Export storage did not commit the requested evidence.
    StorageUnavailable,
    /// The privacy context is incomplete or outside its bounds.
    InvalidPrivacy,
    /// Input exceeds its section or aggregate budget.
    InputTooLarge,
    /// A section occurs more than once or no section was supplied.
    InvalidSections,
}

/// Private source text that cannot be printed before masking.
pub struct DiagnosticInput {
    pub(super) section: DiagnosticSection,
    pub(super) text: String,
}

impl DiagnosticInput {
    /// Collects text from one permitted source section.
    ///
    /// # Errors
    /// Refuses text outside the bounded input budget.
    pub fn new(
        section: DiagnosticSection,
        text: impl Into<String>,
    ) -> Result<Self, DiagnosticExportFailure> {
        let text = text.into();
        if text.len() > MAX_SECTION_BYTES {
            return Err(DiagnosticExportFailure::InputTooLarge);
        }
        Ok(Self { section, text })
    }
}

impl fmt::Debug for DiagnosticInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DiagnosticInput(<redacted>)")
    }
}

/// Masked text for one fixed included filename.
#[derive(Debug)]
pub struct DiagnosticEntry {
    pub(super) section: DiagnosticSection,
    pub(super) text: String,
}

impl DiagnosticEntry {
    /// Returns the fixed, allowlisted included filename.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.section.name()
    }

    /// Returns only the already-masked preview text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// An immutable preview that grants no storage operation by itself.
#[derive(Debug)]
pub struct DiagnosticPreview {
    pub(super) entries: Vec<DiagnosticEntry>,
}

impl DiagnosticPreview {
    /// Returns every included section for the user's preview.
    #[must_use]
    pub fn entries(&self) -> &[DiagnosticEntry] {
        &self.entries
    }

    /// Acknowledges that issue attachments are public for this exact preview.
    #[must_use]
    pub fn confirm_public_attachment_warning(self) -> ConfirmedDiagnosticExport {
        ConfirmedDiagnosticExport {
            entries: self.entries,
        }
    }
}

/// The exact masked preview after explicit public-attachment acknowledgement.
#[derive(Debug)]
pub struct ConfirmedDiagnosticExport {
    entries: Vec<DiagnosticEntry>,
}

impl ConfirmedDiagnosticExport {
    /// Returns the frozen masked entries, without recollecting their sources.
    #[must_use]
    pub fn entries(&self) -> &[DiagnosticEntry] {
        &self.entries
    }
}
