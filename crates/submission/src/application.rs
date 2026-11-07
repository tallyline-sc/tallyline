// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic state-and-outbox application operations.

use crate::api::model::{JobId, QueueRefusal, SubmissionChange, SubmissionCommand, SubmissionJob};

/// A privacy-safe durable queue failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionRepositoryError {
    /// A concurrent command already changed the job.
    Conflict,
    /// Persisted data violates the submission contract.
    Invalid,
    /// Storage could not complete its bounded operation.
    Unavailable,
}

/// A refused application command without speculative state publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionApplicationError {
    /// No durable job has this identity.
    Missing,
    /// The aggregate refused this phase transition.
    Refused(QueueRefusal),
    /// No new state is published when its transaction fails.
    Repository(SubmissionRepositoryError),
}
impl From<SubmissionRepositoryError> for SubmissionApplicationError {
    fn from(value: SubmissionRepositoryError) -> Self {
        Self::Repository(value)
    }
}

/// Durable jobs and their outcome events, committed atomically.
pub trait SubmissionJobRepository {
    /// Loads a validated immutable aggregate.
    ///
    /// # Errors
    /// Returns corrupt or unavailable storage.
    fn load(&mut self, id: JobId) -> Result<Option<SubmissionJob>, SubmissionRepositoryError>;
    /// Compares the expected revision and saves state and event in one transaction.
    ///
    /// # Errors
    /// Returns a stale version, invalid state or unavailable storage; publishes nothing on failure.
    fn save(&mut self, change: &SubmissionChange) -> Result<(), SubmissionRepositoryError>;
}

/// Applies closed outcome commands after their atomic persistence succeeds.
pub struct SubmissionApplication<R> {
    repository: R,
}
impl<R: SubmissionJobRepository> SubmissionApplication<R> {
    /// Composes the application with its owned persistence port.
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }
    /// Applies one command against the exact version observed by its caller.
    ///
    /// # Errors
    /// Refuses missing or stale jobs, invalid transitions and failed transactions.
    pub fn apply(
        &mut self,
        id: JobId,
        expected_revision: u64,
        command: SubmissionCommand,
    ) -> Result<SubmissionJob, SubmissionApplicationError> {
        let current = self
            .repository
            .load(id)?
            .ok_or(SubmissionApplicationError::Missing)?;
        if current.revision() != expected_revision {
            return Err(SubmissionRepositoryError::Conflict.into());
        }
        let change = current
            .apply(command)
            .map_err(SubmissionApplicationError::Refused)?;
        if change.state().revision() != expected_revision {
            self.repository.save(&change)?;
        }
        Ok(change.into_state())
    }
    /// Returns the owned persistence port for shutdown or composition.
    #[must_use]
    pub fn into_repository(self) -> R {
        self.repository
    }
}
