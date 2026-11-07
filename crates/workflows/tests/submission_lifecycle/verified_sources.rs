// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Positive unknown-delivery proofs retain lifecycle authority without rewriting delivery.

use super::support;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9_r_val_7_q_manual_r_nf_11__verified_unknown_row_expires_durably_without_replacing_its_attempt()
 {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_reference_data::api::AcceptedPriorHistoryStore;
    use tallyline_reference_data::api::model::{LocalPriorProjection, LocalPriorRevocation};
    use tallyline_reporting::api::ReportHistoryRepository;
    use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        LifecycleRefresh, PayloadHash, RowEditability, SubmissionCommand,
        SubmissionRepositoryError, SubmissionResolutionAction, SubmissionResult,
        UexLifecycleObservation, UexReportStatus, VerifiedUexReport,
    };
    use tallyline_workflows::api::{ManualReleaseStore, SubmissionLifecycleStore};
    let directory = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
    let path = directory.path().join("verified-unknown.db");
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let release = store.release_manual(&gate).unwrap();
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([9; 32]),
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
    assert_eq!(
        store.claim_lifecycle_read(
            unknown.state(),
            LifecycleRefresh::Manual,
            Timestamp::from_seconds(1001)
        ),
        Err(SubmissionRepositoryError::Invalid)
    );
    let request = unknown.state().request();
    let proof = VerifiedUexReport::from_lookup_at(
        718,
        request.header().terminal,
        request.rows()[0].commodity(),
        Timestamp::from_seconds(900),
        request,
        Timestamp::from_seconds(900),
    )
    .unwrap();
    let verified = unknown
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proof.clone()],
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &verified).unwrap();
    let original_report = store.retained_report(gate.source().id()).unwrap().unwrap();
    let claim = store
        .claim_lifecycle_read(
            verified.state(),
            LifecycleRefresh::Manual,
            Timestamp::from_seconds(1002),
        )
        .unwrap()
        .unwrap();
    let observation = UexLifecycleObservation::new(
        proof,
        UexReportStatus::Expired,
        RowEditability::NotEditable,
        Timestamp::from_seconds(1003),
    );
    let committed = store.commit_lifecycle_read(&claim, &[observation]).unwrap();
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        reopened.load_lifecycle(verified.state().id()).unwrap(),
        Some(committed)
    );
    let job = SubmissionJobRepository::load(&mut reopened, verified.state().id())
        .unwrap()
        .unwrap();
    assert_eq!(&job, verified.state());
    assert_eq!(job.result(), Some(&SubmissionResult::Unknown));
    assert_eq!(job.attempts(), unknown.state().attempts());
    assert_eq!(
        reopened.retained_report(original_report.id()).unwrap(),
        Some(original_report)
    );
    assert_eq!(
        reopened.accepted_history().unwrap().records()[0].projection(),
        LocalPriorProjection::Revoked(LocalPriorRevocation::Expired)
    );
}
