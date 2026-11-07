// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Passive Draft publications cannot undo committed queue lifecycle or newer source versions.

use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::model::{Report, ReportId, ReportKey};
use tallyline_shared_kernel::api::model::{
    CommodityId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    JobId, SubmissionHeader, SubmissionJob, SubmissionRequest, SubmissionRow,
};
use tallyline_ui::api::ShellState;
use tallyline_ui::api::model::{BackgroundEvent, SubmissionUpdate};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9_r_ui_16__late_draft_lists_cannot_reopen_a_released_report_or_replace_its_renewed_draft()
 {
    let mut report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let job = SubmissionJob::queued(
        JobId(1),
        SubmissionRequest::new(
            SubmissionHeader {
                report: report.id(),
                report_version: report.version() + 2,
                account: "dummy-a".to_owned(),
                mode: SubmissionMode::DryRun,
                terminal: TerminalId(11),
                side: TradeSide::Buy,
                environment: UexEnvironment::Live,
                game_version: "4.10.2".to_owned(),
                faction_affinity: None,
                observed_at: report.observed_at(),
            },
            vec![
                SubmissionRow::new(
                    row,
                    CommodityId(42),
                    "123.45".parse().unwrap(),
                    "0".parse().unwrap(),
                    3,
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    shell.receive(BackgroundEvent::SubmissionUpdated(Box::new(
        SubmissionUpdate {
            lifecycles: vec![],
            lifecycle_busy: false,
            lifecycle_account: None,
            assessed_at: tallyline_shared_kernel::api::model::Timestamp::from_seconds(0),
            client_version: None,
            service_fences: vec![],
            authenticated_quotas: vec![],
            jobs: vec![job.clone()],
            mappings: vec![],
            waiting: vec![],
            reports: vec![],
            feedback: None,
            background_feedback: None,
            completed: false,
        },
    )));
    assert!(shell.manual().selected().is_none());
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    assert!(
        shell.manual().selected().is_none(),
        "a stale worker snapshot cannot reopen a queued Report"
    );
    let cancelled = job.cancel().unwrap();
    let mut renewed = report.clone();
    for value in ["1", "2", "0"] {
        renewed
            .set_stock(row, Some(value.parse().unwrap()))
            .unwrap();
    }
    assert!(renewed.version() > job.request().header().report_version);
    shell.receive(BackgroundEvent::SubmissionUpdated(Box::new(
        SubmissionUpdate {
            lifecycles: vec![],
            lifecycle_busy: false,
            lifecycle_account: None,
            assessed_at: tallyline_shared_kernel::api::model::Timestamp::from_seconds(0),
            client_version: None,
            service_fences: vec![],
            authenticated_quotas: vec![],
            jobs: vec![cancelled],
            mappings: vec![],
            waiting: vec![],
            reports: vec![renewed.clone()],
            feedback: None,
            background_feedback: None,
            completed: false,
        },
    )));
    assert_eq!(shell.manual().selected(), Some(&renewed));
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    assert_eq!(
        shell.manual().selected(),
        Some(&renewed),
        "late source versions cannot replace the renewed Draft"
    );
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![],
        selected: None,
    });
    assert_eq!(
        shell.manual().selected(),
        Some(&renewed),
        "an earlier complete list cannot erase a later committed Draft"
    );
}
