// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Quantitative and age changes assessed through the normative level-bound approval rule.

use super::failures::UnavailableWrites;
use super::*;
use tallyline_shared_kernel::api::model::{Percent, RowId};

fn entered(value: &str) -> Result<(Report, RowId), Box<dyn std::error::Error>> {
    let mut report = Report::manual(
        ReportId(1),
        key()?,
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )?;
    let row = report.add_commodity(CommodityId(42))?;
    report.set_price(row, Some(value.parse()?))?;
    report.set_stock(row, Some("17".parse()?))?;
    report.set_status(row, std::num::NonZeroU8::new(5))?;
    let version = report.version();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let report = app.reassess_stocks(ReportId(1), version, &reference(Some("100"))?)?;
    Ok((report, row))
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_12_q_manual__larger_or_smaller_major_amounts_preserve_level_bound_approval_and_create_no_save()
 {
    for (prior, amount, percent) in [
        ("50", "150", "+300.00"),
        ("150", "50", "+33.33"),
        ("100", "100", "+100.00"),
    ] {
        let (report, row) = entered("200").unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let status = app
            .confirm_status(
                ReportId(1),
                report.version(),
                row,
                &reference(Some("100")).unwrap(),
            )
            .unwrap();
        let approved = app
            .confirm_price(
                ReportId(1),
                status.version(),
                row,
                &reference(Some("100")).unwrap(),
            )
            .unwrap();
        let mut app =
            ReportingApplication::new(UnavailableWrites(Repository(vec![approved.clone()])), Fixed);
        let reviewed = app
            .review_manual(
                ReportId(1),
                approved.version(),
                &reference(Some(prior)).unwrap(),
            )
            .unwrap();
        assert_eq!(
            reviewed.prices()[0].difference().unwrap().to_string(),
            amount
        );
        assert_eq!(
            reviewed.prices()[0]
                .percentage_difference()
                .unwrap()
                .as_deref(),
            Some(percent)
        );
        assert_eq!(reviewed.prices()[0].deviation(), Some(Deviation::Major));
        assert_eq!(reviewed.report(), &approved);
        assert_eq!(app.drafts().unwrap(), vec![approved]);
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_val_7_q_manual__tightened_tolerance_revokes_only_when_the_unlowered_price_class_worsens()
 {
    for (value, revoked) in [("120", true), ("200", false)] {
        let (report, row) = entered(value).unwrap();
        let prior = reference(Some("100")).unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let status = app
            .confirm_status(ReportId(1), report.version(), row, &prior)
            .unwrap();
        let approved = app
            .confirm_price(ReportId(1), status.version(), row, &prior)
            .unwrap();
        app.set_price_tolerance_override(Some(Percent::whole(10)));
        let reviewed = app
            .review_manual(ReportId(1), approved.version(), &prior)
            .unwrap();
        assert_eq!(reviewed.prices()[0].tolerance().percent().get(), 10);
        assert_eq!(reviewed.prices()[0].deviation(), Some(Deviation::Major));
        assert_eq!(
            reviewed.report().rows()[0].price_confirmation().is_none(),
            revoked
        );
        assert_eq!(
            reviewed.report().rows()[0].price_confirmation_revoked(),
            revoked
        );
        assert_eq!(
            reviewed.report().rows()[0].price_needs_confirmation(),
            revoked
        );
        assert_eq!(
            reviewed.report().rows()[0].price(),
            approved.rows()[0].price()
        );
        assert_eq!(
            reviewed.report().rows()[0].stock(),
            approved.rows()[0].stock()
        );
        assert_eq!(
            reviewed.report().rows()[0].status_confirmation(),
            approved.rows()[0].status_confirmation()
        );
        if !revoked {
            assert_eq!(reviewed.report(), &approved);
        }
        assert_eq!(app.drafts().unwrap(), vec![reviewed.report().clone()]);
    }
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
fn r_ui_11_q_manual_r_val_7_q_manual__fresh_to_stale_changes_display_only_and_focus_never_overwrites_confirmed_values()
 {
    for (value, unlowered, displayed) in [
        ("120", Deviation::Minor, Deviation::Equal),
        ("200", Deviation::Major, Deviation::Major),
    ] {
        let (report, row) = entered(value).unwrap();
        let prior = reference(Some("100")).unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let status = app
            .confirm_status(ReportId(1), report.version(), row, &prior)
            .unwrap();
        let approved = app
            .confirm_price(ReportId(1), status.version(), row, &prior)
            .unwrap();
        let fresh = app.review_prices(ReportId(1), &prior).unwrap();
        assert!(!fresh[0].reference_outdated());
        let later = At(Timestamp::from_millis(900_000 + 1_296_000_001));
        let mut app =
            ReportingApplication::new(UnavailableWrites(Repository(vec![approved.clone()])), later);
        let review = app
            .review_manual(ReportId(1), approved.version(), &prior)
            .unwrap();
        assert!(review.prices()[0].reference_outdated());
        assert_eq!(
            review.prices()[0].reference_age_millis(),
            Some(1_296_000_001)
        );
        assert_eq!(review.prices()[0].deviation(), Some(unlowered));
        assert_eq!(review.prices()[0].display_deviation(), Some(displayed));
        assert_eq!(review.report(), &approved);
        assert_eq!(
            app.focus_price(ReportId(1), approved.version(), row)
                .unwrap(),
            approved
        );
        assert_eq!(
            app.focus_status(ReportId(1), approved.version(), row)
                .unwrap(),
            approved
        );
        assert_eq!(app.drafts().unwrap(), vec![approved]);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual_r_val_7_q_manual__unconfirmed_major_becoming_informational_clears_unreviewed_without_inventing_approval()
 {
    for (prior, level) in [
        (Some("200"), Deviation::Equal),
        (Some("190"), Deviation::Minor),
        (None, Deviation::NoReference),
    ] {
        let (report, row) = entered("200").unwrap();
        let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
        let pending = app
            .reassess_prices(
                ReportId(1),
                report.version(),
                &reference(Some("100")).unwrap(),
            )
            .unwrap();
        assert!(pending.rows()[0].price_unreviewed());
        assert!(pending.rows()[0].price_needs_confirmation());
        let informational = app
            .reassess_prices(ReportId(1), pending.version(), &reference(prior).unwrap())
            .unwrap();
        assert_eq!(informational.rows()[0].price_deviation(), Some(level));
        assert!(!informational.rows()[0].price_unreviewed());
        assert!(!informational.rows()[0].price_needs_confirmation());
        assert!(informational.rows()[0].price_confirmation().is_none());
        assert_eq!(informational.rows()[0].price(), report.rows()[0].price());
        assert_eq!(informational.rows()[0].stock(), report.rows()[0].stock());
        assert_eq!(informational.rows()[0].status(), report.rows()[0].status());
        assert_eq!(app.drafts().unwrap(), vec![informational]);
        assert_eq!(row, pending.rows()[0].id());
    }
}
