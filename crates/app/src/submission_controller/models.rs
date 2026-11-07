// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed preferences and closed failures for the app-owned submission queue.

use tallyline_reporting::api::ManualReviewer;
use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};
use tallyline_submission::api::model::SubmissionRepositoryError;
use tallyline_workflows::api::{ManualDispatchError, ManualReleaseError};

/// Fresh validated policy read by the application before release and before every send.
#[derive(Clone, PartialEq, Eq)]
pub struct SubmissionQueuePreferences {
    /// Actual channel mapping and tighten-only numerical review settings.
    pub reviewer: ManualReviewer,
    /// Tighten-only observation age and confirmation lifetime.
    pub age: ObservationAgeSettings,
    /// Current global mode, affecting new manual releases only.
    pub mode: SubmissionMode,
}
/// Closed queue-control failures without credentials, paths, pixels or remote messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionControlError {
    /// The full release gate refused the current Draft.
    Release(ManualReleaseError),
    /// Full fresh dispatch checks refused the fixed released snapshot.
    Dispatch(ManualDispatchError),
    /// The aggregate refused the requested phase transition.
    Refused(tallyline_submission::api::model::QueueRefusal),
    /// Durable queue state could not be read or saved.
    Repository(SubmissionRepositoryError),
    /// Settings, current references or a required worker are unavailable.
    Unavailable,
    /// Selected private evidence cannot be verified or encoded within its bound.
    AttachmentUnavailable,
    /// The requested identity, phase or version is inconsistent.
    Invalid,
    /// The displayed source or chosen global mode changed.
    Conflict,
}
