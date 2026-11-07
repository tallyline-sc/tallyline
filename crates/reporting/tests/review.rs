// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual application behaviour at its public persistence and reference boundaries.

//! Status review behaviour through the public Reporting application.

mod support;

use support::{Fixed, Repository, key, snapshot};
use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::model::{Report, ReportId, ReportKey};
use tallyline_reporting::api::{ReportingApplication, RepositoryError};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_1_q_manual_r_ui_10_q_manual__status_review_marks_two_levels_as_major_without_changing_user_values()
 {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    report.set_stock(row, Some("17".parse().unwrap())).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let review = app
        .review_statuses(report.id(), &snapshot(UexEnvironment::Live).unwrap())
        .unwrap();
    assert_eq!(review.len(), 1);
    assert_eq!(review[0].row(), row);
    assert_eq!(
        review[0].deviation(),
        tallyline_reporting::api::model::Deviation::Major
    );
    assert_eq!(review[0].reference(), std::num::NonZeroU8::new(3));
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_10_q_manual__status_review_distinguishes_equal_minor_missing_and_unsent_fields() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let live = snapshot(UexEnvironment::Live).unwrap();
    assert_eq!(
        app.review_statuses(report.id(), &live).unwrap()[0].deviation(),
        tallyline_reporting::api::model::Deviation::Equal
    );
    report.set_status(row, std::num::NonZeroU8::new(2)).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    assert_eq!(
        app.review_statuses(report.id(), &live).unwrap()[0].deviation(),
        tallyline_reporting::api::model::Deviation::Minor
    );
    let sell = ReportKey::new(TerminalId(11), TradeSide::Sell, GameEnvironment::Live).unwrap();
    report.change_key(sell).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    assert_eq!(
        app.review_statuses(report.id(), &live).unwrap()[0].deviation(),
        tallyline_reporting::api::model::Deviation::NoReference
    );
    report.set_status(row, None).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    assert!(app.review_statuses(ReportId(1), &live).unwrap().is_empty());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual__stale_status_reference_only_lowers_minor_display_and_keeps_unlowered_gate_level()
 {
    struct Later;
    impl Clock for Later {
        fn now(&self) -> Timestamp {
            Timestamp::from_millis(1_296_900_001)
        }
    }
    let reference = snapshot(UexEnvironment::Live).unwrap();
    for (status, level, displayed) in [
        (
            2,
            tallyline_reporting::api::model::Deviation::Minor,
            tallyline_reporting::api::model::Deviation::Equal,
        ),
        (
            5,
            tallyline_reporting::api::model::Deviation::Major,
            tallyline_reporting::api::model::Deviation::Major,
        ),
    ] {
        let mut report = Report::manual(
            ReportId(1),
            key().unwrap(),
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
        let row = report.add_commodity(CommodityId(42)).unwrap();
        report
            .set_status(row, std::num::NonZeroU8::new(status))
            .unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report]), Later);
        let assessment = app.review_statuses(ReportId(1), &reference).unwrap()[0];
        assert_eq!(assessment.reference_age_millis(), Some(1_296_000_001));
        assert!(assessment.reference_outdated());
        assert_eq!(assessment.deviation(), level);
        assert_eq!(assessment.display_deviation(), displayed);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual__status_reference_boundary_is_precise_and_future_dates_have_unknown_age() {
    struct At(Timestamp);
    impl Clock for At {
        fn now(&self) -> Timestamp {
            self.0
        }
    }
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(4)).unwrap();
    for (now, age) in [(1_296_900_000, Some(1_296_000_000)), (899_999, None)] {
        let mut app = ReportingApplication::new(
            Repository(vec![report.clone()]),
            At(Timestamp::from_millis(now)),
        );
        let assessment = app
            .review_statuses(report.id(), &snapshot(UexEnvironment::Live).unwrap())
            .unwrap()[0];
        assert_eq!(assessment.reference_age_millis(), age);
        assert!(!assessment.reference_outdated());
        assert_eq!(
            assessment.display_deviation(),
            tallyline_reporting::api::model::Deviation::Minor
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_4_q_manual__status_review_refuses_missing_drafts_and_mismatched_environment() {
    let report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    assert_eq!(
        app.review_statuses(ReportId(2), &snapshot(UexEnvironment::Live).unwrap()),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.review_statuses(report.id(), &snapshot(UexEnvironment::Ptu).unwrap()),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.review_prices(ReportId(2), &snapshot(UexEnvironment::Live).unwrap()),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.review_prices(report.id(), &snapshot(UexEnvironment::Ptu).unwrap()),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}
