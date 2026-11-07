// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact manual value translation remains separate from complete release authorisation.

use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
use tallyline_reporting::api::model::{Report, ReportKey};
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_workflows::api::manual_submission_request;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_1_r_sub_8_r_man_2__manual_request_maps_exact_entered_rows_and_fixed_release_context() {
    let mut report = Report::manual(
        ReportId(4),
        ReportKey::new(
            TerminalId(89),
            TradeSide::Sell,
            GameEnvironment::TechPreview,
        )
        .unwrap(),
        Timestamp::from_seconds(1000),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(7)).unwrap();
    report.add_commodity(CommodityId(8)).unwrap();
    report
        .set_price(row, Some("120.125".parse().unwrap()))
        .unwrap();
    assert!(
        manual_submission_request(
            &report,
            EnvironmentMapping::default().environment(report.key().environment()),
            "dummy-a",
            SubmissionMode::DryRun
        )
        .is_err()
    );
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(1)).unwrap();
    let before = report.clone();
    let request = manual_submission_request(
        &report,
        EnvironmentMapping::default().environment(report.key().environment()),
        "dummy-a",
        SubmissionMode::DryRun,
    )
    .unwrap();
    assert_eq!(request.header().report, ReportId(4));
    assert_eq!(request.header().report_version, report.version());
    assert_eq!(request.header().account, "dummy-a");
    assert_eq!(request.header().mode, SubmissionMode::DryRun);
    assert_eq!(request.header().environment, UexEnvironment::Ptu);
    assert_eq!(request.header().terminal, TerminalId(89));
    assert_eq!(request.header().side, TradeSide::Sell);
    assert_eq!(request.header().observed_at, Timestamp::from_seconds(1000));
    assert_eq!(request.header().game_version, "4.10.2");
    assert_eq!(request.rows().len(), 1);
    assert_eq!(request.rows()[0].id(), row);
    assert_eq!(request.rows()[0].commodity(), CommodityId(7));
    assert_eq!(request.rows()[0].price().to_string(), "120.125");
    assert!(request.rows()[0].stock().is_zero());
    assert_eq!(request.rows()[0].status(), 1);
    let remapped = EnvironmentMapping::default()
        .with_environment(GameEnvironment::TechPreview, UexEnvironment::Live);
    let production = manual_submission_request(
        &report,
        remapped.environment(report.key().environment()),
        "dummy-b",
        SubmissionMode::Production,
    )
    .unwrap();
    assert_eq!(production.header().environment, UexEnvironment::Live);
    assert_eq!(production.header().account, "dummy-b");
    assert_eq!(production.header().mode, SubmissionMode::Production);
    assert_eq!(production.rows(), request.rows());
    assert_eq!(report, before);
}
