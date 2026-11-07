// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Append-only delivery proofs preserve existing lifecycle evidence and per-row cadence.

use super::support;
#[path = "../manual_release/three_rows.rs"]
mod three_rows;

use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;

struct ManualClock(Timestamp);
impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}
impl ManualClock {
    fn set(&mut self, seconds: u64) {
        self.0 = Timestamp::from_seconds(seconds);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_9_r_val_7_q_manual_r_nf_11__late_verified_rows_keep_old_status_cadence_and_reject_superseded_completion()
 {
    use tallyline_reference_data::api::AcceptedPriorHistoryStore;
    use tallyline_reference_data::api::model::{LocalPriorProjection, LocalPriorRevocation};
    use tallyline_reporting::api::ReportHistoryRepository;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        LifecycleRefresh, PayloadHash, RowEditability, SubmissionCommand,
        SubmissionRepositoryError, SubmissionResolutionAction, SubmissionResult,
        UexLifecycleObservation, UexReportStatus, VerifiedUexReport,
    };
    use tallyline_workflows::api::{ManualReleaseStore, SubmissionLifecycleStore};
    let directory = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
    let path = directory.path().join("source-evolution.db");
    let (mut store, gate) = three_rows::gate(&path).unwrap();
    let release = store.release_manual(&gate).unwrap();
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([14; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let partial = sending
        .state()
        .apply(SubmissionCommand::Complete {
            at: Timestamp::from_seconds(1001),
            result: SubmissionResult::Partial(Vec::new()),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &partial).unwrap();
    let request = partial.state().request();
    let proofs: Vec<_> = request
        .rows()
        .iter()
        .zip([730, 731, 732])
        .map(|(row, id)| {
            VerifiedUexReport::from_lookup_at(
                id,
                request.header().terminal,
                row.commodity(),
                Timestamp::from_seconds(900),
                request,
                Timestamp::from_seconds(900),
            )
            .unwrap()
        })
        .collect();
    let first = partial
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proofs[0].clone()],
                at: Timestamp::from_seconds(1002),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &first).unwrap();
    let mut clock = ManualClock(Timestamp::from_seconds(1300));
    let first_claim = store
        .claim_lifecycle_read(first.state(), LifecycleRefresh::Automatic, clock.now())
        .unwrap()
        .unwrap();
    clock.set(1301);
    store
        .commit_lifecycle_read(
            &first_claim,
            &[UexLifecycleObservation::new(
                proofs[0].clone(),
                UexReportStatus::Pending,
                RowEditability::Editable,
                clock.now(),
            )],
        )
        .unwrap();
    let second = first
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proofs[1].clone()],
                at: Timestamp::from_seconds(1350),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &second).unwrap();
    let loaded = store.load_lifecycle(second.state().id()).unwrap().unwrap();
    assert_eq!(
        loaded.observation(proofs[0].row()).unwrap().checked_at(),
        Timestamp::from_seconds(1301)
    );
    clock.set(1350);
    let second_claim = store
        .claim_lifecycle_read(second.state(), LifecycleRefresh::Automatic, clock.now())
        .unwrap()
        .unwrap();
    assert_eq!(
        second_claim
            .targets()
            .map(VerifiedUexReport::id)
            .collect::<Vec<_>>(),
        vec![731]
    );
    clock.set(1351);
    store
        .commit_lifecycle_read(
            &second_claim,
            &[UexLifecycleObservation::new(
                proofs[1].clone(),
                UexReportStatus::Declined,
                RowEditability::NotEditable,
                clock.now(),
            )],
        )
        .unwrap();
    clock.set(1599);
    assert!(
        store
            .claim_lifecycle_read(second.state(), LifecycleRefresh::Automatic, clock.now())
            .unwrap()
            .is_none()
    );
    clock.set(1600);
    let pending_old_source = store
        .claim_lifecycle_read(second.state(), LifecycleRefresh::Automatic, clock.now())
        .unwrap()
        .unwrap();
    assert_eq!(
        pending_old_source
            .targets()
            .map(VerifiedUexReport::id)
            .collect::<Vec<_>>(),
        vec![730]
    );
    let third = second
        .state()
        .apply(SubmissionCommand::ResolveDelivery(
            SubmissionResolutionAction::Verified {
                receipts: vec![proofs[2].clone()],
                at: Timestamp::from_seconds(1601),
            },
        ))
        .unwrap();
    SubmissionJobRepository::save(&mut store, &third).unwrap();
    let report = store.retained_report(gate.source().id()).unwrap().unwrap();
    clock.set(1602);
    assert_eq!(
        store.commit_lifecycle_read(
            &pending_old_source,
            &[UexLifecycleObservation::new(
                proofs[0].clone(),
                UexReportStatus::Approved,
                RowEditability::Editable,
                clock.now(),
            )]
        ),
        Err(SubmissionRepositoryError::Conflict)
    );
    clock.set(1603);
    let third_claim = store
        .claim_lifecycle_read(third.state(), LifecycleRefresh::Automatic, clock.now())
        .unwrap()
        .unwrap();
    assert_eq!(
        third_claim
            .targets()
            .map(VerifiedUexReport::id)
            .collect::<Vec<_>>(),
        vec![732]
    );
    clock.set(1604);
    store
        .commit_lifecycle_read(
            &third_claim,
            &[UexLifecycleObservation::new(
                proofs[2].clone(),
                UexReportStatus::Expired,
                RowEditability::NotEditable,
                clock.now(),
            )],
        )
        .unwrap();
    let lifecycle = store.load_lifecycle(third.state().id()).unwrap().unwrap();
    assert_eq!(
        lifecycle.observation(proofs[0].row()).unwrap().status(),
        UexReportStatus::Pending
    );
    assert_eq!(
        lifecycle.observation(proofs[0].row()).unwrap().checked_at(),
        Timestamp::from_seconds(1301)
    );
    assert_eq!(
        lifecycle.observation(proofs[1].row()).unwrap().status(),
        UexReportStatus::Declined
    );
    assert_eq!(
        lifecycle.observation(proofs[1].row()).unwrap().checked_at(),
        Timestamp::from_seconds(1351)
    );
    clock.set(1899);
    assert!(
        store
            .claim_lifecycle_read(third.state(), LifecycleRefresh::Automatic, clock.now())
            .unwrap()
            .is_none()
    );
    clock.set(1900);
    let remaining = store
        .claim_lifecycle_read(third.state(), LifecycleRefresh::Automatic, clock.now())
        .unwrap()
        .unwrap();
    assert_eq!(
        remaining
            .targets()
            .map(VerifiedUexReport::id)
            .collect::<Vec<_>>(),
        vec![730]
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut store, third.state().id())
            .unwrap()
            .as_ref(),
        Some(third.state())
    );
    assert_eq!(third.state().attempts(), partial.state().attempts());
    assert_eq!(third.state().result(), partial.state().result());
    assert_eq!(store.retained_report(report.id()).unwrap(), Some(report));
    let mut projections: Vec<_> = store
        .accepted_history()
        .unwrap()
        .records()
        .iter()
        .map(|record| (record.prior().commodity().0, record.projection()))
        .collect();
    projections.sort_by_key(|(commodity, _)| *commodity);
    assert_eq!(
        projections,
        vec![
            (42, LocalPriorProjection::Active),
            (
                43,
                LocalPriorProjection::Revoked(LocalPriorRevocation::Declined)
            ),
            (
                44,
                LocalPriorProjection::Revoked(LocalPriorRevocation::Expired)
            ),
        ]
    );
}
