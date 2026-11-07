// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Verified lifecycle observations preserve delivery evidence and revoke local priors durably.

#[path = "submission_lifecycle/accepted_fixture.rs"]
mod accepted_fixture;
mod support;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_val_7_q_manual_r_nf_11__verified_decline_revokes_the_prior_without_rewriting_delivery()
{
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_reference_data::api::AcceptedPriorHistoryStore;
    use tallyline_reference_data::api::model::{LocalPriorProjection, LocalPriorRevocation};
    use tallyline_reporting::api::ReportHistoryRepository;
    use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        LifecycleRefresh, PayloadHash, RowEditability, SubmissionRepositoryError,
        UexLifecycleObservation, UexReportStatus,
    };
    use tallyline_workflows::api::{
        EventOutbox, ManualReleaseStore, OutboxKind, SubmissionLifecycleStore,
    };

    let directory = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
    let path = directory.path().join("lifecycle.db");
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let release = store.release_manual(&gate).unwrap();
    let (accepted, receipt) = accepted_fixture::accepted_and_enriched(
        &mut store,
        release.job(),
        123,
        accepted_fixture::AcceptedAttempt {
            started_at: Timestamp::from_seconds(1000),
            completed_at: Timestamp::from_seconds(1001),
            date_added: Timestamp::from_seconds(900),
            payload_hash: PayloadHash::new([8; 32]),
        },
    )
    .unwrap();
    let original_report = store
        .retained_report(accepted.state().request().header().report)
        .unwrap()
        .unwrap();

    let pending_claim = store
        .claim_lifecycle_read(
            accepted.state(),
            LifecycleRefresh::Automatic,
            Timestamp::from_seconds(1300),
        )
        .unwrap()
        .unwrap();
    let pending = UexLifecycleObservation::new(
        receipt.clone(),
        UexReportStatus::Pending,
        RowEditability::Editable,
        Timestamp::from_seconds(1301),
    );
    let committed_pending = store
        .commit_lifecycle_read(&pending_claim, &[pending])
        .unwrap();
    assert_eq!(
        committed_pending
            .observation(receipt.row())
            .unwrap()
            .status(),
        UexReportStatus::Pending
    );
    assert_eq!(
        store.accepted_history().unwrap().records()[0].projection(),
        LocalPriorProjection::Active
    );

    let declined_claim = store
        .claim_lifecycle_read(
            accepted.state(),
            LifecycleRefresh::Automatic,
            Timestamp::from_seconds(1600),
        )
        .unwrap()
        .unwrap();
    let declined = UexLifecycleObservation::new(
        receipt.clone(),
        UexReportStatus::Declined,
        RowEditability::NotEditable,
        Timestamp::from_seconds(1601),
    );
    let stale_pending = UexLifecycleObservation::new(
        receipt.clone(),
        UexReportStatus::Pending,
        RowEditability::Editable,
        Timestamp::from_seconds(1301),
    );
    assert_eq!(
        store.commit_lifecycle_read(&pending_claim, &[stale_pending]),
        Err(SubmissionRepositoryError::Conflict)
    );
    let fault = rusqlite::Connection::open(&path).unwrap();
    fault.execute_batch("CREATE TRIGGER ignore_lifecycle_event BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission_lifecycle' BEGIN SELECT RAISE(IGNORE); END;").unwrap();
    assert_eq!(
        store.commit_lifecycle_read(&declined_claim, std::slice::from_ref(&declined)),
        Err(SubmissionRepositoryError::Conflict)
    );
    assert_eq!(
        store
            .load_lifecycle(accepted.state().id())
            .unwrap()
            .unwrap()
            .observation(receipt.row())
            .unwrap()
            .status(),
        UexReportStatus::Pending
    );
    assert_eq!(
        store.accepted_history().unwrap().records()[0].projection(),
        LocalPriorProjection::Active
    );
    fault
        .execute_batch("DROP TRIGGER ignore_lifecycle_event")
        .unwrap();
    drop(fault);
    let committed = store
        .commit_lifecycle_read(&declined_claim, std::slice::from_ref(&declined))
        .unwrap();
    assert_eq!(
        store
            .commit_lifecycle_read(&declined_claim, &[declined])
            .unwrap(),
        committed
    );
    drop(store);

    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        reopened.load_lifecycle(accepted.state().id()).unwrap(),
        Some(committed.clone())
    );
    let events: Vec<_> = reopened
        .pending_events()
        .unwrap()
        .into_iter()
        .filter(|event| event.kind() == OutboxKind::UexReportStatusChanged)
        .collect();
    assert_eq!(events.len(), 2);
    assert_ne!(events[0].id(), events[1].id());
    assert_eq!(events[1].id().aggregate_id(), accepted.state().id().0);
    assert_eq!(events[1].id().version(), committed.revision());
    assert_eq!(
        events[1].id().source(),
        tallyline_shared_kernel::api::model::EventSource::SubmissionLifecycle
    );
    let observation = committed.observation(receipt.row()).unwrap();
    assert_eq!(observation.receipt(), &receipt);
    assert_eq!(observation.status(), UexReportStatus::Declined);
    assert_eq!(observation.editability(), RowEditability::NotEditable);
    assert_eq!(observation.checked_at(), Timestamp::from_seconds(1601));
    let history = reopened.accepted_history().unwrap();
    assert_eq!(history.records().len(), 1);
    assert_eq!(
        history.records()[0].projection(),
        LocalPriorProjection::Revoked(LocalPriorRevocation::Declined)
    );
    assert_eq!(
        history.records()[0].prior().context().submission,
        accepted.state().id()
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, accepted.state().id())
            .unwrap()
            .as_ref(),
        Some(accepted.state())
    );
    assert_eq!(
        reopened.retained_report(original_report.id()).unwrap(),
        Some(original_report)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_val_7_q_manual__successful_test_submissions_have_neither_lifecycle_nor_local_prior() {
    use tallyline_reference_data::api::AcceptedPriorHistoryStore;
    use tallyline_shared_kernel::api::model::Timestamp;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        LifecycleRefresh, PayloadHash, SubmissionCommand, SubmissionRepositoryError,
        SubmissionResult,
    };
    use tallyline_workflows::api::{ManualReleaseStore, SubmissionLifecycleStore};
    let directory = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
    let (mut store, gate) = support::stored_gate(&directory.path().join("dry-run.db")).unwrap();
    let release = store.release_manual(&gate).unwrap();
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([3; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let succeeded = sending
        .state()
        .apply(SubmissionCommand::Complete {
            at: Timestamp::from_seconds(1001),
            result: SubmissionResult::DryRunSucceeded,
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &succeeded).unwrap();
    assert_eq!(store.load_lifecycle(succeeded.state().id()).unwrap(), None);
    assert_eq!(
        store.claim_lifecycle_read(
            succeeded.state(),
            LifecycleRefresh::Manual,
            Timestamp::from_seconds(1001)
        ),
        Err(SubmissionRepositoryError::Invalid)
    );
    assert!(store.accepted_history().unwrap().records().is_empty());
}

#[path = "submission_lifecycle/verified_sources.rs"]
mod verified_sources;

#[path = "submission_lifecycle/batch_admission.rs"]
mod batch_admission;

#[path = "submission_lifecycle/batch_budget.rs"]
mod batch_budget;

#[path = "submission_lifecycle/source_evolution.rs"]
mod source_evolution;
