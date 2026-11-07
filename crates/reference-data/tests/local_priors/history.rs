// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Environment refresh and verified revocation preserve accepted evidence.

use super::{local, values};
use std::sync::Arc;
use tallyline_reference_data::api::model::AcceptedLocalPrior;
use tallyline_shared_kernel::api::model::{CommodityId, SubmissionJobId, UexEnvironment};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_api_6__refresh_discards_active_local_reference_and_replayed_acceptance_cannot_revive_it()
 {
    use tallyline_reference_data::api::model::{
        AcceptedPriorHistory, LocalPriorProjection, LocalPriorRevocation,
    };
    let live = local(UexEnvironment::Live).unwrap();
    let ptu = local(UexEnvironment::Ptu).unwrap();
    let mut context = ptu.context().clone();
    context.submission = SubmissionJobId(8);
    let ptu =
        AcceptedLocalPrior::new(Arc::new(context), ptu.commodity(), ptu.values().clone()).unwrap();
    let history = AcceptedPriorHistory::default()
        .accepted(&[live.clone(), ptu.clone()])
        .unwrap();
    assert_eq!(history.records().len(), 2);
    assert_eq!(history.active(UexEnvironment::Live), vec![live.clone()]);
    assert!(history.active(UexEnvironment::Ptu).is_empty());
    let mut context = live.context().clone();
    context.submission = SubmissionJobId(9);
    let withdrawn =
        AcceptedLocalPrior::new(Arc::new(context), CommodityId(8), values("140").unwrap()).unwrap();
    let history = history
        .accepted(std::slice::from_ref(&withdrawn))
        .unwrap()
        .revoked(
            SubmissionJobId(9),
            CommodityId(8),
            LocalPriorRevocation::Withdrawn,
        )
        .unwrap();
    let ptu_refreshed = history.refreshed(UexEnvironment::Ptu);
    assert_eq!(
        ptu_refreshed.active(UexEnvironment::Live),
        vec![live.clone()]
    );
    let refreshed = ptu_refreshed.refreshed(UexEnvironment::Live);
    assert_eq!(
        refreshed
            .records()
            .iter()
            .map(|record| (record.prior(), record.projection()))
            .collect::<Vec<_>>(),
        vec![
            (&live, LocalPriorProjection::Refreshed),
            (&ptu, LocalPriorProjection::Refreshed),
            (
                &withdrawn,
                LocalPriorProjection::Revoked(LocalPriorRevocation::Withdrawn)
            )
        ]
    );
    assert!(
        refreshed
            .revoked(
                SubmissionJobId(9),
                CommodityId(8),
                LocalPriorRevocation::Declined
            )
            .is_err()
    );
    assert_eq!(refreshed.records().len(), 3);
    assert!(refreshed.active(UexEnvironment::Live).is_empty());
    let replayed = refreshed.accepted(&[live, ptu]).unwrap();
    assert_eq!(replayed, refreshed);
    assert!(replayed.active(UexEnvironment::Live).is_empty());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7__revoking_a_local_update_restores_only_unreplaced_accepted_history() {
    use tallyline_reference_data::api::model::{AcceptedPriorHistory, LocalPriorRevocation};
    let first = local(UexEnvironment::Live).unwrap();
    let mut context = first.context().clone();
    context.submission = SubmissionJobId(8);
    let second =
        AcceptedLocalPrior::new(Arc::new(context), CommodityId(7), values("130").unwrap()).unwrap();
    let history = AcceptedPriorHistory::default()
        .accepted(&[first.clone(), second.clone()])
        .unwrap();
    assert_eq!(history.active(UexEnvironment::Live), vec![second.clone()]);
    let reverted = history
        .revoked(
            SubmissionJobId(8),
            CommodityId(7),
            LocalPriorRevocation::Withdrawn,
        )
        .unwrap();
    assert_eq!(reverted.active(UexEnvironment::Live), vec![first]);
    assert_eq!(reverted.records().len(), 2);
    let refreshed = history.refreshed(UexEnvironment::Live);
    let reverted = refreshed
        .revoked(
            SubmissionJobId(8),
            CommodityId(7),
            LocalPriorRevocation::Declined,
        )
        .unwrap();
    assert!(reverted.active(UexEnvironment::Live).is_empty());
    assert_eq!(reverted.accepted(&[second]).unwrap(), reverted);
    assert_eq!(
        reverted
            .revoked(
                SubmissionJobId(8),
                CommodityId(7),
                LocalPriorRevocation::Declined
            )
            .unwrap(),
        reverted
    );
    assert!(
        reverted
            .revoked(
                SubmissionJobId(99),
                CommodityId(7),
                LocalPriorRevocation::Expired
            )
            .is_err()
    );
}
