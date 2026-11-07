// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit status decisions through the Reporting application.

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
fn r_ui_10_q_manual_r_ui_12_q_manual__explicit_status_confirmation_is_value_bound_and_saved() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let expected = report.version();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let reference = snapshot(UexEnvironment::Live).unwrap();
    let confirmed = app
        .confirm_status(ReportId(1), expected, row, &reference)
        .unwrap();
    assert_eq!(
        confirmed.rows()[0]
            .status_confirmation()
            .unwrap()
            .value()
            .get(),
        5
    );
    assert_eq!(
        confirmed.rows()[0]
            .status_confirmation()
            .unwrap()
            .deviation(),
        Deviation::Major
    );
    assert_eq!(app.drafts().unwrap(), vec![confirmed.clone()]);
    assert_eq!(
        app.confirm_status(ReportId(1), expected, row, &reference),
        Err(RepositoryError::Conflict)
    );
    let mut changed = confirmed.clone();
    changed
        .set_status(row, std::num::NonZeroU8::new(4))
        .unwrap();
    assert!(changed.rows()[0].status_confirmation().is_none());
    assert_eq!(confirmed.rows()[0].status().unwrap().get(), 5);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_12_q_manual__reassessment_revokes_only_worsening_status_approvals_without_changing_values()
 {
    use tallyline_reference_data::api::model::{
        EndpointData, PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    };
    use tallyline_shared_kernel::api::model::{TerminalId, TradeSide};
    let reference = |status: Option<u8>| {
        support::cached_snapshot(
            UexEnvironment::Live,
            vec![EndpointData::TerminalPriors(
                TerminalPriors::new(
                    TerminalId(11),
                    status
                        .map(|status| {
                            PricePrior::new(
                                CommodityId(42),
                                TradeSide::Buy,
                                PriorValues {
                                    price: "10".parse().unwrap(),
                                    stock: "0".parse().unwrap(),
                                    status: std::num::NonZeroU8::new(status),
                                    container_sizes: vec![],
                                },
                                PriorProvenance {
                                    game_version: "4.10.2".to_owned(),
                                    modified_at: Timestamp::from_seconds(900),
                                },
                            )
                            .unwrap()
                        })
                        .into_iter()
                        .collect(),
                )
                .unwrap(),
            )],
        )
    };
    let cases = [
        (Some(3), Deviation::Equal, [false, true, true, false]),
        (Some(2), Deviation::Minor, [false, false, true, false]),
        (Some(1), Deviation::Major, [false, false, false, false]),
        (None, Deviation::NoReference, [false, true, true, false]),
    ];
    for (old_prior, old_level, expected_worse) in cases {
        for (index, (new_prior, new_level, _)) in cases.into_iter().enumerate() {
            let worse = expected_worse[index];
            let mut report = Report::manual(
                ReportId(1),
                key().unwrap(),
                Timestamp::from_seconds(900),
                None,
            )
            .unwrap();
            let row = report.add_commodity(CommodityId(42)).unwrap();
            report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
            let expected = report.version();
            let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
            let confirmed = app
                .confirm_status(ReportId(1), expected, row, &reference(old_prior))
                .unwrap();
            let reassessed = app
                .reassess_statuses(ReportId(1), confirmed.version(), &reference(new_prior))
                .unwrap();
            assert_eq!(reassessed.rows()[0].status().unwrap().get(), 3);
            assert_eq!(
                reassessed.rows()[0].status_confirmation().is_none(),
                worse,
                "{old_level:?} -> {new_level:?}"
            );
            assert_eq!(reassessed.rows()[0].status_unreviewed(), worse);
            assert_eq!(app.drafts().unwrap(), vec![reassessed]);
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__keyboard_focus_clears_unreviewed_but_never_approves_major_status() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let expected = report.version();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let reference = snapshot(UexEnvironment::Live).unwrap();
    let assessed = app
        .reassess_statuses(ReportId(1), expected, &reference)
        .unwrap();
    assert!(assessed.rows()[0].status_unreviewed());
    assert!(assessed.rows()[0].status_needs_confirmation());
    let focused = app
        .focus_status(ReportId(1), assessed.version(), row)
        .unwrap();
    assert!(!focused.rows()[0].status_unreviewed());
    assert!(focused.rows()[0].status_needs_confirmation());
    let repeated = app
        .reassess_statuses(ReportId(1), focused.version(), &reference)
        .unwrap();
    assert_eq!(repeated, focused);
    let confirmed = app
        .confirm_status(ReportId(1), focused.version(), row, &reference)
        .unwrap();
    assert!(!confirmed.rows()[0].status_needs_confirmation());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__status_approvals_survive_equal_edits_but_never_a_changed_report_context() {
    use tallyline_game::api::model::GameEnvironment;
    use tallyline_reporting::api::model::ReportKey;
    use tallyline_shared_kernel::api::model::{RowId, TerminalId, TradeSide};
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let expected = report.version();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let reference = snapshot(UexEnvironment::Live).unwrap();
    let confirmed = app
        .confirm_status(ReportId(1), expected, row, &reference)
        .unwrap();
    for context in [
        ReportKey::new(TerminalId(12), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        ReportKey::new(TerminalId(11), TradeSide::Sell, GameEnvironment::Live).unwrap(),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Ptu).unwrap(),
    ] {
        let mut changed = confirmed.clone();
        changed.change_key(context).unwrap();
        assert!(changed.rows()[0].status_confirmation().is_none());
        assert_eq!(changed.rows()[0].status().unwrap().get(), 5);
        assert_eq!(changed.rows()[0].status_deviation(), None);
    }
    let mut equal = confirmed.clone();
    equal.change_key(confirmed.key()).unwrap();
    equal.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    assert_eq!(
        equal.rows()[0].status_confirmation(),
        confirmed.rows()[0].status_confirmation()
    );
    equal.set_status(row, None).unwrap();
    assert!(equal.rows()[0].status_confirmation().is_none());
    assert!(!equal.rows()[0].status_unreviewed());
    assert_eq!(equal.rows()[0].status_deviation(), None);
    assert_eq!(
        app.confirm_status(ReportId(1), confirmed.version(), RowId(99), &reference),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.focus_status(ReportId(1), expected, row),
        Err(RepositoryError::Conflict)
    );
    assert_eq!(
        app.focus_status(ReportId(1), confirmed.version(), RowId(99)),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.reassess_statuses(ReportId(1), expected, &reference),
        Err(RepositoryError::Conflict)
    );
    assert_eq!(
        app.reassess_statuses(
            ReportId(1),
            confirmed.version(),
            &snapshot(UexEnvironment::Ptu).unwrap()
        ),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(app.drafts().unwrap(), vec![confirmed]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_12_q_manual__revoked_minor_confirmation_remains_a_decision_after_focus_and_restart_state()
 {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(4)).unwrap();
    let version = report.version();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let empty = support::cached_snapshot(UexEnvironment::Live, vec![]);
    let confirmed = app
        .confirm_status(ReportId(1), version, row, &empty)
        .unwrap();
    let reference = snapshot(UexEnvironment::Live).unwrap();
    let revoked = app
        .reassess_statuses(ReportId(1), confirmed.version(), &reference)
        .unwrap();
    assert_eq!(revoked.rows()[0].status_deviation(), Some(Deviation::Minor));
    assert!(revoked.rows()[0].status_needs_confirmation());
    assert!(revoked.rows()[0].status_confirmation_revoked());
    let focused = app
        .focus_status(ReportId(1), revoked.version(), row)
        .unwrap();
    assert!(focused.rows()[0].status_needs_confirmation());
    assert!(!focused.rows()[0].status_unreviewed());
    let confirmed = app
        .confirm_status(ReportId(1), focused.version(), row, &reference)
        .unwrap();
    assert!(!confirmed.rows()[0].status_needs_confirmation());
    assert!(!confirmed.rows()[0].status_confirmation_revoked());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__unreviewed_clears_when_an_unconfirmed_major_status_no_longer_needs_a_decision()
{
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let version = report.version();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let assessed = app
        .reassess_statuses(
            ReportId(1),
            version,
            &snapshot(UexEnvironment::Live).unwrap(),
        )
        .unwrap();
    assert!(assessed.rows()[0].status_unreviewed());
    let missing = support::cached_snapshot(UexEnvironment::Live, vec![]);
    let improved = app
        .reassess_statuses(ReportId(1), assessed.version(), &missing)
        .unwrap();
    assert!(!improved.rows()[0].status_unreviewed());
    assert!(!improved.rows()[0].status_needs_confirmation());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_6_r_ui_12_q_manual__mapping_changes_invalidate_status_approval_even_without_worsening() {
    use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let version = report.version();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let confirmed = app
        .confirm_status(
            ReportId(1),
            version,
            row,
            &snapshot(UexEnvironment::Live).unwrap(),
        )
        .unwrap();
    app.set_mapping(
        EnvironmentMapping::default().with_environment(GameEnvironment::Live, UexEnvironment::Ptu),
    );
    let changed = app
        .reassess_statuses(
            ReportId(1),
            confirmed.version(),
            &snapshot(UexEnvironment::Ptu).unwrap(),
        )
        .unwrap();
    assert!(changed.rows()[0].status_confirmation().is_none());
    assert!(changed.rows()[0].status_needs_confirmation());
    assert!(changed.rows()[0].status_unreviewed());
    assert_eq!(changed.rows()[0].status().unwrap().get(), 3);
}

struct UnavailableWrites(Repository);
impl tallyline_reporting::api::ReportRepository for UnavailableWrites {
    fn create_manual(
        &mut self,
        key: tallyline_reporting::api::model::ReportKey,
        at: Timestamp,
        version: Option<&str>,
    ) -> Result<Report, RepositoryError> {
        self.0.create_manual(key, at, version)
    }
    fn load_draft(&mut self, id: ReportId) -> Result<Option<Report>, RepositoryError> {
        self.0.load_draft(id)
    }
    fn drafts(&mut self) -> Result<Vec<Report>, RepositoryError> {
        self.0.drafts()
    }
    fn save_draft(&mut self, _: &Report, _: u64) -> Result<(), RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual_r_nf_11__failed_decision_saves_never_publish_or_replace_the_durable_report() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    let version = report.version();
    let reference = snapshot(UexEnvironment::Live).unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report]), Fixed);
    let pending = app
        .reassess_statuses(ReportId(1), version, &reference)
        .unwrap();
    let mut app =
        ReportingApplication::new(UnavailableWrites(Repository(vec![pending.clone()])), Fixed);
    assert_eq!(
        app.confirm_status(ReportId(1), pending.version(), row, &reference),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(
        app.focus_status(ReportId(1), pending.version(), row),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(
        app.reassess_statuses(
            ReportId(1),
            pending.version(),
            &support::cached_snapshot(UexEnvironment::Live, vec![])
        ),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(
        app.focus_status(ReportId(99), 0, row),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(app.drafts().unwrap(), vec![pending]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__an_unsent_status_cannot_be_approved_or_focused_and_reassessment_does_not_write()
 {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    let version = report.version();
    let mut app =
        ReportingApplication::new(UnavailableWrites(Repository(vec![report.clone()])), Fixed);
    let reference = snapshot(UexEnvironment::Live).unwrap();
    assert_eq!(
        app.confirm_status(ReportId(1), version, row, &reference),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.focus_status(ReportId(1), version, row),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.reassess_statuses(ReportId(1), version, &reference)
            .unwrap(),
        report
    );
}
