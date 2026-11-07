// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unknown resolution through public release, history and real SQLite contracts.

use super::support;
use rusqlite::Connection;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_game::api::model::EnvironmentMapping;
use tallyline_reporting::api::ManualReviewer;
use tallyline_reporting::api::ReportHistoryRepository;
use tallyline_reporting::api::model::ReportDelivery;
use tallyline_reporting::api::model::{
    ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
    ReportState, StockSettings,
};
use tallyline_shared_kernel::api::model::{
    CommodityId, ObservationAgeSettings, SubmissionMode, TerminalId, Timestamp, UexEnvironment,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_nf_11__received_unknown_resolution_commits_and_reopens_without_rewriting_attempt() {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, ReceiptDecision, SubmissionCommand, SubmissionResolutionAction,
    };
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-unknown-received-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let unknown = sending
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unknown).unwrap();
    let received = unknown
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::Received,
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    let fault = Connection::open(&path).unwrap();
    fault.execute_batch("CREATE TRIGGER fail_unknown_resolution BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'test resolution rollback'); END;").unwrap();
    assert!(SubmissionJobRepository::save(&mut store, &received).is_err());
    assert_eq!(
        SubmissionJobRepository::load(&mut store, released.job().id())
            .unwrap()
            .as_ref(),
        Some(unknown.state())
    );
    assert_eq!(
        ReportHistoryRepository::retained_report(&mut store, gate.source().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::OutcomeUnknown
    );
    fault
        .execute_batch("DROP TRIGGER fail_unknown_resolution")
        .unwrap();
    drop(fault);
    let saved = SubmissionJobRepository::save(&mut store, &received);
    assert!(
        saved.is_ok(),
        "explicit received and its Report must commit together: {saved:?}"
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let job = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(&job, received.state());
    assert_eq!(job.attempts(), unknown.state().attempts());
    assert!(job.verified_row_receipts().next().is_none());
    assert!(
        tallyline_workflows::api::accepted_submission_priors(&job)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ReportHistoryRepository::retained_report(&mut reopened, gate.source().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::Submitted
    );
    assert!(job.retry().is_err());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__not_received_retains_origin_and_explicit_duplicate_creates_a_fresh_unreleased_draft() {
    use tallyline_reporting::api::{ReportHistoryRepository, ReportRepository};
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, ReceiptDecision, SubmissionCommand, SubmissionResolutionAction,
    };
    use tallyline_workflows::api::{ManualReleaseStore, ResolutionDraftStore};
    let path = std::env::temp_dir().join(format!(
        "tallyline-unknown-duplicate-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let unknown = sending
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unknown).unwrap();
    assert!(
        store
            .duplicate_resolved(
                unknown.state().id(),
                unknown.state().revision(),
                Timestamp::from_seconds(1002)
            )
            .is_err()
    );
    let not_received = unknown
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::NotReceived,
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &not_received).unwrap();
    assert!(store.drafts().unwrap().is_empty());
    let fault = Connection::open(&path).unwrap();
    fault.execute_batch("CREATE TRIGGER fail_unknown_duplicate BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'test duplicate rollback'); END;").unwrap();
    assert!(
        store
            .duplicate_resolved(
                not_received.state().id(),
                not_received.state().revision(),
                Timestamp::from_seconds(1003)
            )
            .is_err()
    );
    assert!(store.drafts().unwrap().is_empty());
    assert_eq!(
        SubmissionJobRepository::load(&mut store, not_received.state().id())
            .unwrap()
            .as_ref(),
        Some(not_received.state())
    );
    fault
        .execute_batch("DROP TRIGGER fail_unknown_duplicate")
        .unwrap();
    drop(fault);
    let duplicated = store.duplicate_resolved(
        not_received.state().id(),
        not_received.state().revision(),
        Timestamp::from_seconds(1003),
    );
    assert!(
        duplicated.is_ok(),
        "the explicit specified action must create a separate Draft: {duplicated:?}"
    );
    let duplicated = duplicated.unwrap();
    assert_ne!(duplicated.draft.id(), released.report().id());
    assert_eq!(duplicated.draft.state(), ReportState::Draft);
    assert!(duplicated.draft.release_binding().is_none());
    assert_eq!(duplicated.draft.observed_at(), Timestamp::from_seconds(900));
    assert_eq!(duplicated.draft.game_version(), Some("4.10.2"));
    assert_eq!(
        duplicated.draft.rows()[0].price().unwrap().to_string(),
        "123.45"
    );
    assert_eq!(duplicated.draft.rows()[0].stock().unwrap().to_string(), "0");
    assert_eq!(duplicated.draft.rows()[0].status().unwrap().get(), 3);
    assert_eq!(duplicated.origin.attempts(), unknown.state().attempts());
    assert_eq!(
        store
            .retained_report(released.report().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::OutcomeUnknown
    );
    assert!(
        store
            .duplicate_resolved(
                duplicated.origin.id(),
                duplicated.origin.revision(),
                Timestamp::from_seconds(1004)
            )
            .is_err()
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.drafts().unwrap(), vec![duplicated.draft]);
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, duplicated.origin.id())
            .unwrap()
            .unwrap(),
        duplicated.origin
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__verified_unknown_receipt_updates_report_and_reopens_without_fabricating_success() {
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, SubmissionCommand, SubmissionResolutionAction, SubmissionResult,
        VerifiedUexReport,
    };
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-unknown-verified-{}.db",
        std::process::id()
    ));
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(895),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let unknown = sending
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unknown).unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(11),
        CommodityId(42),
        Timestamp::from_seconds(895),
        unknown.state().request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    let verified = unknown
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proof],
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &verified).unwrap();
    let report = ReportHistoryRepository::retained_report(&mut store, gate.source().id())
        .unwrap()
        .unwrap();
    assert_eq!(
        report.state(),
        ReportState::Submitted,
        "positive complete evidence must resolve the Report without replacing its attempt result"
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let job = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(&job, verified.state());
    assert_eq!(job.attempts(), unknown.state().attempts());
    assert_eq!(job.result(), Some(&SubmissionResult::Unknown));
    assert_eq!(job.verified_row_receipts().next().unwrap().id(), 718);
    assert!(!job.can_duplicate_unknown());
    let fault = Connection::open(&path).unwrap();
    let identity = i64::try_from(job.id().0).unwrap();
    let (revision, original): (i64, String) = fault
        .query_row(
            "SELECT revision,payload FROM submission_jobs WHERE id=?1",
            [identity],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    for (expression, delta) in [
        (
            "json_set(?2,'$.resolution_actions[0].verified.receipts[0][2]','900000')",
            0,
        ),
        (
            "json_insert(?2,'$.resolution_actions[#]',json_extract(?2,'$.resolution_actions[0]'))",
            1,
        ),
        (
            "json_set(?2,'$.resolution_actions[0].verified.receipts[0][1]',0)",
            0,
        ),
    ] {
        let query =
            format!("UPDATE submission_jobs SET revision=?1,payload={expression} WHERE id=?3");
        fault
            .execute(&query, (revision + delta, &original, identity))
            .unwrap();
        assert!(
            SubmissionJobRepository::load(&mut reopened, job.id()).is_err(),
            "forged, redundant or zero receipt identities must fail validated reconstruction"
        );
    }
    fault
        .execute(
            "UPDATE submission_jobs SET revision=?1,payload=?2 WHERE id=?3",
            (revision, original, identity),
        )
        .unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, job.id())
            .unwrap()
            .as_ref(),
        Some(&job)
    );
    drop(fault);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__overlapping_three_row_windows_count_unique_coverage_and_preserve_partial_after_reopen()
{
    use tallyline_adapter_storage::api::SqliteReferenceCache;

    use tallyline_reporting::api::ReportHistoryRepository;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, SubmissionCommand, SubmissionJob, SubmissionResolutionAction,
        SubmissionResult, VerifiedUexReport,
    };
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-overlapping-unknown-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = super::three_rows::gate(&path).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(895),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let unknown = sending
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unknown).unwrap();
    let uncertain_report = store
        .retained_report(released.report().id())
        .unwrap()
        .unwrap();
    assert!(
        uncertain_report
            .apply_delivery(released.job().id(), ReportDelivery::FullAcceptance)
            .is_err()
    );
    let proof = |job: &SubmissionJob, id, commodity| {
        VerifiedUexReport::from_lookup_at(
            id,
            TerminalId(11),
            CommodityId(commodity),
            Timestamp::from_seconds(895),
            job.request(),
            Timestamp::from_seconds(895),
        )
        .unwrap()
    };
    let first_proof = proof(unknown.state(), 718, 42);
    let second_proof = proof(unknown.state(), 719, 43);
    let third_proof = proof(unknown.state(), 720, 44);
    let first = unknown
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![first_proof.clone()],
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &first).unwrap();
    let overlap = first
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![first_proof, second_proof],
                at: Timestamp::from_seconds(1003),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &overlap).unwrap();
    assert_eq!(
        overlap
            .state()
            .verified_row_receipts()
            .map(|receipt| receipt.row().0)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        store
            .retained_report(released.report().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::PartiallyAccepted,
        "two distinct verified rows cannot prove a third row"
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let restored = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(restored, *overlap.state());
    assert_eq!(
        reopened
            .retained_report(released.report().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::PartiallyAccepted
    );
    let complete = restored
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![third_proof],
                at: Timestamp::from_seconds(1004),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut reopened, &complete).unwrap();
    assert_eq!(
        reopened
            .retained_report(released.report().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::Submitted
    );
    assert_eq!(complete.state().verified_row_receipts().count(), 3);
    assert_eq!(complete.state().attempts(), unknown.state().attempts());
    assert_eq!(complete.state().result(), Some(&SubmissionResult::Unknown));
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__late_positive_origin_evidence_blocks_release_of_its_existing_duplicate_draft() {
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, ReceiptDecision, SubmissionCommand, SubmissionResolutionAction,
        VerifiedUexReport,
    };
    use tallyline_workflows::api::{
        ManualReleaseStore, ResolutionDraftStore, SubmissionQueueStore,
    };
    let path = std::env::temp_dir().join(format!(
        "tallyline-unknown-duplicate-conflict-{}.db",
        std::process::id()
    ));
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(895),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let unknown = sending
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unknown).unwrap();
    let decided = unknown
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::NotReceived,
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &decided).unwrap();
    let duplicate = store
        .duplicate_resolved(
            decided.state().id(),
            decided.state().revision(),
            Timestamp::from_seconds(1003),
        )
        .unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(11),
        CommodityId(42),
        Timestamp::from_seconds(895),
        duplicate.origin.request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    let verified = duplicate
        .origin
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proof],
                at: Timestamp::from_seconds(1004),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &verified).unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let renewed = reviewer.submission_gate(
        duplicate.draft,
        &support::snapshot(UexEnvironment::Live).unwrap(),
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::Production,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1302),
    );
    let history = store.queue_jobs().unwrap();
    let renewed =
        tallyline_workflows::api::assess_release_observation_history(&renewed, Some(&history));
    let result = store.release_manual(&renewed);
    assert!(
        result.is_err(),
        "a late positively evidenced origin cannot authorise its duplicate to send: {result:?}"
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__late_positive_origin_evidence_refuses_write_ahead_for_an_already_queued_duplicate() {
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        JobPhase, PayloadHash, ReceiptDecision, SubmissionCommand, SubmissionResolutionAction,
        VerifiedUexReport,
    };
    use tallyline_workflows::api::{
        ManualReleaseStore, ResolutionDraftStore, SubmissionQueueStore,
    };
    let path = std::env::temp_dir().join(format!(
        "tallyline-unknown-queued-duplicate-conflict-{}.db",
        std::process::id()
    ));
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(895),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let unknown = sending
        .state()
        .apply(SubmissionCommand::Recover {
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unknown).unwrap();
    let decided = unknown
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::NotReceived,
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &decided).unwrap();
    let duplicate = store
        .duplicate_resolved(
            decided.state().id(),
            decided.state().revision(),
            Timestamp::from_seconds(1003),
        )
        .unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let renewed = reviewer.submission_gate(
        duplicate.draft,
        &support::snapshot(UexEnvironment::Live).unwrap(),
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::Production,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1302),
    );
    let history = store.queue_jobs().unwrap();
    let renewed =
        tallyline_workflows::api::assess_release_observation_history(&renewed, Some(&history));
    let queued_duplicate = store.release_manual(&renewed).unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(11),
        CommodityId(42),
        Timestamp::from_seconds(895),
        duplicate.origin.request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    let verified = duplicate
        .origin
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proof],
                at: Timestamp::from_seconds(1303),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &verified).unwrap();
    let begin = queued_duplicate
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1304),
            payload_hash: PayloadHash::new([9; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    assert!(
        SubmissionJobRepository::save(&mut store, &begin).is_err(),
        "late evidence must close the atomic write-ahead boundary before any duplicate POST"
    );
    let current = SubmissionJobRepository::load(&mut store, queued_duplicate.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(current.phase(), JobPhase::Queued);
    assert!(current.attempts().is_empty());
    assert_eq!(
        SubmissionJobRepository::load(&mut store, duplicate.origin.id())
            .unwrap()
            .as_ref(),
        Some(verified.state())
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_11__received_decision_preserves_account_fence_and_repeated_check_history_after_reopen()
 {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_reporting::api::ReportHistoryRepository;
    use tallyline_submission::api::model::{
        JobPhase, PayloadHash, ReceiptDecision, SubmissionAccountCheck, SubmissionCommand,
        SubmissionPauseReason, SubmissionResolutionAction, SubmissionResult,
    };
    use tallyline_submission::api::{SubmissionJobRepository, SubmissionServiceFenceRepository};
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-unknown-account-history-{}.db",
        std::process::id()
    ));
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(895),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let unknown = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::AccountRefusal {
                code: None,
                http_status: 403,
                reason: SubmissionPauseReason::KeyCheckUnavailable,
                check: SubmissionAccountCheck::Pending,
                outcome: Box::new(SubmissionResult::Unknown),
            },
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unknown).unwrap();
    let fences = store.service_fences().unwrap();
    assert_eq!(fences.len(), 1);
    let unavailable = unknown
        .state()
        .apply(SubmissionCommand::ResolveAccountCheck(
            SubmissionPauseReason::KeyCheckUnavailable,
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &unavailable).unwrap();
    let received = unavailable
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::Received,
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &received).unwrap();
    assert_eq!(
        store.service_fences().unwrap(),
        fences,
        "a received statement cannot clear account authority fences"
    );
    assert_eq!(received.state().attempts(), unavailable.state().attempts());
    let acknowledged = received.state().apply(SubmissionCommand::Retry).unwrap();
    SubmissionJobRepository::save(&mut store, &acknowledged).unwrap();
    assert!(store.service_fences().unwrap().is_empty());
    let checked = acknowledged
        .state()
        .apply(SubmissionCommand::ResolveAccountCheck(
            SubmissionPauseReason::KeyChecked,
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &checked).unwrap();
    assert!(store.service_fences().unwrap().is_empty());
    assert_eq!(
        store
            .retained_report(released.report().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::Submitted
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let restored = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(&restored, checked.state());
    assert_eq!(restored.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(
        restored.attempts()[0].account_checks(),
        &[
            SubmissionPauseReason::KeyCheckUnavailable,
            SubmissionPauseReason::KeyChecked
        ]
    );
    let evidence = restored.attempts()[0].account_evidence().unwrap();
    assert_eq!(evidence.http_status(), 403);
    assert!(evidence.code().is_none());
    assert_eq!(
        evidence.original_action(),
        SubmissionPauseReason::KeyCheckUnavailable
    );
    assert_eq!(evidence.origin_revision(), Some(3));
    assert!(restored.service_pause_acknowledged());
    assert!(restored.retry().is_err());
    assert!(restored.cancel().is_err());
    assert_eq!(restored.unknown_decision(), Some(ReceiptDecision::Received));
    assert_eq!(
        restored.result().unwrap().final_result(),
        &SubmissionResult::Unknown
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_4_r_nf_11__partial_positive_resolution_is_atomic_durable_and_updates_only_proven_prior()
 {
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, SubmissionCommand, SubmissionResolutionAction, SubmissionResult,
        VerifiedUexReport,
    };
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-partial-positive-{}.db",
        std::process::id()
    ));
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([9; 32]),
            date_added: Timestamp::from_seconds(895),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let partial = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::Partial(vec![]),
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &partial).unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(11),
        CommodityId(42),
        Timestamp::from_seconds(895),
        partial.state().request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    let verified = partial.state().apply(SubmissionCommand::ResolveDelivery(
        SubmissionResolutionAction::Verified {
            receipts: vec![proof],
            at: Timestamp::from_seconds(1002),
        },
    ));
    assert!(
        verified.is_ok(),
        "partial row proof must be accepted without changing transport history: {verified:?}"
    );
    let verified = verified.unwrap();
    let fault = Connection::open(&path).unwrap();
    fault.execute_batch("CREATE TRIGGER fail_partial_resolution BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'test partial rollback'); END;").unwrap();
    assert!(SubmissionJobRepository::save(&mut store, &verified).is_err());
    assert_eq!(
        SubmissionJobRepository::load(&mut store, partial.state().id())
            .unwrap()
            .as_ref(),
        Some(partial.state())
    );
    assert_eq!(
        store
            .retained_report(gate.source().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::PartiallyAccepted
    );
    fault
        .execute_batch("DROP TRIGGER fail_partial_resolution")
        .unwrap();
    drop(fault);
    SubmissionJobRepository::save(&mut store, &verified).unwrap();
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let job = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(&job, verified.state());
    assert_eq!(job.attempts(), partial.state().attempts());
    assert_eq!(job.result(), Some(&SubmissionResult::Partial(vec![])));
    assert_eq!(job.verified_row_receipts().next().unwrap().id(), 718);
    assert_eq!(
        tallyline_workflows::api::accepted_submission_priors(&job)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        reopened
            .retained_report(gate.source().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::Submitted
    );
    assert!(job.retry().is_err());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
