// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only access to packaged installation token data.

use std::io::{self, Read};
use std::path::PathBuf;
use tallyline_submission::api::model::{ReleaseToken, ReleaseTokenFailure};

const TOKEN_FILENAME: &str = "tallyline-app-token";
const MAX_FILE_BYTES: usize = 4096;
const READ_LIMIT: u64 = 4097;

/// Reads the fixed token file beside an injected executable directory.
pub struct ReleaseTokenFile {
    directory: PathBuf,
}

impl ReleaseTokenFile {
    /// Selects the executable's installation directory without reading a token.
    #[must_use]
    pub const fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// Reads token data without modifying the installation or exposing raw failures.
    ///
    /// # Errors
    /// Returns only an unreadable or malformed installation-data outcome.
    pub fn read(&self) -> Result<Option<ReleaseToken>, ReleaseTokenFailure> {
        let file = match std::fs::File::open(self.directory.join(TOKEN_FILENAME)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ReleaseTokenFailure::Unreadable),
        };
        let mut bytes = Vec::new();
        file.take(READ_LIMIT)
            .read_to_end(&mut bytes)
            .map_err(|_| ReleaseTokenFailure::Unreadable)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(ReleaseTokenFailure::Malformed);
        }
        let value = std::str::from_utf8(&bytes).map_err(|_| ReleaseTokenFailure::Malformed)?;
        let value = value.trim_matches(|character: char| character.is_ascii_whitespace());
        if value.is_empty() {
            return Ok(None);
        }
        ReleaseToken::new(value.to_owned())
            .map(Some)
            .map_err(|_| ReleaseTokenFailure::Malformed)
    }
}
