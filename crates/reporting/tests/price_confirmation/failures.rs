// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Refusal and atomic publication at the numeric decision application seam.

use super::*;
use tallyline_reporting::api::ReportRepository;
use tallyline_reporting::api::model::{ReportKey, RowId};

pub(super) struct UnavailableWrites(pub(super) Repository);
impl ReportRepository for UnavailableWrites {
    fn create_manual(
        &mut self,
        key: ReportKey,
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
fn r_ui_12_q_manual_r_nf_11__failed_price_decision_saves_never_publish_a_new_report() {
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
    let pending = app
        .reassess_prices(ReportId(1), report.version(), &prior)
        .unwrap();
    let mut app =
        ReportingApplication::new(UnavailableWrites(Repository(vec![pending.clone()])), Fixed);
    assert_eq!(
        app.confirm_price(ReportId(1), pending.version(), row, &prior),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(
        app.focus_price(ReportId(1), pending.version(), row),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(
        app.reassess_prices(
            ReportId(1),
            pending.version(),
            &reference(Some("200")).unwrap()
        ),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(app.drafts().unwrap(), vec![pending]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual__price_commands_refuse_stale_missing_and_mismatched_contexts() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report.set_price(row, Some("100".parse().unwrap())).unwrap();
    let missing_price = report.add_commodity(CommodityId(43)).unwrap();
    let version = report.version();
    let mut app =
        ReportingApplication::new(UnavailableWrites(Repository(vec![report.clone()])), Fixed);
    let prior = reference(Some("100")).unwrap();
    for id in [ReportId(1), ReportId(99)] {
        let failure = if id == ReportId(1) {
            RepositoryError::Conflict
        } else {
            RepositoryError::Invalid
        };
        for expected in [version - 1, version + 1] {
            assert_eq!(app.confirm_price(id, expected, row, &prior), Err(failure));
            assert_eq!(app.reassess_prices(id, expected, &prior), Err(failure));
            assert_eq!(app.focus_price(id, expected, row), Err(failure));
            assert_eq!(app.review_manual(id, expected, &prior), Err(failure));
        }
    }
    for row in [RowId(99), missing_price] {
        assert_eq!(
            app.confirm_price(ReportId(1), version, row, &prior),
            Err(RepositoryError::Invalid)
        );
        assert_eq!(
            app.focus_price(ReportId(1), version, row),
            Err(RepositoryError::Invalid)
        );
    }
    let ptu = snapshot(UexEnvironment::Ptu).unwrap();
    assert_eq!(
        app.confirm_price(ReportId(1), version, row, &ptu),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.reassess_prices(ReportId(1), version, &ptu),
        Err(RepositoryError::Invalid)
    );
    assert_eq!(
        app.reassess_prices(ReportId(1), version, &prior),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_nf_11__combined_review_never_publishes_partial_or_failed_assessments() {
    let mut report = Report::manual(
        ReportId(1),
        key().unwrap(),
        Timestamp::from_seconds(900),
        None,
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    let prior = snapshot(UexEnvironment::Live).unwrap();
    let mut app =
        ReportingApplication::new(UnavailableWrites(Repository(vec![report.clone()])), Fixed);
    assert_eq!(
        app.review_manual(ReportId(1), report.version(), &prior)
            .unwrap()
            .report(),
        &report
    );
    report.set_status(row, std::num::NonZeroU8::new(5)).unwrap();
    report
        .set_price(row, Some("79228162514264337593543950335".parse().unwrap()))
        .unwrap();
    let mut app = ReportingApplication::new(Repository(vec![report.clone()]), Fixed);
    assert_eq!(
        app.review_manual(
            ReportId(1),
            report.version(),
            &reference(Some("0.0000000000000000000000000001")).unwrap()
        ),
        Err(RepositoryError::AssessmentUnavailable)
    );
    assert_eq!(app.drafts().unwrap(), vec![report.clone()]);
    let mut app =
        ReportingApplication::new(UnavailableWrites(Repository(vec![report.clone()])), Fixed);
    assert_eq!(
        app.review_manual(ReportId(1), report.version(), &prior),
        Err(RepositoryError::Unavailable)
    );
    assert_eq!(app.drafts().unwrap(), vec![report]);
}
