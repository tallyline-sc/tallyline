// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded technical error identifiers for public-contract diagnostics.

const MAX_CODE_BYTES: usize = 64;

/// An ASCII technical error identifier with no whitespace or message text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicContractCode(String);

impl PublicContractCode {
    pub(super) fn from_identifier(text: &str) -> Option<Self> {
        let mut bytes = text.bytes();
        let first = bytes.next()?;
        if text.len() > MAX_CODE_BYTES
            || !(first.is_ascii_alphabetic() || first == b'_')
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return None;
        }
        Some(Self(text.to_owned()))
    }

    /// Returns the validated technical identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
