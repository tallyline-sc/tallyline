// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded complete-record reads from fixed application log files.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use tallyline_workflows::api::{DiagnosticExportFailure, DiagnosticInput, DiagnosticSection};

const LOG_FILES: [&str; 3] = ["tallyline.log", "tallyline.1.log", "tallyline.2.log"];
const MAX_TAIL_BYTES: u64 = 64 * 1024;

/// Reads only fixed application-log names from the owned log directory.
pub struct DiagnosticLogFiles {
    directory: PathBuf,
}

impl DiagnosticLogFiles {
    /// Selects the already-resolved application log directory without accessing it.
    #[must_use]
    pub const fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// Collects complete UTF-8 records without exposing unmasked source text.
    /// Returns no section when the directory or every permitted file is absent.
    ///
    /// # Errors
    /// Refuses relative paths, visible links, non-files, unstable reads and invalid UTF-8.
    pub fn collect(&self) -> Result<Option<DiagnosticInput>, DiagnosticExportFailure> {
        if !self.directory.is_absolute() {
            return Err(DiagnosticExportFailure::SourceUnavailable);
        }
        let metadata = match fs::symlink_metadata(&self.directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(DiagnosticExportFailure::SourceUnavailable),
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(DiagnosticExportFailure::SourceUnavailable);
        }
        let mut text = String::new();
        for name in LOG_FILES.iter().rev() {
            let Some(tail) = read_tail(&self.directory.join(name))? else {
                continue;
            };
            text.push_str(&format!("[{name}]\n{tail}\n"));
        }
        if text.is_empty() {
            return Ok(None);
        }
        DiagnosticInput::new(DiagnosticSection::Logs, text).map(Some)
    }
}

fn read_tail(path: &Path) -> Result<Option<String>, DiagnosticExportFailure> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(DiagnosticExportFailure::SourceUnavailable),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(DiagnosticExportFailure::SourceUnavailable);
    }
    let mut file = File::open(path).map_err(|_| DiagnosticExportFailure::SourceUnavailable)?;
    let before = file
        .metadata()
        .map_err(|_| DiagnosticExportFailure::SourceUnavailable)?;
    if !before.is_file() {
        return Err(DiagnosticExportFailure::SourceUnavailable);
    }
    let modified = before
        .modified()
        .map_err(|_| DiagnosticExportFailure::SourceUnavailable)?;
    let start = before.len().saturating_sub(MAX_TAIL_BYTES + 1);
    let count = usize::try_from(before.len() - start)
        .map_err(|_| DiagnosticExportFailure::SourceUnavailable)?;
    file.seek(SeekFrom::Start(start))
        .map_err(|_| DiagnosticExportFailure::SourceUnavailable)?;
    let mut bytes = vec![0; count];
    file.read_exact(&mut bytes)
        .map_err(|_| DiagnosticExportFailure::SourceUnavailable)?;
    let after = file
        .metadata()
        .map_err(|_| DiagnosticExportFailure::SourceUnavailable)?;
    if after.len() != before.len()
        || after
            .modified()
            .map_err(|_| DiagnosticExportFailure::SourceUnavailable)?
            != modified
    {
        return Err(DiagnosticExportFailure::SourceUnavailable);
    }
    complete_lines(&bytes, before.len() > MAX_TAIL_BYTES).map(Some)
}

fn complete_lines(bytes: &[u8], clipped: bool) -> Result<String, DiagnosticExportFailure> {
    let start = if clipped {
        bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| index + 1)
    } else {
        0
    };
    let end = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    if start >= end {
        return Ok("[no complete app log records within the bounded tail]".to_owned());
    }
    let complete = bytes
        .get(start..end)
        .ok_or(DiagnosticExportFailure::SourceUnavailable)?;
    std::str::from_utf8(complete)
        .map(str::to_owned)
        .map_err(|_| DiagnosticExportFailure::SourceUnavailable)
}
