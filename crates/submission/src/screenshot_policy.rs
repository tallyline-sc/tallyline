// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dated account evidence without an invented user-API flag.

use crate::api::model::{AccountState, ScreenshotPolicy, ScreenshotRequirement, StorageFailure};
use tallyline_shared_kernel::api::model::Timestamp;

/// Persists screenshot evidence independently for each validated account identity.
pub trait ScreenshotPolicyStore {
    /// Reads only the newest observed requirement for this account.
    ///
    /// # Errors
    /// Returns a closed storage failure; callers must not treat it as unknown policy.
    fn required_since(&mut self, account: &str) -> Result<Option<Timestamp>, StorageFailure>;
    /// Atomically retains the newest observed requirement date.
    ///
    /// # Errors
    /// Returns a closed failure without claiming that policy was saved.
    fn remember(&mut self, account: &str, at: Timestamp) -> Result<(), StorageFailure>;
    /// Clears only this account's remembered evidence after an explicit user action.
    ///
    /// # Errors
    /// Returns a closed failure without claiming that policy was cleared.
    fn clear(&mut self, account: &str) -> Result<(), StorageFailure>;
}

/// Reads policy from current validated account state and saves observations before publication.
pub struct ScreenshotPolicyApplication<S> {
    store: S,
}
impl<S: ScreenshotPolicyStore> ScreenshotPolicyApplication<S> {
    /// Binds the policy use cases to durable storage.
    #[must_use]
    pub const fn new(store: S) -> Self {
        Self { store }
    }
    /// Resolves current knowledge for the accepted account without borrowing another account's flag.
    ///
    /// # Errors
    /// Returns unreadable policy instead of silently permitting a known-required account.
    pub fn current(&mut self, account: &AccountState) -> Result<ScreenshotPolicy, StorageFailure> {
        let Some(identity) = identity(account) else {
            return Ok(ScreenshotPolicy::Unknown);
        };
        Ok(self
            .store
            .required_since(identity)?
            .map_or(ScreenshotPolicy::Unknown, |observed_at| {
                ScreenshotPolicy::Required { observed_at }
            }))
    }
    /// Records the documented answer against the account used by that submission attempt.
    ///
    /// # Errors
    /// Refuses an unvalidated identity and failed persistence.
    pub fn observe(
        &mut self,
        account: &AccountState,
        answer: ScreenshotRequirement,
        at: Timestamp,
    ) -> Result<(), StorageFailure> {
        let identity = identity(account).ok_or(StorageFailure::Unavailable)?;
        match answer {
            ScreenshotRequirement::Required => self.store.remember(identity, at),
        }
    }
    /// Explicitly forgets this account's remembered requirement to try without a screenshot.
    ///
    /// # Errors
    /// Refuses an unvalidated identity and failed persistence.
    pub fn try_without_screenshot(&mut self, account: &AccountState) -> Result<(), StorageFailure> {
        self.store
            .clear(identity(account).ok_or(StorageFailure::Unavailable)?)
    }
}
fn identity(account: &AccountState) -> Option<&str> {
    match account {
        AccountState::Validated { username, .. } => Some(username),
        AccountState::Unconfigured | AccountState::Rejected(_) => None,
    }
}
