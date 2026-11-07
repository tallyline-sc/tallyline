// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact token-bearing request admission supplied by the shared application owner.

use crate::api::UexReader;
use std::sync::Arc;
use tallyline_submission::api::model::{CredentialValues, TokenAdmission};

/// A live nonsecret quota callback for token-bearing GET and POST routes.
#[derive(Clone)]
pub struct AuthenticatedReadGate {
    allows: Arc<dyn Fn(&TokenAdmission) -> bool + Send + Sync>,
}
impl AuthenticatedReadGate {
    /// Borrows the shared owner's current quota decision without capturing credentials.
    #[must_use]
    pub fn new(allows: impl Fn(&TokenAdmission) -> bool + Send + Sync + 'static) -> Self {
        Self {
            allows: Arc::new(allows),
        }
    }
    fn permits(&self, proof: &TokenAdmission) -> bool {
        (self.allows)(proof)
    }
}
impl std::fmt::Debug for AuthenticatedReadGate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthenticatedReadGate")
            .finish_non_exhaustive()
    }
}
impl UexReader {
    /// Applies shared quota admission to token-bearing validation, receipt reads and submissions.
    #[must_use]
    pub fn with_authenticated_read_gate(mut self, gate: AuthenticatedReadGate) -> Self {
        self.read_gate = Some(gate);
        self
    }
    pub(crate) fn token_request_allowed(&self, values: &CredentialValues) -> bool {
        let locally_allowed = self.quota_observations.lock().is_ok_and(|observations| {
            !observations
                .as_ref()
                .iter()
                .any(|observation| observation.blocks_admission(values.token_admission()))
        });
        locally_allowed
            && self
                .read_gate
                .as_ref()
                .is_none_or(|gate| gate.permits(values.token_admission()))
    }
}
