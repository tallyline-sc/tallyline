// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fresh reference and attachment preparation without owning queue state.

use super::{SubmissionControlError, SubmissionQueuePreferences};
use tallyline_adapter_files::api::model::{AttachmentEncodingSettings, EncodedAttachment};
use tallyline_adapter_files::api::{encode_manual_attachment, restore_manual_attachment};
use tallyline_reference_data::api::model::{CachedReference, ReferenceSnapshot, UexEnvironment};
use tallyline_reference_data::api::{
    AcceptedPriorHistoryStore, ReferenceApplication, ReferenceDataCache,
};
use tallyline_reporting::api::model::{
    ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy, Report,
    SubmissionGate,
};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::ScreenshotPolicyStore;
use tallyline_workflows::api::SubmissionQueueStore;

struct SnapshotCache(Vec<CachedReference>);
impl ReferenceDataCache for SnapshotCache {
    type Error = std::convert::Infallible;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}
struct Fixed(Timestamp);
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        self.0
    }
}

pub(super) fn reference_snapshot(
    store: &mut tallyline_adapter_storage::api::SqliteReferenceCache,
    environment: UexEnvironment,
    now: Timestamp,
) -> Result<ReferenceSnapshot, SubmissionControlError> {
    let records =
        ReferenceDataCache::load(store).map_err(|_| SubmissionControlError::Unavailable)?;
    let snapshot = ReferenceApplication::load(SnapshotCache(records), Fixed(now))
        .map_err(|_| SubmissionControlError::Unavailable)?
        .snapshot(environment);
    let history = store
        .accepted_history()
        .map_err(|_| SubmissionControlError::Unavailable)?;
    snapshot
        .with_local_priors(&history.active(environment))
        .map_err(|_| SubmissionControlError::Invalid)
}

pub(super) fn gate(
    store: &mut tallyline_adapter_storage::api::SqliteReferenceCache,
    copies: &mut Option<tallyline_adapter_files::api::ManualWorkingCopies>,
    report: Report,
    account: &str,
    mode: SubmissionMode,
    preferences: &SubmissionQueuePreferences,
    now: Timestamp,
) -> Result<(SubmissionGate, Option<EncodedAttachment>), SubmissionControlError> {
    let environment = preferences
        .reviewer
        .environment_for(report.key().environment());
    let snapshot = reference_snapshot(store, environment, now)?;
    let preview = match (copies, report.attachment()) {
        (Some(copies), Some(_)) => restore_manual_attachment(&report, copies).ok(),
        (None, _) | (_, None) => None,
    };
    let working_copy = match report.attachment() {
        Some(attachment) => match (preview.as_ref(), attachment.working_copy()) {
            (Some(_), Some(digest)) => ReleaseWorkingCopy::Verified {
                binding: attachment.binding(),
                digest,
            },
            _ => ReleaseWorkingCopy::Unavailable,
        },
        None => ReleaseWorkingCopy::Absent,
    };
    let policy = match store.required_since(account) {
        Ok(Some(_)) => ReleaseScreenshotPolicy::Required,
        Ok(None) => ReleaseScreenshotPolicy::Unknown,
        Err(_) => ReleaseScreenshotPolicy::Unavailable,
    };
    let gate = preferences.reviewer.submission_gate(
        report,
        &snapshot,
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted(account.to_owned()),
            mode,
            policy,
            working_copy,
        ),
        preferences.age,
        now,
    );
    let history = store
        .queue_jobs()
        .map_err(SubmissionControlError::Repository)?;
    let gate = tallyline_workflows::api::assess_release_observation_history(&gate, Some(&history));
    let encoded = if gate.blocks(gate.report()).is_empty() {
        preview
            .map(|preview| {
                encode_manual_attachment(
                    gate.report(),
                    &preview,
                    &AttachmentEncodingSettings::default(),
                )
            })
            .transpose()
            .map_err(|_| SubmissionControlError::AttachmentUnavailable)?
    } else {
        None
    };
    Ok((gate, encoded))
}
