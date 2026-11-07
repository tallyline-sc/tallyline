// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Guarded quantity decisions at the Reporting application seam.

use super::*;
use tallyline_reporting::api::ReportRepository;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_ui_12_q_manual__composed_stock_review_uses_shipping_bands_and_value_bound_approval()
 {
    use tallyline_shared_kernel::api::model::TradeSide;
    let report = entered(Some("200")).unwrap();
    let row = report.rows()[0].id();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let snapshot = reference("100", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap();
    let review = app
        .review_manual(report.id(), report.version(), &snapshot)
        .unwrap();
    assert_eq!(review.stocks()[0].tolerance().percent().get(), 20);
    assert_eq!(review.stocks()[0].tolerance().minimum().to_string(), "16");
    assert!(review.report().rows()[0].stock_needs_confirmation());
    assert_eq!(
        review.confirmation_fields(),
        vec![tallyline_reporting::api::model::ReviewField::Stock(row)]
    );
    assert_eq!(review.summary().major, 1);
    assert_eq!(review.summary().needs_confirmation, 1);
    assert_eq!(review.summary().unreviewed, 1);
    let approved = app
        .confirm_stock(report.id(), review.report().version(), row, &snapshot)
        .unwrap();
    assert_eq!(
        approved.rows()[0]
            .stock_confirmation()
            .unwrap()
            .value()
            .to_string(),
        "200"
    );
    assert!(!approved.rows()[0].stock_needs_confirmation());
    let changed = app
        .edit(
            report.id(),
            approved.version(),
            tallyline_reporting::api::ManualEdit::Stock(row, Some("201".parse().unwrap())),
            &snapshot,
        )
        .unwrap();
    assert!(changed.rows()[0].stock_confirmation().is_none());
    assert_eq!(changed.rows()[0].stock().unwrap().to_string(), "201");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual__stock_approval_survives_same_class_and_revokes_only_for_worse_class() {
    use tallyline_shared_kernel::api::model::TradeSide;
    let report = entered(Some("120")).unwrap();
    let row = report.rows()[0].id();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let first = reference("100", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap();
    let approved = app
        .confirm_stock(report.id(), report.version(), row, &first)
        .unwrap();
    assert_eq!(
        approved.rows()[0].stock_confirmation().unwrap().deviation(),
        Deviation::Minor
    );
    let same = reference("101", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap();
    app.set_stock_settings(
        tallyline_reporting::api::model::StockSettings::new(
            Percent::whole(20),
            "8".parse().unwrap(),
        )
        .unwrap(),
    );
    let unchanged = app
        .review_manual(report.id(), approved.version(), &same)
        .unwrap();
    assert!(unchanged.report().rows()[0].stock_confirmation().is_some());
    app.set_stock_settings(
        tallyline_reporting::api::model::StockSettings::new(
            Percent::whole(10),
            "8".parse().unwrap(),
        )
        .unwrap(),
    );
    let worse = app
        .review_manual(report.id(), unchanged.report().version(), &same)
        .unwrap();
    assert!(worse.report().rows()[0].stock_confirmation_revoked());
    assert!(worse.report().rows()[0].stock_needs_confirmation());
    assert!(worse.report().rows()[0].stock_unreviewed());
    assert_eq!(worse.report().rows()[0].stock().unwrap().to_string(), "120");
    assert!(
        tallyline_reporting::api::model::StockSettings::new(
            Percent::whole(21),
            "8".parse().unwrap()
        )
        .is_err()
    );
    assert!(
        tallyline_reporting::api::model::StockSettings::new(
            Percent::whole(10),
            "17".parse().unwrap()
        )
        .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_1_q_manual_r_ui_12_q_manual__numeric_readiness_requires_complete_entered_rows_and_all_decisions()
 {
    use tallyline_reporting::api::model::{NumericBlock, ReviewField};
    let report = entered(Some("200")).unwrap();
    let row = report.rows()[0].id();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    let review = app
        .review_manual(report.id(), report.version(), &snapshot)
        .unwrap();
    let blocks = review.numeric_readiness().unwrap_err();
    assert!(blocks.contains(&NumericBlock::MissingValue(ReviewField::Price(row))));
    assert!(blocks.contains(&NumericBlock::MissingValue(ReviewField::Status(row))));
    assert!(blocks.contains(&NumericBlock::NeedsConfirmation(ReviewField::Stock(row))));
    let report = app
        .edit(
            report.id(),
            review.report().version(),
            tallyline_reporting::api::ManualEdit::Price(row, Some("100".parse().unwrap())),
            &snapshot,
        )
        .unwrap();
    let report = app
        .edit(
            report.id(),
            report.version(),
            tallyline_reporting::api::ManualEdit::Status(row, std::num::NonZeroU8::new(3)),
            &snapshot,
        )
        .unwrap();
    let report = app
        .confirm_stock(report.id(), report.version(), row, &snapshot)
        .unwrap();
    let review = app
        .review_manual(report.id(), report.version(), &snapshot)
        .unwrap();
    assert_eq!(review.numeric_readiness(), Ok(()));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_12_q_manual__stock_approval_preserves_same_major_magnitude_and_stale_display_changes()
 {
    use tallyline_shared_kernel::api::model::TradeSide;
    let report = entered(Some("200")).unwrap();
    let row = report.rows()[0].id();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let snapshot = reference("100", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap();
    let approved = app
        .confirm_stock(report.id(), report.version(), row, &snapshot)
        .unwrap();
    let different = reference("50", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap();
    let review = app
        .review_manual(report.id(), approved.version(), &different)
        .unwrap();
    assert!(review.report().rows()[0].stock_confirmation().is_some());
    let mut old = ReportingApplication::new(
        Repository(vec![review.report().clone()]),
        At(Timestamp::from_seconds(2_000_000)),
    );
    let stale = old
        .review_manual(report.id(), review.report().version(), &different)
        .unwrap();
    assert!(stale.stocks()[0].reference_outdated());
    assert!(stale.report().rows()[0].stock_confirmation().is_some());
    assert_eq!(stale.report().rows()[0].stock(), approved.rows()[0].stock());
    let unchanged = old
        .edit(
            report.id(),
            stale.report().version(),
            tallyline_reporting::api::ManualEdit::Stock(row, Some("200".parse().unwrap())),
            &different,
        )
        .unwrap();
    assert!(unchanged.rows()[0].stock_confirmation().is_some());
    let changed_key = tallyline_reporting::api::model::ReportKey::new(
        unchanged.key().terminal(),
        TradeSide::Sell,
        unchanged.key().environment(),
    )
    .unwrap();
    let changed = old
        .edit(
            report.id(),
            unchanged.version(),
            tallyline_reporting::api::ManualEdit::ChangeKey(changed_key),
            &support::snapshot(UexEnvironment::Live).unwrap(),
        )
        .unwrap();
    assert!(changed.rows()[0].stock_confirmation().is_none());
    assert_eq!(changed.rows()[0].stock().unwrap().to_string(), "200");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_1_q_manual_r_val_2_q_manual__empty_rows_do_not_become_sent_rows_and_exact_failure_is_atomic()
 {
    use tallyline_reporting::api::model::NumericBlock;
    use tallyline_reporting::api::{ManualEdit, RepositoryError};
    let report = entered(None).unwrap();
    let row = report.rows()[0].id();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    assert_eq!(
        app.review_manual(report.id(), report.version(), &snapshot)
            .unwrap()
            .numeric_readiness(),
        Err(vec![NumericBlock::NoEnteredRows])
    );
    assert_eq!(
        app.confirm_stock(report.id(), report.version(), row, &snapshot),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.focus_stock(report.id(), report.version(), row),
        Err(RepositoryError::Invalid)
    );
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::Stock(row, Some("79228162514264337593543950335".parse().unwrap())),
            &snapshot,
        )
        .unwrap();
    let tiny = reference(
        "1e-28",
        tallyline_shared_kernel::api::model::TradeSide::Buy,
        Timestamp::from_seconds(900),
    )
    .unwrap();
    assert_eq!(
        app.review_manual(report.id(), report.version(), &tiny),
        Err(RepositoryError::AssessmentUnavailable)
    );
    assert_eq!(
        app.confirm_stock(report.id(), report.version(), row, &tiny),
        Err(RepositoryError::AssessmentUnavailable)
    );
    assert_eq!(app.drafts().unwrap(), vec![report.clone()]);
    assert_eq!(
        app.confirm_stock(report.id(), report.version() - 1, row, &snapshot),
        Err(RepositoryError::Conflict)
    );
    assert_eq!(
        app.focus_stock(report.id(), report.version() - 1, row),
        Err(RepositoryError::Conflict)
    );
    assert_eq!(
        app.reassess_stocks(report.id(), report.version() - 1, &snapshot),
        Err(RepositoryError::Conflict)
    );
}

struct RefusingWrites(Repository);
impl ReportRepository for RefusingWrites {
    fn create_manual(
        &mut self,
        key: tallyline_reporting::api::model::ReportKey,
        at: Timestamp,
        version: Option<&str>,
    ) -> Result<Report, tallyline_reporting::api::RepositoryError> {
        self.0.create_manual(key, at, version)
    }
    fn load_draft(
        &mut self,
        id: ReportId,
    ) -> Result<Option<Report>, tallyline_reporting::api::RepositoryError> {
        self.0.load_draft(id)
    }
    fn drafts(&mut self) -> Result<Vec<Report>, tallyline_reporting::api::RepositoryError> {
        self.0.drafts()
    }
    fn save_draft(
        &mut self,
        _: &Report,
        _: u64,
    ) -> Result<(), tallyline_reporting::api::RepositoryError> {
        Err(tallyline_reporting::api::RepositoryError::Unavailable)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_11_r_ui_12_q_manual__stock_save_failures_never_publish_and_unchanged_review_needs_no_save()
{
    use tallyline_reporting::api::RepositoryError;
    let report = entered(Some("100")).unwrap();
    let row = report.rows()[0].id();
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let pending = app
        .review_manual(report.id(), report.version(), &snapshot)
        .unwrap()
        .report()
        .clone();
    let mut app =
        ReportingApplication::new(RefusingWrites(Repository(vec![pending.clone()])), Fixed);
    assert_eq!(
        app.review_manual(report.id(), pending.version(), &snapshot)
            .unwrap()
            .report(),
        &pending
    );
    assert_eq!(
        app.confirm_stock(report.id(), pending.version(), row, &snapshot),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(
        app.focus_stock(report.id(), pending.version(), row),
        Err(RepositoryError::Unavailable)
    );
    let equal = reference(
        "100",
        tallyline_shared_kernel::api::model::TradeSide::Buy,
        Timestamp::from_seconds(900),
    )
    .unwrap();
    assert_eq!(
        app.reassess_stocks(report.id(), pending.version(), &equal),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(
        app.review_manual(report.id(), pending.version(), &equal),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(app.drafts().unwrap(), vec![pending]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual_r_val_7_q_manual__focused_major_stock_stays_reviewed_during_same_class_reassessment()
 {
    use tallyline_shared_kernel::api::model::TradeSide;
    let report = entered(Some("100")).unwrap();
    let row = report.rows()[0].id();
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let pending = app
        .review_manual(report.id(), report.version(), &snapshot)
        .unwrap();
    let focused = app
        .focus_stock(report.id(), pending.report().version(), row)
        .unwrap();
    assert!(!focused.rows()[0].stock_unreviewed());
    assert!(focused.rows()[0].stock_needs_confirmation());
    for current in [
        snapshot,
        reference("1", TradeSide::Buy, Timestamp::from_seconds(900)).unwrap(),
    ] {
        let mut app = ReportingApplication::new(Repository(vec![focused.clone()]), Fixed);
        let repeated = app
            .review_manual(report.id(), focused.version(), &current)
            .unwrap();
        assert!(!repeated.report().rows()[0].stock_unreviewed());
        assert_eq!(repeated.summary().unreviewed, 0);
        assert_eq!(repeated.summary().needs_confirmation, 1);
        assert!(repeated.report().rows()[0].stock_confirmation().is_none());
        assert_eq!(repeated.report(), &focused);
        assert_eq!(app.drafts().unwrap(), vec![focused.clone()]);
    }
}
