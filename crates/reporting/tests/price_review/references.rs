// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public manual price review contracts.

use super::price_reference;
use crate::support;
use crate::support::Fixed;
use crate::support::Repository;
use crate::support::key;
use tallyline_reference_data::api::model::EndpointData;
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::ReportingApplication;
use tallyline_reporting::api::model::Deviation;
use tallyline_reporting::api::model::Report;
use tallyline_reporting::api::model::ReportId;
use tallyline_shared_kernel::api::model::CommodityId;
use tallyline_shared_kernel::api::model::TerminalId;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_shared_kernel::api::model::TradeSide;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual__stale_price_review_lowers_only_minor_display_and_retains_reference_age() {
    struct Later;
    impl tallyline_shared_kernel::api::Clock for Later {
        fn now(&self) -> Timestamp {
            Timestamp::from_millis(1_296_900_001)
        }
    }
    let reference = price_reference("123.45", 25).unwrap();
    for (value, deviation, displayed) in [
        ("150", Deviation::Minor, Deviation::Equal),
        ("185.175", Deviation::Major, Deviation::Major),
    ] {
        let mut report = Report::manual(
            ReportId(1),
            key().unwrap(),
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(CommodityId(42)).unwrap();
        report.set_price(row, Some(value.parse().unwrap())).unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report]), Later);
        let assessment = app.review_prices(ReportId(1), &reference).unwrap()[0];
        assert_eq!(assessment.reference_age_millis(), Some(1_296_000_001));
        assert!(assessment.reference_outdated());
        assert_eq!(assessment.deviation(), Some(deviation));
        assert_eq!(assessment.display_deviation(), Some(displayed));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2a_q_manual_r_api_6__missing_terminal_price_uses_only_its_environment_average_with_twice_the_effective_band()
 {
    use tallyline_reference_data::api::model::{Commodity, CommodityAverages};
    use tallyline_reporting::api::model::PriceReferenceSource;
    for (value, expected) in [
        ("120", Deviation::NoReference),
        ("120.00000000000000000000000001", Deviation::Major),
    ] {
        let reference = support::cached_snapshot(
            UexEnvironment::Live,
            vec![EndpointData::Commodities(vec![Commodity::new(
                CommodityId(42),
                "Gold",
                "GOLD",
                true,
                CommodityAverages::new(Some("80".parse().unwrap()), Some("40".parse().unwrap())),
            )])],
        );
        let mut report = Report::manual(
            ReportId(1),
            key().unwrap(),
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(CommodityId(42)).unwrap();
        report.set_price(row, Some(value.parse().unwrap())).unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let assessment = app.review_prices(ReportId(1), &reference).unwrap()[0];
        assert_eq!(assessment.deviation(), Some(expected));
        assert_eq!(assessment.reference(), Some("80".parse().unwrap()));
        assert_eq!(
            assessment.reference_source(),
            Some(PriceReferenceSource::CommodityAverage)
        );
        assert_eq!(assessment.reference_age_millis(), None);
        assert!(!assessment.reference_outdated());
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2a_q_manual_r_api_6__price_review_never_borrows_live_averages_for_ptu_or_the_opposite_side()
 {
    use tallyline_game::api::model::GameEnvironment;
    use tallyline_reference_data::api::model::{Commodity, CommodityAverages};
    use tallyline_reporting::api::model::ReportKey;
    for (environment, channel, side, buy, sell, expected, reference_value) in [
        (
            UexEnvironment::Ptu,
            GameEnvironment::Ptu,
            TradeSide::Buy,
            Some("80"),
            Some("40"),
            Deviation::NoReference,
            None,
        ),
        (
            UexEnvironment::Live,
            GameEnvironment::Live,
            TradeSide::Buy,
            None,
            Some("40"),
            Deviation::NoReference,
            None,
        ),
        (
            UexEnvironment::Live,
            GameEnvironment::Live,
            TradeSide::Buy,
            Some("0"),
            Some("40"),
            Deviation::NoReference,
            None,
        ),
        (
            UexEnvironment::Live,
            GameEnvironment::Live,
            TradeSide::Sell,
            Some("80"),
            Some("40"),
            Deviation::Major,
            Some("40"),
        ),
    ] {
        let reference = support::cached_snapshot(
            environment,
            vec![EndpointData::Commodities(vec![Commodity::new(
                CommodityId(42),
                "Gold",
                "GOLD",
                true,
                CommodityAverages::new(
                    buy.map(|value| value.parse().unwrap()),
                    sell.map(|value| value.parse().unwrap()),
                ),
            )])],
        );
        let key = ReportKey::new(TerminalId(11), side, channel).unwrap();
        let mut report = Report::manual(
            ReportId(1),
            key,
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(CommodityId(42)).unwrap();
        report
            .set_price(row, Some("1000".parse().unwrap()))
            .unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let assessment = app.review_prices(ReportId(1), &reference).unwrap()[0];
        assert_eq!(assessment.deviation(), Some(expected));
        assert_eq!(
            assessment.reference(),
            reference_value.map(|value| value.parse().unwrap())
        );
        assert_eq!(assessment.reference_age_millis(), None);
        assert!(!assessment.reference_outdated());
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual__price_staleness_boundary_is_precise_and_future_reference_age_is_unknown() {
    struct At(Timestamp);
    impl tallyline_shared_kernel::api::Clock for At {
        fn now(&self) -> Timestamp {
            self.0
        }
    }
    let reference = price_reference("123.45", 25).unwrap();
    for (now, age) in [(1_296_900_000, Some(1_296_000_000)), (899_999, None)] {
        let mut report = Report::manual(
            ReportId(1),
            key().unwrap(),
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(CommodityId(42)).unwrap();
        report.set_price(row, Some("150".parse().unwrap())).unwrap();
        let mut app =
            ReportingApplication::new(Repository(vec![report]), At(Timestamp::from_millis(now)));
        let assessment = app.review_prices(ReportId(1), &reference).unwrap()[0];
        assert_eq!(assessment.reference_age_millis(), age);
        assert!(!assessment.reference_outdated());
        assert_eq!(assessment.display_deviation(), Some(Deviation::Minor));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_4_q_manual_r_ui_10_q_manual__price_review_never_borrows_another_commodity_or_side_and_skips_unsent_prices()
 {
    use tallyline_game::api::model::GameEnvironment;
    use tallyline_reporting::api::model::ReportKey;
    let reference = price_reference("123.45", 25).unwrap();
    for (commodity, side) in [
        (CommodityId(43), TradeSide::Buy),
        (CommodityId(42), TradeSide::Sell),
    ] {
        let key = ReportKey::new(TerminalId(11), side, GameEnvironment::Live).unwrap();
        let mut report = Report::manual(
            ReportId(1),
            key,
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(commodity).unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        assert!(
            app.review_prices(report.id(), &reference)
                .unwrap()
                .is_empty()
        );
        report
            .set_price(row, Some("185.175".parse().unwrap()))
            .unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let assessment = app.review_prices(report.id(), &reference).unwrap()[0];
        assert_eq!(assessment.deviation(), Some(Deviation::NoReference));
        assert_eq!(assessment.reference(), None);
        assert_eq!(assessment.reference_source(), None);
        assert_eq!(assessment.difference(), None);
        assert_eq!(assessment.percentage_difference(), Ok(None));
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}
