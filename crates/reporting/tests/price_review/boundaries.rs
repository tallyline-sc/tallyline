// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public manual price review contracts.

use super::price_reference;
use crate::support::Fixed;
use crate::support::Repository;
use crate::support::key;
use tallyline_reporting::api::ReportingApplication;
use tallyline_reporting::api::model::AssessmentUnavailable;
use tallyline_reporting::api::model::Deviation;
use tallyline_reporting::api::model::Report;
use tallyline_reporting::api::model::ReportId;
use tallyline_shared_kernel::api::model::CommodityId;
use tallyline_shared_kernel::api::model::Timestamp;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_ui_10_q_manual_r_ui_11_q_manual__price_review_uses_uex_percent_and_an_exact_delta_without_changing_values()
 {
    let reference = price_reference("123.45", 25).unwrap();
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("185.175".parse().unwrap()))
        .unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let review = app.review_prices(report.id(), &reference).unwrap();
    assert_eq!(review.len(), 1);
    assert_eq!(review[0].row(), row);
    assert_eq!(review[0].reference(), Some("123.45".parse().unwrap()));
    assert_eq!(review[0].difference().unwrap().to_string(), "61.725");
    assert_eq!(
        review[0].deviation(),
        Some(tallyline_reporting::api::model::Deviation::Major)
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_ui_10_q_manual__price_boundaries_are_strict_in_both_directions_without_rounding()
 {
    let reference = price_reference("123.45", 25).unwrap();
    for (value, difference, expected) in [
        ("154.3125", "30.8625", Deviation::Minor),
        (
            "154.31250000000000000000000001",
            "30.86250000000000000000000001",
            Deviation::Major,
        ),
        ("92.5875", "-30.8625", Deviation::Minor),
        (
            "92.58749999999999999999999999",
            "-30.86250000000000000000000001",
            Deviation::Major,
        ),
        ("123.45", "0", Deviation::Equal),
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
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let assessment = app.review_prices(report.id(), &reference).unwrap()[0];
        assert_eq!(assessment.deviation(), Some(expected), "{value}");
        assert_eq!(assessment.difference().unwrap().to_string(), difference);
        assert_eq!(assessment.unavailable(), None);
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__unrepresentable_price_comparisons_remain_unassessed_without_narrowing_user_inputs()
 {
    for value in ["79228162514264337593543950335", "1000000000"] {
        let reference = price_reference("1e-28", 25).unwrap();
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
        let assessment = app.review_prices(report.id(), &reference).unwrap()[0];
        assert_eq!(assessment.deviation(), None);
        assert_eq!(
            assessment.unavailable(),
            Some(AssessmentUnavailable::PrecisionUnavailable)
        );
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual_r_val_2_q_manual__display_percentage_is_explicitly_rounded_and_never_decides_the_tolerance_boundary()
 {
    let reference = price_reference("123.45", 25).unwrap();
    for (value, percentage, expected) in [
        ("185.175", "+50.00", Deviation::Major),
        ("61.725", "-50.00", Deviation::Major),
        ("150", "+21.51", Deviation::Minor),
        ("154.31250000000000000000000001", "+25.00", Deviation::Major),
        ("123.45", "0.00", Deviation::Equal),
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
        let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
        let assessment = app.review_prices(ReportId(1), &reference).unwrap()[0];
        assert_eq!(
            assessment.percentage_difference().unwrap().as_deref(),
            Some(percentage)
        );
        assert_eq!(assessment.deviation(), Some(expected));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual__percentage_display_rounds_half_points_away_from_zero_and_carries_to_the_whole_part()
 {
    for (prior, value, expected) in [
        ("20000", "20001", "+0.01"),
        ("20000", "19999", "-0.01"),
        ("100000", "100999.9", "+1.00"),
        (
            "1",
            "10000000000000000000000000000",
            "+999999999999999999999999999900.00",
        ),
    ] {
        let reference = price_reference(prior, 25).unwrap();
        let mut report = Report::manual(
            ReportId(1),
            key().unwrap(),
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(CommodityId(42)).unwrap();
        report.set_price(row, Some(value.parse().unwrap())).unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
        let assessment = app.review_prices(ReportId(1), &reference).unwrap()[0];
        assert_eq!(
            assessment.percentage_difference().unwrap().as_deref(),
            Some(expected)
        );
        assert_eq!(assessment.unavailable(), None);
    }
}
