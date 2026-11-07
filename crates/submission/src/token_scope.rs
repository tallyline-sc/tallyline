// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nonsecret token associations supplied only by protected credential storage.

const REGISTRY_ID_BYTES: usize = 16;

/// An opaque registry generation and exact token association.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenScope([u8; 32]);
impl TokenScope {
    /// Validates independently nonzero registry and token identities.
    #[must_use]
    pub fn new(bytes: [u8; 32]) -> Option<Self> {
        let (registry, token) = bytes.split_at(REGISTRY_ID_BYTES);
        if registry.iter().all(|byte| *byte == 0) || token.iter().all(|byte| *byte == 0) {
            return None;
        }
        Some(Self(bytes))
    }
    /// Borrows the nonsecret persistent association.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// Reports whether both associations have the same protected registry generation.
    #[must_use]
    pub fn shares_registry(self, other: Self) -> bool {
        self.0.get(..REGISTRY_ID_BYTES) == other.0.get(..REGISTRY_ID_BYTES)
    }
}
