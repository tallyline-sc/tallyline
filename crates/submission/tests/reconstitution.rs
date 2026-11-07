// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Persistence reconstitution rejects ambiguous attempts instead of replaying them.

use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    JobId, JobPhase, PayloadHash, StoredSubmissionAttempt, StoredSubmissionJob, SubmissionHeader,
    SubmissionJob, SubmissionPauseReason, SubmissionRequest, SubmissionResult, SubmissionRow,
};

fn queued() -> Option<SubmissionJob> {
    SubmissionJob::queued(
        JobId(7),
        SubmissionRequest::new(
            SubmissionHeader {
                report: ReportId(4),
                report_version: 3,
                account: "test-account".to_owned(),
                mode: SubmissionMode::DryRun,
                terminal: TerminalId(89),
                side: TradeSide::Sell,
                environment: UexEnvironment::Live,
                game_version: "4.10.2".to_owned(),
                faction_affinity: None,
                observed_at: Timestamp::from_seconds(100),
            },
            vec![
                SubmissionRow::new(
                    RowId(2),
                    CommodityId(7),
                    "120".parse().ok()?,
                    "0".parse().ok()?,
                    1,
                )
                .ok()?,
            ],
        )
        .ok()?,
    )
    .ok()
}
fn stored(job: &SubmissionJob) -> StoredSubmissionJob {
    StoredSubmissionJob {
        resolution_actions: job.resolution_actions().to_vec(),
        id: job.id(),
        revision: job.revision(),
        request: job.request().clone(),
        phase: job.phase(),
        result: job.result().cloned(),
        review_reason: job.review_reason(),
        service_pause_acknowledged: job.service_pause_acknowledged(),
        client_update: job.client_update(),
        attempts: job
            .attempts()
            .iter()
            .map(|attempt| StoredSubmissionAttempt {
                started_at: attempt.started_at(),
                rows: attempt.rows(),
                payload_hash: attempt.payload_hash(),
                date_added: attempt.date_added(),
                ended_at: attempt.ended_at(),
                outcome: attempt.outcome(),
                receipt_checked: attempt.receipt_checked(),
                account_checks: Some(attempt.account_checks().to_vec()),
                account_evidence: attempt.account_evidence().cloned(),
                client_update: attempt.client_update(),
            })
            .collect(),
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__reconstituted_jobs_preserve_fixed_snapshots_and_interrupted_sending_stays_closed() {
    let queued = queued().unwrap();
    let sending = queued
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([8; 32]),
            Timestamp::from_seconds(95),
        )
        .unwrap();
    let unknown = sending.recover(Timestamp::from_seconds(120)).unwrap();
    let success = sending
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(120),
        )
        .unwrap();
    for job in [
        queued.clone(),
        queued.cancel().unwrap(),
        queued
            .return_for_review(
                tallyline_submission::api::model::SendReturnReason::GameVersionChanged,
            )
            .unwrap(),
        queued.recover(Timestamp::from_seconds(120)).unwrap(),
        sending.clone(),
        unknown.clone(),
        success,
    ] {
        assert_eq!(SubmissionJob::reconstitute(stored(&job)).unwrap(), job);
    }
    let restored = SubmissionJob::reconstitute(stored(&sending)).unwrap();
    assert_eq!(
        restored.recover(Timestamp::from_seconds(120)).unwrap(),
        unknown
    );
    assert!(restored.cancel().is_err());
    let safe_code =
        tallyline_submission::api::model::UexErrorCode::new("Future_Code".to_owned()).unwrap();
    let pending = sending
        .complete(
            SubmissionResult::AccountRefusal {
                check: tallyline_submission::api::model::SubmissionAccountCheck::NotRequired,
                code: Some(safe_code.clone()),
                http_status: 403,
                reason: SubmissionPauseReason::KeyCheckUnavailable,
                outcome: Box::new(SubmissionResult::NeedsOutcomeCheck {
                    code: safe_code,
                    http_status: 403,
                }),
            },
            Timestamp::from_seconds(120),
        )
        .unwrap();
    assert_eq!(
        SubmissionJob::reconstitute(stored(&pending)).unwrap(),
        pending
    );
    let mut corrupt_account = stored(&pending);
    if let Some(SubmissionResult::AccountRefusal { code, .. }) = &mut corrupt_account.result {
        *code = tallyline_submission::api::model::UexErrorCode::new("future-code".to_owned());
    }
    assert!(
        SubmissionJob::reconstitute(corrupt_account).is_err(),
        "contradictory account and receipt provenance must be rejected"
    );
    let account_retry = sending
        .complete(
            SubmissionResult::AccountRefusal {
                check: tallyline_submission::api::model::SubmissionAccountCheck::NotRequired,
                code: Some(
                    tallyline_submission::api::model::UexErrorCode::new("Future_Code".to_owned())
                        .unwrap(),
                ),
                http_status: 403,
                reason: SubmissionPauseReason::KeyChecked,
                outcome: Box::new(SubmissionResult::Paused(SubmissionPauseReason::KeyChecked)),
            },
            Timestamp::from_seconds(120),
        )
        .unwrap()
        .retry()
        .unwrap();
    assert!(
        format!("{:?}", account_retry.attempts()).contains("Future_Code"),
        "an allowed retry must retain the original account refusal in its attempt"
    );
    let pending_key = sending
        .complete(
            SubmissionResult::AccountRefusal {
                code: Some(
                    tallyline_submission::api::model::UexErrorCode::new(
                        "invalid_secret_key".to_owned(),
                    )
                    .unwrap(),
                ),
                http_status: 503,
                reason: SubmissionPauseReason::KeyCheckUnavailable,
                check: tallyline_submission::api::model::SubmissionAccountCheck::Pending,
                outcome: Box::new(SubmissionResult::Paused(
                    SubmissionPauseReason::KeyCheckUnavailable,
                )),
            },
            Timestamp::from_seconds(120),
        )
        .unwrap();
    assert!(
        !pending_key.can_retry(),
        "an unfinished account GET must not authorise a POST retry"
    );
    let checked = pending_key
        .resolve_account_check(SubmissionPauseReason::KeyChecked)
        .unwrap();
    assert!(!checked.attempts()[0].receipt_checked());
    assert_eq!(
        checked.attempts()[0]
            .account_evidence()
            .unwrap()
            .origin_revision(),
        Some(3)
    );
    assert_eq!(
        checked.attempts()[0]
            .account_evidence()
            .unwrap()
            .original_action(),
        SubmissionPauseReason::KeyCheckUnavailable
    );
    assert_eq!(
        checked.attempts()[0].account_evidence().unwrap().check(),
        tallyline_submission::api::model::SubmissionAccountCheck::Completed(
            SubmissionPauseReason::KeyChecked
        )
    );
    assert_eq!(
        SubmissionJob::reconstitute(stored(&checked)).unwrap(),
        checked
    );
    let unavailable = pending_key
        .resolve_account_check(SubmissionPauseReason::KeyCheckUnavailable)
        .unwrap();
    let repeated = unavailable
        .resolve_account_check(SubmissionPauseReason::KeyChecked)
        .unwrap();
    for final_job in [
        repeated.clone(),
        repeated.cancel().unwrap(),
        repeated.recover(Timestamp::from_seconds(130)).unwrap(),
        unavailable
            .cancel()
            .unwrap()
            .resolve_account_check(SubmissionPauseReason::KeyChecked)
            .unwrap(),
    ] {
        assert_eq!(
            SubmissionJob::reconstitute(stored(&final_job)).unwrap(),
            final_job
        );
    }
    for actions in [
        vec![],
        vec![SubmissionPauseReason::KeyChecked],
        vec![
            SubmissionPauseReason::KeyCheckUnavailable,
            SubmissionPauseReason::KeyCheckUnavailable,
            SubmissionPauseReason::KeyChecked,
        ],
        vec![
            SubmissionPauseReason::KeyChecked,
            SubmissionPauseReason::Reauthenticate,
        ],
    ] {
        let mut corrupt = stored(&repeated);
        corrupt.attempts[0].account_checks = Some(actions);
        assert!(SubmissionJob::reconstitute(corrupt).is_err());
    }
    let mut legacy = stored(&checked);
    legacy.attempts[0].account_checks = None;
    assert_eq!(SubmissionJob::reconstitute(legacy).unwrap(), checked);
    let retried = checked.retry().unwrap();
    assert_eq!(
        SubmissionJob::reconstitute(stored(&retried)).unwrap(),
        retried
    );
    assert!(format!("{:?}", retried.attempts()).contains("invalid_secret_key"));
    let acknowledged = pending.retry().unwrap();
    let settled = acknowledged
        .resolve_outcome_check(SubmissionResult::Paused(
            SubmissionPauseReason::KeyCheckUnavailable,
        ))
        .unwrap();
    assert!(
        !settled.can_retry(),
        "queue-only acknowledgement must never become origin POST authority after a later receipt result"
    );
    assert_eq!(
        SubmissionJob::reconstitute(stored(&settled)).unwrap(),
        settled
    );
    let snapshot = stored(&sending);
    let mut corrupt = snapshot.clone();
    corrupt.phase = JobPhase::Queued;
    assert!(SubmissionJob::reconstitute(corrupt).is_err());
    let mut corrupt = snapshot.clone();
    corrupt.revision = 1;
    assert!(SubmissionJob::reconstitute(corrupt).is_err());
    let mut corrupt = snapshot.clone();
    corrupt.attempts[0].rows = 2;
    assert!(SubmissionJob::reconstitute(corrupt).is_err());
    let mut corrupt = snapshot.clone();
    corrupt.attempts[0].date_added = Timestamp::from_seconds(101);
    assert!(SubmissionJob::reconstitute(corrupt).is_err());
    let mut corrupt = snapshot;
    corrupt.attempts[0].ended_at = Some(Timestamp::from_seconds(120));
    assert!(SubmissionJob::reconstitute(corrupt).is_err());
}
