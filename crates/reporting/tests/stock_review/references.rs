// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Quantity references never borrow another terminal, commodity or price average.

use super::*;
use tallyline_reference_data::api::model::{
    Commodity, CommodityAverages, EndpointData, PricePrior, TerminalPriors,
};
use tallyline_shared_kernel::api::model::{TerminalId, TradeSide};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_val_2a_q_manual__quantity_priors_require_exact_context_and_never_fall_back_to_a_price_average()
 {
    let original = reference("1744", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap();
    let prior = original.terminal_priors(TerminalId(11)).unwrap().rows()[0].clone();
    let other = PricePrior::new(
        CommodityId(99),
        TradeSide::Buy,
        prior.values().clone(),
        prior.provenance().clone(),
    )
    .unwrap();
    let entries = [
        EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(22), vec![prior]).unwrap()),
        EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), vec![other]).unwrap()),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(42),
            "Gold",
            "GOLD",
            true,
            CommodityAverages::new(Some("1744".parse().unwrap()), Some("1744".parse().unwrap())),
        )]),
    ];
    for entry in entries {
        let report = entered(Some("1744")).unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let snapshot = support::cached_snapshot(UexEnvironment::Live, vec![entry]);
        let review = app
            .review_stock(
                ReportId(1),
                &snapshot,
                ScuTolerance::new(Percent::whole(25), "16".parse().unwrap()),
            )
            .unwrap()[0];
        assert_eq!(review.reference(), None);
        assert_eq!(review.deviation(), Some(Deviation::NoReference));
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}
