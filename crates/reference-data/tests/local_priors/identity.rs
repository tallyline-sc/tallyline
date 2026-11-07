// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Each terminal, trade side and commodity has an independent accepted value.

use super::{Cache, Time, local, values};
use std::sync::Arc;
use tallyline_reference_data::api::ReferenceApplication;
use tallyline_reference_data::api::model::{AcceptedLocalPrior, PriorReferenceSource};
use tallyline_shared_kernel::api::model::{
    CommodityId, SubmissionJobId, TerminalId, TradeSide, UexEnvironment,
};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7__accepted_values_replace_only_the_same_terminal_side_and_commodity() {
    use tallyline_reference_data::api::model::AcceptedPriorHistory;
    let first = local(UexEnvironment::Live).unwrap();
    let mut rows = vec![first.clone()];
    for (submission, terminal, side, commodity, price) in [
        (8, 90, TradeSide::Sell, 7, "130"),
        (9, 89, TradeSide::Buy, 7, "140"),
        (10, 89, TradeSide::Sell, 8, "150"),
    ] {
        let mut context = first.context().clone();
        context.submission = SubmissionJobId(submission);
        context.terminal = TerminalId(terminal);
        context.side = side;
        rows.push(
            AcceptedLocalPrior::new(
                Arc::new(context),
                CommodityId(commodity),
                values(price).unwrap(),
            )
            .unwrap(),
        );
    }
    let history = AcceptedPriorHistory::default().accepted(&rows).unwrap();
    assert_eq!(history.active(UexEnvironment::Live), rows);
    let mut context = first.context().clone();
    context.submission = SubmissionJobId(11);
    let replacement =
        AcceptedLocalPrior::new(Arc::new(context), CommodityId(7), values("160").unwrap()).unwrap();
    let replaced = history
        .accepted(std::slice::from_ref(&replacement))
        .unwrap();
    assert_eq!(replaced.records().len(), 5);
    assert_eq!(
        replaced.active(UexEnvironment::Live),
        vec![
            rows[1].clone(),
            rows[2].clone(),
            rows[3].clone(),
            replacement.clone()
        ]
    );
    let application = ReferenceApplication::load(Cache(vec![]), Time).unwrap();
    let active = replaced.active(UexEnvironment::Live);
    let snapshot = application
        .snapshot(UexEnvironment::Live)
        .with_local_priors(&active)
        .unwrap();
    for (terminal, side, commodity, price, submission) in [
        (90, TradeSide::Sell, 7, "130", 8),
        (89, TradeSide::Buy, 7, "140", 9),
        (89, TradeSide::Sell, 8, "150", 10),
        (89, TradeSide::Sell, 7, "160", 11),
    ] {
        let selected = snapshot
            .prior_reference(TerminalId(terminal), CommodityId(commodity), side)
            .unwrap();
        assert_eq!(
            (
                selected.values(),
                selected.source(),
                selected.local_source().unwrap().submission
            ),
            (
                &values(price).unwrap(),
                PriorReferenceSource::AcceptedLocal,
                SubmissionJobId(submission)
            )
        );
    }
}
