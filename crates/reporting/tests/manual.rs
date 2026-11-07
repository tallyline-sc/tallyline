// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Manual Draft command contracts.

use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::model::{Report, ReportId, ReportKey};
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_2__manual_draft_has_no_implicitly_observed_rows() {
    let mut report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(100),
        Some("4.10.2"),
    )
    .unwrap();
    report.add_commodity(CommodityId(42)).unwrap();
    let row = report.rows().first().unwrap();
    assert_eq!(row.price(), None);
    assert_eq!(row.stock(), None);
    assert_eq!(row.status(), None);
    assert!(!row.entered());
    assert!(report.add_commodity(CommodityId(42)).is_err());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_2__manual_values_remain_explicit_user_values_when_the_report_key_changes() {
    let mut report = Report::manual(
        ReportId(2),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(100),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    report
        .change_key(ReportKey::new(TerminalId(12), TradeSide::Sell, GameEnvironment::Ptu).unwrap())
        .unwrap();
    let row = report.rows().first().unwrap();
    assert_eq!(row.price().unwrap().to_string(), "123.45");
    assert_eq!(row.stock().unwrap().to_string(), "0");
    assert_eq!(row.status().unwrap().get(), 3);
    assert_eq!(
        row.origin(),
        Some(tallyline_reporting::api::model::FieldOrigin::User)
    );
    assert_eq!(report.game_version(), None);
    assert_eq!(report.observed_at(), Timestamp::from_seconds(100));
    assert!(report.version() >= 5);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2__each_independent_field_is_a_user_observation_and_clearing_it_restores_an_empty_row() {
    use tallyline_reporting::api::model::{FieldOrigin, RowId};
    let mut report = Report::manual(
        ReportId(3),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(100),
        None,
    )
    .unwrap();
    let first = report.add_commodity(CommodityId(42)).unwrap();
    let second = report.add_commodity(CommodityId(43)).unwrap();
    assert_ne!(first, second);
    for field in 0..3 {
        match field {
            0 => report
                .set_price(first, Some("1.25".parse().unwrap()))
                .unwrap(),
            1 => report.set_stock(first, Some("0".parse().unwrap())).unwrap(),
            _ => report
                .set_status(first, std::num::NonZeroU8::new(3))
                .unwrap(),
        }
        assert!(report.rows().first().unwrap().entered());
        assert_eq!(
            report.rows().first().unwrap().origin(),
            Some(FieldOrigin::User)
        );
        assert!(!report.rows().get(1).unwrap().entered());
        report.set_price(first, None).unwrap();
        report.set_stock(first, None).unwrap();
        report.set_status(first, None).unwrap();
        assert!(!report.rows().first().unwrap().entered());
        assert_eq!(report.rows().first().unwrap().origin(), None);
    }
    let saved = report.clone();
    assert!(report.set_price(first, Some("0".parse().unwrap())).is_err());
    assert!(report.set_price(RowId(99), None).is_err());
    assert!(report.set_stock(RowId(99), None).is_err());
    assert!(report.set_status(RowId(99), None).is_err());
    assert!(report.add_commodity(CommodityId(0)).is_err());
    assert_eq!(report, saved);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_6__manual_creation_refuses_absent_identity_or_blank_game_version() {
    let key = ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap();
    assert!(Report::manual(ReportId(0), key, Timestamp::from_seconds(100), None).is_err());
    for version in ["", "  "] {
        let error = Report::manual(
            ReportId(1),
            key,
            Timestamp::from_seconds(100),
            Some(version),
        )
        .unwrap_err();
        assert!(!error.to_string().trim().is_empty());
    }
    assert!(Report::manual(ReportId(1), key, Timestamp::from_seconds(100), None).is_ok());
}
