// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic manual edit contracts at the public application boundary.

mod support;

use support::{Fixed, Repository};
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::{ManualEdit, ReportingApplication};
use tallyline_shared_kernel::api::model::CommodityId;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_3__ordered_edits_preserve_exact_user_values_and_explicit_takeover_together() {
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let report = app.create(support::key().unwrap(), None).unwrap();
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::AddCommodity(CommodityId(42)),
            &snapshot,
        )
        .unwrap();
    let row = report.rows()[0].id();
    let report = app
        .edit_batch(
            report.id(),
            report.version(),
            &[
                ManualEdit::TakeOver(row),
                ManualEdit::Price(row, Some("12345678901234567890.125".parse().unwrap())),
            ],
            &snapshot,
        )
        .unwrap();
    assert_eq!(
        report.rows()[0].price().unwrap().to_string(),
        "12345678901234567890.125"
    );
    assert_eq!(report.rows()[0].stock().unwrap().to_string(), "0");
    assert_eq!(report.rows()[0].status().unwrap().get(), 3);
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_6__batch_refusals_preserve_the_complete_draft() {
    use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
    use tallyline_reporting::api::RepositoryError;
    use tallyline_reporting::api::model::{ReportKey, RowId};
    use tallyline_shared_kernel::api::model::{TerminalId, TradeSide};
    let live = support::snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let report = app.create(support::key().unwrap(), None).unwrap();
    let report = app
        .edit(
            report.id(),
            0,
            ManualEdit::AddCommodity(CommodityId(42)),
            &live,
        )
        .unwrap();
    let row = report.rows()[0].id();
    assert_eq!(
        app.edit_batch(report.id(), report.version(), &[], &live),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.edit_batch(report.id(), 0, &[ManualEdit::TakeOver(row)], &live),
        Err(RepositoryError::Conflict)
    );
    for invalid in [
        ManualEdit::AddCommodity(CommodityId(999)),
        ManualEdit::Price(RowId(99), Some("100".parse().unwrap())),
        ManualEdit::Price(row, Some("0".parse().unwrap())),
        ManualEdit::Status(row, std::num::NonZeroU8::new(7)),
        ManualEdit::ChangeKey(
            ReportKey::new(TerminalId(999), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        ),
    ] {
        assert_eq!(
            app.edit_batch(
                report.id(),
                report.version(),
                &[
                    ManualEdit::Stock(row, Some("12.5".parse().unwrap())),
                    invalid,
                ],
                &live
            ),
            Err(RepositoryError::Invalid)
        );
        assert_eq!(app.drafts().unwrap(), vec![report.clone()]);
    }
    let empty = support::cached_snapshot(UexEnvironment::Live, vec![]);
    assert_eq!(
        app.edit_batch(
            report.id(),
            report.version(),
            &[
                ManualEdit::Stock(row, Some("12.5".parse().unwrap())),
                ManualEdit::TakeOver(row),
            ],
            &empty
        ),
        Err(RepositoryError::Invalid)
    );
    app.set_mapping(
        EnvironmentMapping::default().with_environment(GameEnvironment::Live, UexEnvironment::Ptu),
    );
    assert_eq!(
        app.edit_batch(
            report.id(),
            report.version(),
            &[ManualEdit::TakeOver(row)],
            &live
        ),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_val_7_q_manual__successful_batch_revokes_only_decisions_for_changed_values() {
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let report = app.create(support::key().unwrap(), None).unwrap();
    let report = app
        .edit(
            report.id(),
            0,
            ManualEdit::AddCommodity(CommodityId(42)),
            &snapshot,
        )
        .unwrap();
    let row = report.rows()[0].id();
    let report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::Price(row, Some("123.45".parse().unwrap())),
            &snapshot,
        )
        .unwrap();
    let report = app
        .confirm_price(report.id(), report.version(), row, &snapshot)
        .unwrap();
    let approval = report.rows()[0].price_confirmation();
    assert!(approval.is_some());
    let report = app
        .edit_batch(
            report.id(),
            report.version(),
            &[
                ManualEdit::Price(row, Some("123.45".parse().unwrap())),
                ManualEdit::Stock(row, Some("2.5".parse().unwrap())),
            ],
            &snapshot,
        )
        .unwrap();
    assert_eq!(report.rows()[0].price_confirmation(), approval);
    let report = app
        .edit_batch(
            report.id(),
            report.version(),
            &[
                ManualEdit::Price(row, Some("900".parse().unwrap())),
                ManualEdit::Stock(row, Some("3.5".parse().unwrap())),
            ],
            &snapshot,
        )
        .unwrap();
    assert!(report.rows()[0].price_confirmation().is_none());
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_13__manual_affinity_is_optional_explicit_and_persisted() {
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository::default(), Fixed);
    let mut report = app.create(support::key().unwrap(), None).unwrap();
    assert_eq!(report.faction_affinity(), None);
    for value in ["-100", "0", "100"] {
        report = app
            .edit(
                report.id(),
                report.version(),
                ManualEdit::FactionAffinity(Some(value.parse().unwrap())),
                &snapshot,
            )
            .unwrap();
        assert_eq!(report.faction_affinity().unwrap().to_string(), value);
        assert_eq!(app.drafts().unwrap(), vec![report.clone()]);
    }
    report = app
        .edit(
            report.id(),
            report.version(),
            ManualEdit::FactionAffinity(None),
            &snapshot,
        )
        .unwrap();
    assert_eq!(report.faction_affinity(), None);
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_13__affinity_refuses_non_integer_or_out_of_range_input_and_stays_bound_to_its_terminal() {
    use tallyline_game::api::model::GameEnvironment;
    use tallyline_reporting::api::model::{Report, ReportId, ReportKey};
    use tallyline_shared_kernel::api::model::{FactionAffinity, TerminalId, Timestamp, TradeSide};
    for input in ["-101", "101", "1.5", "1e1", "", "-", "1000"] {
        assert!(input.parse::<FactionAffinity>().is_err(), "{input}");
    }
    let mut report = Report::manual(
        ReportId(10),
        support::key().unwrap(),
        Timestamp::from_seconds(100),
        None,
    )
    .unwrap();
    report = report
        .set_faction_affinity(Some("-25".parse().unwrap()))
        .unwrap();
    report
        .change_key(ReportKey::new(TerminalId(11), TradeSide::Sell, GameEnvironment::Ptu).unwrap())
        .unwrap();
    assert_eq!(report.faction_affinity().unwrap().value(), -25);
    report
        .change_key(ReportKey::new(TerminalId(12), TradeSide::Sell, GameEnvironment::Ptu).unwrap())
        .unwrap();
    assert_eq!(report.faction_affinity(), None);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_13__affinity_commands_preserve_the_original_report_and_unchanged_version() {
    use tallyline_reporting::api::model::{Report, ReportId};
    use tallyline_shared_kernel::api::model::Timestamp;
    let original = Report::manual(
        ReportId(10),
        support::key().unwrap(),
        Timestamp::from_seconds(100),
        None,
    )
    .unwrap();
    let original_version = original.version();
    let changed = original
        .set_faction_affinity(Some("25".parse().unwrap()))
        .unwrap();
    assert_eq!(original.faction_affinity(), None);
    assert_eq!(original.version(), original_version);
    assert_eq!(changed.faction_affinity().unwrap().value(), 25);
    assert_eq!(changed.version(), original_version + 1);
    let unchanged = changed
        .set_faction_affinity(changed.faction_affinity())
        .unwrap();
    assert_eq!(unchanged, changed);
    let cleared = changed.set_faction_affinity(None).unwrap();
    assert_eq!(cleared.faction_affinity(), None);
    assert_eq!(cleared.version(), original_version + 2);
    assert_eq!(changed.faction_affinity().unwrap().value(), 25);
    assert_eq!(changed.version(), original_version + 1);
}
