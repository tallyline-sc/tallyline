// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable submissions restore only consistent multi-commodity provenance.

use super::{local, values};
use std::sync::Arc;
use tallyline_reference_data::api::model::AcceptedLocalPrior;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, UexEnvironment};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_sub_8__accepted_history_refuses_conflicting_snapshots_for_one_submission() {
    use tallyline_reference_data::api::model::AcceptedPriorHistory;
    let first = local(UexEnvironment::Live).unwrap();
    let mut context = first.context().clone();
    context.terminal = TerminalId(90);
    let conflicting =
        AcceptedLocalPrior::new(Arc::new(context), CommodityId(8), values("130").unwrap()).unwrap();
    let history = AcceptedPriorHistory::default()
        .accepted(std::slice::from_ref(&first))
        .unwrap();
    assert!(history.accepted(&[conflicting]).is_err());
    assert_eq!(history.active(UexEnvironment::Live), vec![first.clone()]);
    let second = AcceptedLocalPrior::new(
        Arc::new(first.context().clone()),
        CommodityId(8),
        values("130").unwrap(),
    )
    .unwrap();
    let next = history.accepted(std::slice::from_ref(&second)).unwrap();
    assert_eq!(next.records().len(), 2);
    assert_eq!(next.accepted(&[first, second]).unwrap(), next);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_nf_11__accepted_history_restore_preserves_projection_and_refuses_duplicate_evidence() {
    use tallyline_reference_data::api::model::{
        AcceptedPriorHistory, LocalPriorProjection, StoredAcceptedPriorRecord,
    };
    let prior = local(UexEnvironment::Live).unwrap();
    let stored = StoredAcceptedPriorRecord {
        prior: prior.clone(),
        projection: LocalPriorProjection::Refreshed,
    };
    let restored = AcceptedPriorHistory::reconstitute(vec![stored.clone()]).unwrap();
    assert_eq!(restored.records().len(), 1);
    assert_eq!(restored.records()[0].prior(), &prior);
    assert_eq!(
        restored.records()[0].projection(),
        LocalPriorProjection::Refreshed
    );
    assert!(restored.active(UexEnvironment::Live).is_empty());
    assert!(
        restored
            .accepted(&[prior])
            .unwrap()
            .active(UexEnvironment::Live)
            .is_empty()
    );
    let second = AcceptedLocalPrior::new(
        Arc::new(stored.prior.context().clone()),
        CommodityId(8),
        values("130").unwrap(),
    )
    .unwrap();
    let second_stored = StoredAcceptedPriorRecord {
        prior: second.clone(),
        projection: LocalPriorProjection::Active,
    };
    let restored = AcceptedPriorHistory::reconstitute(vec![stored.clone(), second_stored]).unwrap();
    assert_eq!(
        restored
            .records()
            .iter()
            .map(|record| (record.prior(), record.projection()))
            .collect::<Vec<_>>(),
        vec![
            (&stored.prior, LocalPriorProjection::Refreshed),
            (&second, LocalPriorProjection::Active)
        ]
    );
    assert_eq!(restored.active(UexEnvironment::Live), vec![second.clone()]);
    let mut conflicting_context = second.context().clone();
    conflicting_context.terminal = TerminalId(90);
    let conflicting = AcceptedLocalPrior::new(
        Arc::new(conflicting_context),
        CommodityId(8),
        values("130").unwrap(),
    )
    .unwrap();
    assert!(
        AcceptedPriorHistory::reconstitute(vec![
            stored.clone(),
            StoredAcceptedPriorRecord {
                prior: conflicting,
                projection: LocalPriorProjection::Active,
            },
        ])
        .is_err()
    );
    assert!(AcceptedPriorHistory::reconstitute(vec![stored.clone(), stored]).is_err());
}
