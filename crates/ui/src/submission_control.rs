// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit version-bound release intents and passive committed feedback.

use crate::api::model::ResolutionQueueAction;
use tallyline_reporting::api::model::{ReleaseBlockReason, Report, ReportId};
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::model::{
    JobId, QueueRefusal, SubmissionJob, VerifiedReceiptMapping,
};

/// The user-selected release path; onboarding always selects dry-run mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionIntent {
    /// Release in the global mode displayed before the action.
    Manual(SubmissionMode),
    /// Exercise the same queue with a dry-run request.
    OnboardingTest,
}
/// Bounded explicit control without secret values or mutable row payloads.
pub enum SubmissionControl {
    /// Requests one account lifecycle read from the exact displayed versions.
    RefreshLifecycle {
        /// Stable local submission identity.
        id: JobId,
        /// Committed source revision displayed before the action.
        expected: u64,
        /// Committed lifecycle revision, absent before its first admission.
        expected_lifecycle: Option<u64>,
    },
    /// Explicitly partitions the exact displayed waiting job after fresh full checks.
    Split {
        /// Stable original job identity.
        id: JobId,
        /// Committed revision displayed before the action.
        expected: u64,
    },
    /// Applies an explicit action to the exact displayed eligible immutable origin.
    Resolve {
        /// Stable original job identity.
        id: JobId,
        /// Displayed committed concurrency revision.
        expected: u64,
        /// The local decision, read-only check or separate Draft action.
        action: ResolutionQueueAction,
    },
    /// Retry an eligible held job against its displayed revision.
    Retry {
        /// Stable job identity.
        id: JobId,
        /// Displayed committed concurrency revision.
        expected: u64,
    },
    /// Release the exact Draft for the account displayed when the action was requested.
    Release {
        /// Stable Draft identity.
        id: ReportId,
        /// Displayed committed concurrency version.
        expected: u64,
        /// Accepted account displayed before the action.
        account: String,
        /// Fixed user intent.
        intent: SubmissionIntent,
    },
    /// Cancel an unsent job against its displayed revision.
    Cancel {
        /// Stable job identity.
        id: JobId,
        /// Displayed committed concurrency revision.
        expected: u64,
    },
    /// Explicitly confirm an exact older row against the displayed committed history scope.
    ConfirmNewer {
        /// Stable Draft identity.
        id: ReportId,
        /// Displayed committed concurrency version.
        expected: u64,
        /// Exact local row whose unchanged values were shown.
        row: tallyline_shared_kernel::api::model::RowId,
        /// Displayed account, mode, target and newest accepted original observation.
        evidence: tallyline_reporting::api::model::NewerObservationEvidence,
    },
    /// Explicitly confirm the unchanged observation age of this Draft.
    ConfirmAge {
        /// Stable Draft identity.
        id: ReportId,
        /// Displayed committed concurrency version.
        expected: u64,
    },
}
/// Closed actionable feedback without paths, keys or remote response text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmissionFeedback {
    /// The explicit partition committed two released replacement Reports.
    Split,
    /// The displayed account, settings or source version changed.
    Changed,
    /// The full current release gate refused the Draft.
    Blocked(Vec<ReleaseBlockReason>),
    /// Independent current account or service facts hold the fixed queued job.
    Held(tallyline_submission::api::model::SendHoldReason),
    /// Changed observation metadata requires renewed review and release.
    Returned(tallyline_submission::api::model::SendReturnReason),
    /// The aggregate refused the requested lifecycle action.
    Refused(QueueRefusal),
    /// Required local state or a bounded worker is unavailable.
    Unavailable,
    /// The selected private evidence cannot be verified.
    AttachmentUnavailable,
}
/// One passive snapshot prepared on the dedicated CPU owner after persistence.
#[derive(Debug)]
pub struct SubmissionUpdate {
    /// Immutable compiled stable release supplied by the queue owner.
    pub client_version: Option<tallyline_shared_kernel::api::model::ReleaseVersion>,
    /// Durable queue-wide service causes independent of Report phase.
    pub service_fences: Vec<tallyline_submission::api::model::SubmissionServiceFence>,
    /// Durable current-token quota holds from authenticated reads, independent of jobs.
    pub authenticated_quotas: Vec<tallyline_submission::api::model::TokenQuotaObservation>,
    /// Actual committed jobs, including Sending after a failed outcome save.
    pub jobs: Vec<SubmissionJob>,
    /// Exact-version passive waits without changing durable job phases.
    pub waiting: Vec<tallyline_submission::api::model::SubmissionWait>,
    /// Injected assessment time for exact current cooldown row groups.
    pub assessed_at: tallyline_shared_kernel::api::model::Timestamp,
    /// Persisted independently verified receipt mappings.
    pub mappings: Vec<VerifiedReceiptMapping>,
    /// Durable lifecycle evidence independent of the delivery result.
    pub lifecycles: Vec<tallyline_submission::api::model::SubmissionLifecycle>,
    /// Whether the owner retains a lifecycle read or completed save-only retry.
    pub lifecycle_busy: bool,
    /// Current validated account whose credentials may admit a lifecycle read.
    pub lifecycle_account: Option<String>,
    /// Current persisted Drafts after release, return or cancellation.
    pub reports: Vec<Report>,
    /// Whether this snapshot completes the one outstanding explicit control action.
    pub completed: bool,
    /// Independent queue-worker feedback, without completing or replacing a user action.
    pub background_feedback: Option<SubmissionFeedback>,
    /// Closed feedback for the latest control operation.
    pub feedback: Option<SubmissionFeedback>,
}
