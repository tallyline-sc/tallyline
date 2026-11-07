// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable numeric decisions through the Reporting application.

#[path = "price_confirmation/failures.rs"]
mod failures;
mod support;
use support::{Fixed, Repository, key, snapshot};
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::model::{Deviation, Report, ReportId};
use tallyline_reporting::api::{ReportingApplication, RepositoryError};
use tallyline_shared_kernel::api::model::{CommodityId, Timestamp};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_10_q_manual_r_ui_12_q_manual__price_approval_is_saved_and_bound_to_exact_value() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("200".parse().unwrap())).unwrap();
    report.set_stock(row, Some("17".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let reference = snapshot(UexEnvironment::Live).unwrap();
    let status = app
        .confirm_status(ReportId(1), report.version(), row, &reference)
        .unwrap();
    let confirmed = app
        .confirm_price(ReportId(1), status.version(), row, &reference)
        .unwrap();
    let approval = confirmed.rows()[0].price_confirmation().unwrap();
    assert_eq!(approval.value(), "200".parse().unwrap());
    assert_eq!(approval.deviation(), Deviation::Major);
    assert_eq!(approval.reference_environment(), UexEnvironment::Live);
    assert_eq!(app.drafts().unwrap(), vec![confirmed.clone()]);
    assert_eq!(
        app.confirm_price(ReportId(1), status.version(), row, &reference),
        Err(RepositoryError::Conflict)
    );
    let mut edited = confirmed.clone();
    edited
        .set_price(row, Some("200.00".parse().unwrap()))
        .unwrap();
    assert_eq!(edited.rows()[0].price_confirmation(), Some(approval));
    edited.set_price(row, Some("201".parse().unwrap())).unwrap();
    assert!(edited.rows()[0].price_confirmation().is_none());
    assert_eq!(
        edited.rows()[0].status_confirmation(),
        confirmed.rows()[0].status_confirmation()
    );
    assert_eq!(edited.rows()[0].stock(), report.rows()[0].stock());
}

fn reference(
    price: Option<&str>,
) -> Result<tallyline_reference_data::api::model::ReferenceSnapshot, Box<dyn std::error::Error>> {
    use tallyline_reference_data::api::model::{
        EndpointData, PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    };
    use tallyline_shared_kernel::api::model::{TerminalId, TradeSide};
    let mut priors = Vec::new();
    if let Some(price) = price {
        priors.push(PricePrior::new(
            CommodityId(42),
            TradeSide::Buy,
            PriorValues {
                price: price.parse()?,
                stock: "3".parse()?,
                status: std::num::NonZeroU8::new(3),
                container_sizes: vec![],
            },
            PriorProvenance {
                game_version: "4.10.2".to_owned(),
                modified_at: Timestamp::from_seconds(900),
            },
        )?);
    }
    Ok(support::cached_snapshot(
        UexEnvironment::Live,
        vec![EndpointData::TerminalPriors(TerminalPriors::new(
            TerminalId(11),
            priors,
        )?)],
    ))
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_12_q_manual__price_reassessment_uses_unlowered_worsening_relation_and_preserves_user_values()
 {
    let cases = [
        (Some("100"), Deviation::Equal, [false, true, true, false]),
        (Some("90"), Deviation::Minor, [false, false, true, false]),
        (Some("50"), Deviation::Major, [false, false, false, false]),
        (None, Deviation::NoReference, [false, true, true, false]),
    ];
    for (old, old_level, worse) in cases {
        for (index, (new, new_level, _)) in cases.into_iter().enumerate() {
            let mut report = Report::manual(
                ReportId(1),
                key().unwrap(),
                Timestamp::from_seconds(900),
                Some("4.10.2"),
            )
            .unwrap();
            let row = report.add_commodity(CommodityId(42)).unwrap();
            report.set_price(row, Some("100".parse().unwrap())).unwrap();
            report.set_stock(row, Some("17".parse().unwrap())).unwrap();
            report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
            let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
            let status = app
                .confirm_status(
                    ReportId(1),
                    report.version(),
                    row,
                    &snapshot(UexEnvironment::Live).unwrap(),
                )
                .unwrap();
            let approved = app
                .confirm_price(ReportId(1), status.version(), row, &reference(old).unwrap())
                .unwrap();
            let assessed = app
                .reassess_prices(ReportId(1), approved.version(), &reference(new).unwrap())
                .unwrap();
            assert_eq!(
                assessed.rows()[0].price_confirmation().is_none(),
                worse[index],
                "{old_level:?} -> {new_level:?}"
            );
            assert_eq!(assessed.rows()[0].price_unreviewed(), worse[index]);
            assert_eq!(assessed.rows()[0].price_needs_confirmation(), worse[index]);
            assert_eq!(assessed.rows()[0].price_deviation(), Some(new_level));
            assert_eq!(assessed.rows()[0].price(), report.rows()[0].price());
            assert_eq!(assessed.rows()[0].stock(), report.rows()[0].stock());
            assert_eq!(
                assessed.rows()[0].status_confirmation(),
                status.rows()[0].status_confirmation()
            );
            assert_eq!(assessed.observed_at(), report.observed_at());
            assert_eq!(assessed.game_version(), Some("4.10.2"));
            assert_eq!(app.drafts().unwrap(), vec![assessed.clone()]);
            let focused = app
                .focus_price(ReportId(1), assessed.version(), row)
                .unwrap();
            assert!(!focused.rows()[0].price_unreviewed());
            assert_eq!(focused.rows()[0].price_needs_confirmation(), worse[index]);
            assert_eq!(
                app.reassess_prices(ReportId(1), focused.version(), &reference(new).unwrap())
                    .unwrap(),
                focused
            );
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__pending_price_focus_and_context_edits_never_approve_values() {
    use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
    use tallyline_reporting::api::model::ReportKey;
    use tallyline_shared_kernel::api::model::{TerminalId, TradeSide};
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("200".parse().unwrap())).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let prior = snapshot(UexEnvironment::Live).unwrap();
    let assessed = app
        .reassess_prices(ReportId(1), report.version(), &prior)
        .unwrap();
    assert!(assessed.rows()[0].price_unreviewed());
    assert!(assessed.rows()[0].price_needs_confirmation());
    assert!(!assessed.rows()[0].price_confirmation_revoked());
    let focused = app
        .focus_price(ReportId(1), assessed.version(), row)
        .unwrap();
    assert!(!focused.rows()[0].price_unreviewed());
    assert!(focused.rows()[0].price_needs_confirmation());
    assert_eq!(
        app.focus_price(ReportId(1), focused.version(), row)
            .unwrap(),
        focused
    );
    let informational = app
        .reassess_prices(
            ReportId(1),
            focused.version(),
            &reference(Some("200")).unwrap(),
        )
        .unwrap();
    assert!(!informational.rows()[0].price_unreviewed());
    assert!(!informational.rows()[0].price_needs_confirmation());
    let approved = app
        .confirm_price(ReportId(1), informational.version(), row, &prior)
        .unwrap();
    assert!(!approved.rows()[0].price_unreviewed());
    assert!(!approved.rows()[0].price_needs_confirmation());
    for key in [
        ReportKey::new(TerminalId(12), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        ReportKey::new(TerminalId(11), TradeSide::Sell, GameEnvironment::Live).unwrap(),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Ptu).unwrap(),
    ] {
        let mut changed = approved.clone();
        changed.change_key(key).unwrap();
        assert!(changed.rows()[0].price_confirmation().is_none());
        assert!(changed.rows()[0].price_deviation().is_none());
        assert!(!changed.rows()[0].price_unreviewed());
    }
    let mut equal = approved.clone();
    equal.change_key(approved.key()).unwrap();
    assert_eq!(equal, approved);
    equal.set_price(row, None).unwrap();
    assert!(equal.rows()[0].price_confirmation().is_none());
    assert!(equal.rows()[0].price_deviation().is_none());
    let mut mapping = EnvironmentMapping::default();
    mapping = mapping.with_environment(GameEnvironment::Live, UexEnvironment::Ptu);
    app.set_mapping(mapping);
    let remapped = app
        .reassess_prices(
            ReportId(1),
            approved.version(),
            &snapshot(UexEnvironment::Ptu).unwrap(),
        )
        .unwrap();
    assert!(remapped.rows()[0].price_confirmation().is_none());
    assert!(remapped.rows()[0].price_confirmation_revoked());
    assert!(remapped.rows()[0].price_needs_confirmation());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual__unavailable_exact_price_assessment_refuses_approval_and_reassessment_without_publishing()
 {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("79228162514264337593543950335".parse().unwrap()))
        .unwrap();
    let other = report.add_commodity(CommodityId(43)).unwrap();
    report
        .set_price(other, Some("100".parse().unwrap()))
        .unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let unavailable = reference(Some("0.0000000000000000000000000001")).unwrap();
    assert_eq!(
        app.confirm_price(ReportId(1), report.version(), row, &unavailable),
        Err(RepositoryError::AssessmentUnavailable)
    );
    assert_eq!(
        app.reassess_prices(ReportId(1), report.version(), &unavailable),
        Err(RepositoryError::AssessmentUnavailable)
    );
    let failure = app
        .confirm_price(ReportId(1), report.version(), row, &unavailable)
        .unwrap_err();
    assert_eq!(
        failure.to_string(),
        "Exact price comparison unavailable; cross-check the entered value and reference"
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_12_q_manual__combined_review_publishes_one_committed_report_and_its_exact_assessments()
 {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("200".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let prior = snapshot(UexEnvironment::Live).unwrap();
    let review = app
        .review_manual(ReportId(1), report.version(), &prior)
        .unwrap();
    assert!(review.report().rows()[0].price_needs_confirmation());
    assert!(review.report().rows()[0].status_needs_confirmation());
    assert_eq!(review.summary().major, 2);
    assert_eq!(review.summary().minor, 0);
    assert_eq!(review.summary().needs_confirmation, 2);
    assert_eq!(review.summary().unreviewed, 2);
    assert_eq!(
        review.confirmation_fields(),
        vec![
            tallyline_reporting::api::model::ReviewField::Price(row),
            tallyline_reporting::api::model::ReviewField::Status(row)
        ]
    );
    assert_eq!(
        review.prices()[0].reference().unwrap().to_string(),
        "123.45"
    );
    assert_eq!(
        review.prices()[0].difference().unwrap().to_string(),
        "76.55"
    );
    assert_eq!(review.statuses()[0].reference().unwrap().get(), 3);
    assert_eq!(app.drafts().unwrap(), vec![review.report().clone()]);
    let repeated = app
        .review_manual(ReportId(1), review.report().version(), &prior)
        .unwrap();
    assert_eq!(repeated.report(), review.report());
    assert_eq!(
        app.review_manual(ReportId(1), report.version(), &prior)
            .unwrap_err(),
        RepositoryError::Conflict
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__review_counters_keep_informational_differences_separate_from_required_decisions()
 {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("100".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(4)).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    let review = app
        .review_manual(
            ReportId(1),
            report.version(),
            &reference(Some("90")).unwrap(),
        )
        .unwrap();
    assert_eq!(
        review.summary(),
        tallyline_reporting::api::model::ManualReviewSummary {
            minor: 2,
            major: 0,
            needs_confirmation: 0,
            unreviewed: 0
        }
    );
    assert!(review.confirmation_fields().is_empty());
    let absent = app
        .review_manual(
            ReportId(1),
            review.report().version(),
            &reference(None).unwrap(),
        )
        .unwrap();
    assert_eq!(
        absent.summary(),
        tallyline_reporting::api::model::ManualReviewSummary::default()
    );
    assert!(absent.confirmation_fields().is_empty());
    assert_eq!(
        tallyline_reporting::api::model::ReviewField::Price(row).row(),
        row
    );
    assert_eq!(
        tallyline_reporting::api::model::ReviewField::Status(row).row(),
        row
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_12_q_manual__price_approval_binds_ptu_mapping_and_repeated_same_environment_review_retains_it()
 {
    use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("200".parse().unwrap())).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    app.set_mapping(
        EnvironmentMapping::default().with_environment(GameEnvironment::Live, UexEnvironment::Ptu),
    );
    let ptu = snapshot(UexEnvironment::Ptu).unwrap();
    let approved = app
        .confirm_price(ReportId(1), report.version(), row, &ptu)
        .unwrap();
    assert_eq!(
        approved.rows()[0]
            .price_confirmation()
            .unwrap()
            .reference_environment(),
        UexEnvironment::Ptu
    );
    assert_eq!(
        app.reassess_prices(ReportId(1), approved.version(), &ptu)
            .unwrap(),
        approved
    );
    app.set_mapping(EnvironmentMapping::default());
    let remapped = app
        .reassess_prices(
            ReportId(1),
            approved.version(),
            &snapshot(UexEnvironment::Live).unwrap(),
        )
        .unwrap();
    assert!(remapped.rows()[0].price_confirmation().is_none());
    assert!(remapped.rows()[0].price_confirmation_revoked());
}
#[path = "price_confirmation/reassessment.rs"]
mod reassessment;
