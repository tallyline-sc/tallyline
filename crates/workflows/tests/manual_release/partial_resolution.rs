// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit row decisions through public release and real SQLite history.

use super::support;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    PayloadHash, ReceiptDecision, RowFailureKind, SubmissionCommand, SubmissionResolutionAction,
    SubmissionResult,
};
use tallyline_workflows::api::ManualReleaseStore;

struct TestDirectory(std::path::PathBuf);
impl TestDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tallyline-row-decision-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        assert!(
            std::fs::remove_dir_all(&self.0).is_ok(),
            "row-decision fixture cleanup failed"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_nf_11__explicit_not_received_row_survives_restart_without_changing_transport_history()
{
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("rows.db");
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
    let row = partial.state().request().rows()[0].id();
    let decision = partial.state().apply(SubmissionCommand::ResolveDelivery(
        SubmissionResolutionAction::RowDecision {
            row,
            decision: ReceiptDecision::NotReceived,
            at: Timestamp::from_seconds(1002),
        },
    ));
    assert!(
        decision.is_ok(),
        "explicit per-row resolution must be supported: {decision:?}"
    );
    let decision = decision.unwrap();
    SubmissionJobRepository::save(&mut store, &decision).unwrap();
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let job = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(&job, decision.state());
    assert_eq!(job.attempts(), partial.state().attempts());
    assert_eq!(job.result(), partial.state().result());
    assert!(job.verified_row_receipts().next().is_none());
    assert!(job.retry().is_err());
    assert_eq!(job.resolution_actions().len(), 1);
    use tallyline_submission::api::model::{
        JobId, ScheduleDecision, SubmissionJob, SubmissionRequest,
    };
    use tallyline_submission::api::{ObservationOrder, SubmissionSchedule};
    let mut candidate_header = job.request().header().clone();
    candidate_header.report = tallyline_shared_kernel::api::model::ReportId(999);
    candidate_header.observed_at = Timestamp::from_seconds(899);
    let candidate = SubmissionJob::queued(
        JobId(999),
        SubmissionRequest::new(candidate_header, job.request().rows().to_vec()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        SubmissionSchedule::assess(
            &candidate,
            std::slice::from_ref(&job),
            Timestamp::from_seconds(1002)
        ),
        ScheduleDecision::Ready,
        "a resolved not-accepted row must lose only its own provisional hold"
    );
    assert!(
        ObservationOrder::assess(candidate.request(), std::slice::from_ref(&job)).is_empty(),
        "a row resolved not accepted cannot claim a newer accepted observation"
    );
    use tallyline_reporting::api::ReportRepository;
    use tallyline_workflows::api::ResolutionDraftStore;
    let fault = rusqlite::Connection::open(&path).unwrap();
    fault.execute_batch("CREATE TRIGGER fail_partial_copy BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'test partial copy rollback'); END;").unwrap();
    assert!(
        reopened
            .duplicate_resolved(job.id(), job.revision(), Timestamp::from_seconds(1003))
            .is_err()
    );
    assert!(reopened.drafts().unwrap().is_empty());
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, job.id())
            .unwrap()
            .as_ref(),
        Some(&job)
    );
    fault
        .execute_batch("DROP TRIGGER fail_partial_copy")
        .unwrap();
    drop(fault);
    let copied =
        reopened.duplicate_resolved(job.id(), job.revision(), Timestamp::from_seconds(1003));
    assert!(
        copied.is_ok(),
        "resolved not-accepted partial rows must permit an explicit separate Draft: {copied:?}"
    );
    let copied = copied.unwrap();
    let duplicate_id = copied.draft.id();
    assert_ne!(copied.draft.id(), gate.source().id());
    assert_eq!(copied.draft.rows().len(), 1);
    assert_eq!(
        copied.draft.rows()[0].commodity(),
        gate.source().rows()[0].commodity()
    );
    assert_eq!(
        copied.draft.rows()[0].price(),
        gate.source().rows()[0].price()
    );
    assert_eq!(
        copied.draft.rows()[0].stock(),
        gate.source().rows()[0].stock()
    );
    assert_eq!(
        copied.draft.rows()[0].status(),
        gate.source().rows()[0].status()
    );
    assert_eq!(copied.draft.observed_at(), gate.source().observed_at());
    assert_eq!(copied.draft.key(), gate.source().key());
    assert_eq!(copied.draft.game_version(), gate.source().game_version());
    assert!(copied.draft.release_binding().is_none());
    assert_eq!(copied.origin.attempts(), job.attempts());
    assert_eq!(copied.origin.result(), job.result());
    assert!(
        reopened
            .duplicate_resolved(
                copied.origin.id(),
                copied.origin.revision(),
                Timestamp::from_seconds(1004)
            )
            .is_err()
    );
    drop(reopened);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.drafts().unwrap(), vec![copied.draft]);
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, job.id()).unwrap(),
        Some(copied.origin)
    );
    use tallyline_shared_kernel::api::model::{CommodityId, TerminalId};
    use tallyline_submission::api::SubmissionEvidence;
    use tallyline_submission::api::model::VerifiedUexReport;
    let origin = SubmissionJobRepository::load(&mut reopened, job.id())
        .unwrap()
        .unwrap();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(11),
        CommodityId(42),
        Timestamp::from_seconds(895),
        origin.request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    let late = origin
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proof],
                at: Timestamp::from_seconds(1004),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut reopened, &late).unwrap();
    assert_eq!(late.state().resolution_actions().len(), 3);
    assert_eq!(late.state().attempts(), job.attempts());
    assert_eq!(late.state().result(), job.result());
    assert_eq!(
        SubmissionEvidence::resolved_duplicate_origin(
            duplicate_id,
            std::slice::from_ref(late.state())
        ),
        Some(job.id())
    );
    drop(reopened);
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_val_7_r_nf_11__original_refusal_proof_commits_with_first_partial_outcome_and_reopens()
{
    use tallyline_reporting::api::ReportHistoryRepository;
    use tallyline_reporting::api::model::ReportState;
    use tallyline_shared_kernel::api::model::{CommodityId, TerminalId};
    use tallyline_submission::api::model::{UexErrorCode, VerifiedUexReport};
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("original-proof.db");
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
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(11),
        CommodityId(42),
        Timestamp::from_seconds(895),
        sending.state().request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    let partial = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::PartialError {
                code: UexErrorCode::classified(
                    "database_error".to_owned(),
                    RowFailureKind::Database,
                )
                .unwrap(),
                row: None,
                source: tallyline_submission::api::model::RowFailureSource::Unmapped,
                receipts: vec![proof],
            },
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    let saved = SubmissionJobRepository::save(&mut store, &partial);
    assert!(
        saved.is_ok(),
        "the first partial outcome must commit its already verified original proof and Report together: {saved:?}"
    );
    assert_eq!(
        store
            .retained_report(gate.source().id())
            .unwrap()
            .unwrap()
            .state(),
        ReportState::Submitted
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let job = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(&job, partial.state());
    assert_eq!(
        job.verified_row_receipts()
            .map(|proof| (proof.row().0, proof.id(), proof.date_added().seconds()))
            .collect::<Vec<_>>(),
        vec![(1, 718, 895)]
    );
    assert!(job.resolution_actions().is_empty());
    assert_eq!(
        tallyline_workflows::api::accepted_submission_priors(&job)
            .unwrap()
            .len(),
        1
    );
    assert!(job.retry().is_err());
    drop(reopened);
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_4_r_sub_13_r_val_7__mixed_partial_copy_excludes_accepted_and_unmapped_rows_and_rejects_conflicting_proof()
 {
    use tallyline_reporting::api::model::ReportState;
    use tallyline_reporting::api::{ReportHistoryRepository, ReportRepository};
    use tallyline_shared_kernel::api::model::{CommodityId, RowId, TerminalId};
    use tallyline_submission::api::model::{
        RowFailureSource, SubmissionRepositoryError, UexErrorCode, VerifiedUexReport,
    };
    use tallyline_workflows::api::ResolutionDraftStore;
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("mixed.db");
    let (mut store, gate) =
        super::three_rows::gate_with_affinity(&path, Some("75".parse().unwrap())).unwrap();
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
            result: SubmissionResult::PartialError {
                code: UexErrorCode::classified(
                    "invalid_status_buy".to_owned(),
                    RowFailureKind::InvalidBuyStatus,
                )
                .unwrap(),
                row: Some(1),
                source: RowFailureSource::Index,
                receipts: vec![],
            },
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &partial).unwrap();
    assert!(
        store
            .duplicate_resolved(
                partial.state().id(),
                partial.state().revision(),
                Timestamp::from_seconds(1002)
            )
            .is_err(),
        "an unmapped prefix must prevent copying even known not-sent tail rows"
    );
    let conflicting = VerifiedUexReport::from_lookup_at(
        719,
        TerminalId(11),
        CommodityId(43),
        Timestamp::from_seconds(895),
        partial.state().request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    assert!(
        partial
            .state()
            .resolve_delivery(SubmissionResolutionAction::Verified {
                receipts: vec![conflicting],
                at: Timestamp::from_seconds(1002)
            })
            .is_err(),
        "positive evidence contradicting the named not-stored row must fail closed"
    );
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        TerminalId(11),
        CommodityId(42),
        Timestamp::from_seconds(895),
        partial.state().request(),
        Timestamp::from_seconds(895),
    )
    .unwrap();
    let resolved = partial
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proof],
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &resolved).unwrap();
    assert!(matches!(
        store.duplicate_resolved(
            resolved.state().id(),
            partial.state().revision(),
            Timestamp::from_seconds(1003)
        ),
        Err(SubmissionRepositoryError::Conflict)
    ));
    let copied = store
        .duplicate_resolved(
            resolved.state().id(),
            resolved.state().revision(),
            Timestamp::from_seconds(1003),
        )
        .unwrap();
    assert_eq!(
        copied
            .draft
            .rows()
            .iter()
            .map(|row| row.commodity().0)
            .collect::<Vec<_>>(),
        vec![43, 44]
    );
    assert!(
        copied
            .draft
            .rows()
            .iter()
            .all(|row| row.price().unwrap().to_string() == "123.45")
    );
    assert_eq!(copied.draft.faction_affinity().unwrap().value(), 75);
    assert_eq!(copied.draft.observed_at(), Timestamp::from_seconds(900));
    assert_eq!(copied.draft.game_version(), Some("4.10.2"));
    assert!(copied.draft.release_binding().is_none());
    assert_eq!(
        tallyline_submission::api::SubmissionEvidence::resolved_duplicate_origin(
            copied.draft.id(),
            std::slice::from_ref(&copied.origin)
        ),
        None,
        "a verified accepted row outside the copied subset cannot close that subset's release"
    );
    assert_eq!(copied.origin.attempts(), partial.state().attempts());
    assert_eq!(copied.origin.result(), partial.state().result());
    assert_eq!(
        tallyline_workflows::api::accepted_submission_priors(&copied.origin)
            .unwrap()
            .iter()
            .map(|prior| prior.commodity().0)
            .collect::<Vec<_>>(),
        vec![42]
    );
    assert!(copied.origin.resolution_actions().iter().any(|action|matches!(action,SubmissionResolutionAction::RowsDuplicated{report,rows,..} if *report==copied.draft.id() && rows==&vec![RowId(2),RowId(3)])));
    let source = store.retained_report(gate.source().id()).unwrap().unwrap();
    assert_eq!(source.state(), ReportState::PartiallyAccepted);
    assert_eq!(source.rows().len(), 3);
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.drafts().unwrap(), vec![copied.draft]);
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, copied.origin.id()).unwrap(),
        Some(copied.origin)
    );
    drop(reopened);
}

#[path = "row_failure_semantics.rs"]
mod row_failure_semantics;
