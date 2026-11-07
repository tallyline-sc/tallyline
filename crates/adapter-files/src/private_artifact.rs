// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit exclusive private artifact writes.

use std::io::Write;
use std::path::Path;

/// A private artifact could not be created or completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateArtifactError {
    /// The destination was not an explicitly selected absolute path.
    Destination,
    /// A file or link already occupies the destination.
    AlreadyExists,
    /// Creation, writing or synchronising the new file failed.
    Write,
}

/// Creates one private byte artifact without replacing an existing file or following its link.
/// Unix files are owner-only; Windows inherits the caller-selected private directory's ACL.
/// A write failure may leave an incomplete new file, which must not be treated as evidence.
///
/// # Errors
/// Returns closed destination, existing-file or I/O failures without logging content or paths.
pub fn write_private_artifact(path: &Path, bytes: &[u8]) -> Result<(), PrivateArtifactError> {
    if !path.is_absolute() {
        return Err(PrivateArtifactError::Destination);
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            PrivateArtifactError::AlreadyExists
        } else {
            PrivateArtifactError::Write
        }
    })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| PrivateArtifactError::Write)
}

impl std::fmt::Display for PrivateArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "private artifact failed: {self:?}")
    }
}

impl std::error::Error for PrivateArtifactError {}
