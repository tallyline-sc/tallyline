// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! In-memory exact-token distinctions supplied by protected credential storage.

use crate::api::model::TokenScope;

/// A known token scope or bounded proof of inequality with retained protected tokens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TokenAdmission {
    known: Option<TokenScope>,
    distinct: Vec<TokenScope>,
}
impl TokenAdmission {
    /// Maximum retained associations represented in one protected comparison proof.
    pub const MAX_DISTINCT_SCOPES: usize = 64;
    /// Creates authority only for a previously stored exact token association.
    #[must_use]
    pub const fn from_scope(known: Option<TokenScope>) -> Self {
        Self {
            known,
            distinct: Vec::new(),
        }
    }
    /// Retains only distinct associations from one intact protected registry generation.
    #[must_use]
    pub fn distinct_from(scopes: Vec<TokenScope>) -> Option<Self> {
        if scopes.len() > Self::MAX_DISTINCT_SCOPES {
            return None;
        }
        for (index, scope) in scopes.iter().enumerate() {
            if scopes
                .iter()
                .take(index)
                .any(|previous| previous == scope || !previous.shares_registry(*scope))
            {
                return None;
            }
        }
        Some(Self {
            known: None,
            distinct: scopes,
        })
    }
    /// Returns durable identity only when the exact token already has an association.
    #[must_use]
    pub const fn known_scope(&self) -> Option<TokenScope> {
        self.known
    }
    /// Proves inequality only with an exact retained association, never with absence or loss.
    #[must_use]
    pub fn proves_distinct(&self, scope: TokenScope) -> bool {
        self.known
            .is_some_and(|known| known.shares_registry(scope) && known != scope)
            || self.distinct.contains(&scope)
    }
}
