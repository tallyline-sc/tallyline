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

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__price_review_uses_the_shared_sourced_fallback_when_parameters_are_unavailable()
{
    let original = price_reference("123.45", 25).unwrap();
    let reference = support::cached_snapshot(
        UexEnvironment::Live,
        vec![EndpointData::TerminalPriors(
            original.terminal_priors(TerminalId(11)).unwrap().clone(),
        )],
    );
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
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let assessment = app.review_prices(ReportId(1), &reference).unwrap()[0];
    assert_eq!(assessment.deviation(), Some(Deviation::Major));
    assert_eq!(assessment.tolerance().percent().get(), 25);
    assert!(matches!(
        assessment.tolerance().source(),
        tallyline_reference_data::api::model::PriceToleranceSource::SettingsFallback(_)
    ));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_nf_5__application_price_override_can_tighten_but_cannot_hide_a_deviation() {
    use tallyline_shared_kernel::api::model::Percent;
    let reference = price_reference("123.45", 25).unwrap();
    for (value, percent, expected, ignored) in [
        ("150", 10, Deviation::Major, false),
        ("150", 50, Deviation::Minor, true),
        ("185.175", 50, Deviation::Major, true),
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
        app.set_price_tolerance_override(Some(Percent::whole(percent)));
        let assessment = app.review_prices(report.id(), &reference).unwrap()[0];
        assert_eq!(assessment.deviation(), Some(expected));
        assert_eq!(assessment.tolerance().override_ignored(), ignored);
        assert_eq!(app.drafts().unwrap(), vec![report]);
    }
}
