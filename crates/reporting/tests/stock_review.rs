// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parameterised SCU review through the public Reporting application.

mod support;

use support::{Fixed, Repository, key};
use tallyline_reference_data::api::model::{ScuTolerance, UexEnvironment};
use tallyline_reporting::api::ReportingApplication;
use tallyline_reporting::api::model::{Deviation, Report, ReportId};
use tallyline_shared_kernel::api::model::{CommodityId, Percent, Timestamp};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_ui_10_q_manual__stock_review_requires_both_exact_bands_and_preserves_user_values()
 {
    let reference = support::snapshot(UexEnvironment::Live).unwrap();
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_stock(row, Some("16.00000000000000000000000001".parse().unwrap()))
        .unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let tolerance = ScuTolerance::new(Percent::whole(25), "16".parse().unwrap());
    let review = app
        .review_stock(report.id(), &reference, tolerance)
        .unwrap();
    assert_eq!(review.len(), 1);
    assert_eq!(review[0].row(), row);
    assert_eq!(review[0].reference(), Some("0".parse().unwrap()));
    assert_eq!(review[0].deviation(), Some(Deviation::Major));
    assert_eq!(
        review[0].difference().unwrap().to_string(),
        "16.00000000000000000000000001"
    );
    assert_eq!(review[0].percentage_difference().unwrap(), None);
    assert_eq!(review[0].tolerance(), tolerance);
    assert_eq!(review[0].reference_age_millis(), Some(100_000));
    assert!(!review[0].reference_outdated());
    assert_eq!(review[0].display_deviation(), Some(Deviation::Major));
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_nf_5__each_stock_override_can_only_tighten_its_independent_band() {
    let baseline = ScuTolerance::new(Percent::whole(25), "16".parse().unwrap());
    for (percent, minimum, expected_percent, expected_minimum, ignored) in [
        (10, "8", 10, "8", false),
        (5000, "100", 25, "16", true),
        (10, "100", 10, "16", true),
        (5000, "8", 25, "8", true),
        (25, "16", 25, "16", false),
        (0, "0", 0, "0", false),
    ] {
        let tightened = baseline.tighten(Percent::whole(percent), minimum.parse().unwrap());
        assert_eq!(tightened.percent().get(), expected_percent);
        assert_eq!(tightened.minimum().to_string(), expected_minimum);
        assert_eq!(tightened.override_ignored(), ignored);
    }
    assert!(!baseline.override_ignored());
}

fn reference(
    stock: &str,
    side: tallyline_shared_kernel::api::model::TradeSide,
    at: Timestamp,
) -> Result<tallyline_reference_data::api::model::ReferenceSnapshot, Box<dyn std::error::Error>> {
    use tallyline_reference_data::api::model::{
        EndpointData, PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    };
    use tallyline_shared_kernel::api::model::TerminalId;
    let prior = PricePrior::new(
        CommodityId(42),
        side,
        PriorValues {
            price: "1".parse()?,
            stock: stock.parse()?,
            status: std::num::NonZeroU8::new(1),
            container_sizes: vec![],
        },
        PriorProvenance {
            game_version: "4.10.2".to_owned(),
            modified_at: at,
        },
    )?;
    Ok(support::cached_snapshot(
        UexEnvironment::Live,
        vec![EndpointData::TerminalPriors(TerminalPriors::new(
            TerminalId(11),
            vec![prior],
        )?)],
    ))
}
fn entered(value: Option<&str>) -> Result<Report, Box<dyn std::error::Error>> {
    let mut report = Report::manual(
        ReportId(1),
        key()?,
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )?;
    let row = report.add_commodity(CommodityId(42))?;
    report.set_stock(row, value.map(str::parse).transpose()?)?;
    Ok(report)
}
fn assessment(
    prior: &str,
    value: &str,
    tolerance: ScuTolerance,
) -> Result<tallyline_reporting::api::model::StockAssessment, Box<dyn std::error::Error>> {
    use tallyline_shared_kernel::api::model::TradeSide;
    let report = entered(Some(value))?;
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    app.review_stock(
        ReportId(1),
        &reference(prior, TradeSide::Buy, Timestamp::from_seconds(900))?,
        tolerance,
    )?
    .first()
    .copied()
    .ok_or_else(|| "missing fixture assessment".into())
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_val_3_q_manual__missing_stock_has_no_assessment_and_missing_or_other_side_prior_has_no_substitute()
 {
    use tallyline_shared_kernel::api::model::TradeSide;
    let tolerance = ScuTolerance::new(Percent::whole(25), "16".parse().unwrap());
    let mut app = ReportingApplication::new(Repository(vec![entered(None).unwrap()]), Fixed);
    assert!(
        app.review_stock(
            ReportId(1),
            &support::snapshot(UexEnvironment::Live).unwrap(),
            tolerance
        )
        .unwrap()
        .is_empty()
    );
    let report = entered(Some("1744")).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    for snapshot in [
        support::cached_snapshot(UexEnvironment::Live, vec![]),
        reference("0", TradeSide::Sell, Timestamp::from_seconds(900)).unwrap(),
    ] {
        let review = app.review_stock(ReportId(1), &snapshot, tolerance).unwrap()[0];
        assert_eq!(review.deviation(), Some(Deviation::NoReference));
        assert_eq!(review.display_deviation(), Some(Deviation::NoReference));
        assert_eq!(review.reference(), None);
        assert_eq!(review.reference_age_millis(), None);
        assert_eq!(review.difference(), None);
        assert_eq!(review.percentage_difference().unwrap(), None);
        assert_eq!(review.unavailable(), None);
    }
    assert_eq!(app.drafts().unwrap(), vec![report]);
    assert_eq!(
        app.review_stock(
            ReportId(1),
            &support::snapshot(UexEnvironment::Ptu).unwrap(),
            tolerance
        ),
        Err(tallyline_reporting::api::RepositoryError::Invalid)
    );
    assert_eq!(
        app.review_stock(
            ReportId(999),
            &support::snapshot(UexEnvironment::Live).unwrap(),
            tolerance
        ),
        Err(tallyline_reporting::api::RepositoryError::Invalid)
    );
}

struct At(Timestamp);
impl tallyline_shared_kernel::api::Clock for At {
    fn now(&self) -> Timestamp {
        self.0
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual__stale_stock_only_lowers_minor_display_and_future_priors_have_unknown_age() {
    use tallyline_shared_kernel::api::model::TradeSide;
    let tolerance = ScuTolerance::new(Percent::whole(25), "16".parse().unwrap());
    let snapshot = reference("100", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap();
    for (value, now, age, outdated, unlowered, display) in [
        (
            "110",
            899_999,
            None,
            false,
            Deviation::Minor,
            Deviation::Minor,
        ),
        (
            "110",
            900_000,
            Some(0),
            false,
            Deviation::Minor,
            Deviation::Minor,
        ),
        (
            "110",
            1_296_900_000,
            Some(1_296_000_000),
            false,
            Deviation::Minor,
            Deviation::Minor,
        ),
        (
            "110",
            1_296_900_001,
            Some(1_296_000_001),
            true,
            Deviation::Minor,
            Deviation::Equal,
        ),
        (
            "200",
            1_296_900_001,
            Some(1_296_000_001),
            true,
            Deviation::Major,
            Deviation::Major,
        ),
        (
            "100",
            1_296_900_001,
            Some(1_296_000_001),
            true,
            Deviation::Equal,
            Deviation::Equal,
        ),
    ] {
        let report = entered(Some(value)).unwrap();
        let mut app = ReportingApplication::new(
            Repository(vec![report.clone()]),
            At(Timestamp::from_millis(now)),
        );
        let review = app.review_stock(ReportId(1), &snapshot, tolerance).unwrap()[0];
        assert_eq!(review.reference_age_millis(), age);
        assert_eq!(review.reference_outdated(), outdated);
        assert_eq!(review.deviation(), Some(unlowered));
        assert_eq!(review.display_deviation(), Some(display));
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__unrepresentable_stock_comparisons_are_unavailable_without_rounding_inputs() {
    use tallyline_reporting::api::model::AssessmentUnavailable;
    for (prior, value, minimum) in [
        ("1e-28", "79228162514264337593543950335", "16"),
        ("1e-28", "1000000000", "16"),
        ("0", "10000000000000000000000000000", "1e-28"),
    ] {
        let review = assessment(
            prior,
            value,
            ScuTolerance::new(Percent::whole(25), minimum.parse().unwrap()),
        )
        .unwrap();
        assert_eq!(review.deviation(), None);
        assert_eq!(review.display_deviation(), None);
        assert_eq!(
            review.unavailable(),
            Some(AssessmentUnavailable::PrecisionUnavailable)
        );
    }
    let review = assessment(
        "3",
        "2",
        ScuTolerance::new(Percent::whole(25), "0".parse().unwrap()),
    )
    .unwrap();
    assert_eq!(
        review.percentage_difference().unwrap().as_deref(),
        Some("-33.33")
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_3_q_manual__sell_inventory_is_assessed_independently_of_out_of_stock_status() {
    use tallyline_game::api::model::GameEnvironment;
    use tallyline_reporting::api::model::ReportKey;
    use tallyline_shared_kernel::api::model::{TerminalId, TradeSide};
    let key = ReportKey::new(TerminalId(11), TradeSide::Sell, GameEnvironment::Live).unwrap();
    let mut report = Report::manual(ReportId(1), key, Timestamp::from_seconds(900), None).unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_stock(row, Some("1744".parse().unwrap()))
        .unwrap();
    report.set_status(row, std::num::NonZeroU8::new(1)).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let snapshot = reference("1744", TradeSide::Sell, Timestamp::from_seconds(900)).unwrap();
    let review = app
        .review_stock(
            ReportId(1),
            &snapshot,
            ScuTolerance::new(Percent::whole(25), "16".parse().unwrap()),
        )
        .unwrap()[0];
    assert_eq!(review.deviation(), Some(Deviation::Equal));
    assert_eq!(review.reference(), Some("1744".parse().unwrap()));
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[path = "stock_review/boundaries.rs"]
mod boundaries;
#[path = "stock_review/policy.rs"]
mod policy;
#[path = "stock_review/references.rs"]
mod references;

#[path = "stock_review/decisions.rs"]
mod decisions;
