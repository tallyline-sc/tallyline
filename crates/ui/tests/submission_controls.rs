// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit manual and onboarding release intents through the drawn UI.

use egui_kittest::{Harness, kittest::Queryable};
use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::model::{ReleaseBlockReason, Report, ReportId, ReportKey};
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp, TradeSide};
use tallyline_submission::api::model::{AccountState, Persistence};
use tallyline_ui::api::model::{
    BackgroundEvent, BuildPresentation, SubmissionControl, SubmissionFeedback, SubmissionIntent,
    SubmissionUpdate,
};
use tallyline_ui::api::{Catalogue, ShellState, show_submission_controls};
use tallyline_workflows::api::model::{AppSettings, Theme};

mod submission_support;

fn account(name: &str) -> BackgroundEvent {
    BackgroundEvent::CredentialsUpdated {
        account: AccountState::Validated {
            username: name.to_owned(),
            validated_at: Timestamp::from_seconds(1000),
            persistence: Persistence::Session,
        },
        failure: None,
        available_token: true,
        fallback_available: false,
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_8_r_sub_9_r_ui_16__drawn_test_binds_displayed_account_and_passive_snapshots_do_not_complete_its_action()
 {
    let report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let mut shell = ShellState::default();
    shell.set_build_presentation(BuildPresentation::Release {
        version: "1.2.3".to_owned(),
        platform: "Windows".to_owned(),
        tag: "v1.2.3".to_owned(),
        commit: "a".repeat(40),
    });
    shell.receive(BackgroundEvent::SettingsLoaded(
        AppSettings::from_persisted(Theme::System, true),
    ));
    shell.receive(account("dummy-a"));
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    assert_eq!(shell.submission_mode(), SubmissionMode::Production);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(submission_support::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 1000.0))
        .build_ui_state(
            move |ui, shell: &mut ShellState| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_submission_controls(ui, shell, &catalogue);
            },
            shell,
        );
    harness.run();
    harness.get_by_label("Send test through queue").click();
    harness.run();
    harness.state_mut().receive(account("dummy-b"));
    let SubmissionControl::Release {
        id,
        expected,
        account,
        intent,
    } = harness.state_mut().take_submission_command().unwrap()
    else {
        panic!("release intent expected")
    };
    assert_eq!((id, expected), (report.id(), report.version()));
    assert_eq!(account, "dummy-a");
    assert_eq!(intent, SubmissionIntent::OnboardingTest);
    assert!(harness.state().submission_busy());
    harness
        .state_mut()
        .receive(BackgroundEvent::SubmissionUpdated(Box::new(
            SubmissionUpdate {
                lifecycles: vec![],
                lifecycle_busy: false,
                lifecycle_account: None,
                assessed_at: tallyline_shared_kernel::api::model::Timestamp::from_seconds(0),
                client_version: None,
                service_fences: vec![],
                authenticated_quotas: vec![],
                jobs: vec![],
                mappings: vec![],
                waiting: vec![],
                reports: vec![report.clone()],
                feedback: None,
                background_feedback: None,
                completed: false,
            },
        )));
    assert!(harness.state().submission_busy());
    harness
        .state_mut()
        .receive(BackgroundEvent::SubmissionUpdated(Box::new(
            SubmissionUpdate {
                lifecycles: vec![],
                lifecycle_busy: false,
                lifecycle_account: None,
                assessed_at: tallyline_shared_kernel::api::model::Timestamp::from_seconds(0),
                client_version: None,
                service_fences: vec![],
                authenticated_quotas: vec![],
                jobs: vec![],
                mappings: vec![],
                waiting: vec![],
                reports: vec![report.clone()],
                feedback: Some(SubmissionFeedback::Blocked(vec![
                    ReleaseBlockReason::ObservationAgeUnconfirmed,
                ])),
                background_feedback: None,
                completed: true,
            },
        )));
    assert!(!harness.state().submission_busy());
    assert_eq!(harness.state().manual().selected(), Some(&report));
    assert!(!harness.state().dialogs().about_open());
    harness.run();
    harness
        .get_by_label("Confirm unchanged observation age")
        .click();
    harness.run();
    assert!(
        matches!(harness.state_mut().take_submission_command(),Some(SubmissionControl::ConfirmAge{id,expected}) if id==report.id() && expected==report.version())
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_8_r_sub_8_r_ui_16__older_row_choice_shows_exact_values_and_keeps_the_failed_test_scope_after_global_mode_changes()
 {
    use tallyline_shared_kernel::api::model::{CommodityId, UexEnvironment};
    let mut report = Report::manual(
        ReportId(2),
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
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    shell.receive(account("dummy-a"));
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(submission_support::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 1200.0))
        .build_ui_state(
            move |ui, shell: &mut ShellState| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_submission_controls(ui, shell, &catalogue);
            },
            shell,
        );
    harness.run();
    harness.get_by_label("Send test through queue").click();
    harness.run();
    assert!(matches!(
        harness.state_mut().take_submission_command(),
        Some(SubmissionControl::Release {
            intent: SubmissionIntent::OnboardingTest,
            ..
        })
    ));
    harness
        .state_mut()
        .receive(BackgroundEvent::SubmissionUpdated(Box::new(
            SubmissionUpdate {
                lifecycles: vec![],
                lifecycle_busy: false,
                lifecycle_account: None,
                assessed_at: tallyline_shared_kernel::api::model::Timestamp::from_seconds(0),
                client_version: None,
                service_fences: vec![],
                authenticated_quotas: vec![],
                jobs: vec![],
                mappings: vec![],
                waiting: vec![],
                reports: vec![report.clone()],
                feedback: Some(SubmissionFeedback::Blocked(vec![
                    ReleaseBlockReason::NewerObservationSent {
                        row,
                        observed_at: Timestamp::from_seconds(950),
                    },
                ])),
                background_feedback: None,
                completed: true,
            },
        )));
    harness.state_mut().receive(BackgroundEvent::SettingsLoaded(
        AppSettings::default().with_submission_mode(Some(SubmissionMode::Production)),
    ));
    harness.run();
    harness.get_by_label("Row 1 · Commodity 42 · Price 123.45 · Stock 0 · Status 3");
    harness.get_by_label("Send older row 1 anyway").click();
    harness.run();
    let SubmissionControl::ConfirmNewer {
        id,
        expected,
        row: confirmed,
        evidence,
    } = harness.state_mut().take_submission_command().unwrap()
    else {
        panic!("explicit newer-row decision expected")
    };
    assert_eq!(
        (id, expected, confirmed),
        (report.id(), report.version(), row)
    );
    assert_eq!(evidence.account(), "dummy-a");
    assert_eq!(evidence.mode(), SubmissionMode::DryRun);
    assert_eq!(evidence.environment(), UexEnvironment::Live);
    assert_eq!(evidence.observed_at(), Timestamp::from_seconds(950));
    assert_eq!(harness.state().manual().selected(), Some(&report));
    assert!(!harness.state().dialogs().about_open());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_ui_16__committed_split_shows_two_released_reports_without_opening_a_dialog() {
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SubmissionUpdated(Box::new(
        tallyline_ui::api::model::SubmissionUpdate {
            lifecycles: vec![],
            lifecycle_busy: false,
            lifecycle_account: None,
            assessed_at: Timestamp::from_seconds(200),
            client_version: None,
            service_fences: vec![],
            authenticated_quotas: vec![],
            jobs: vec![],
            waiting: vec![],
            mappings: vec![],
            reports: vec![],
            completed: true,
            background_feedback: None,
            feedback: Some(tallyline_ui::api::model::SubmissionFeedback::Split),
        },
    )));
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(submission_support::font_files().unwrap());
    let mut harness = Harness::builder().build_ui_state(
        move |ui, shell: &mut ShellState| {
            if let Some(fonts) = fonts.take() {
                tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
            }
            show_submission_controls(ui, shell, &catalogue);
        },
        shell,
    );
    harness.run();
    harness.get_by_label("Split into two released Reports. Each repeats the normal send checks.");
    assert!(!harness.state().submission_busy());
    assert!(!harness.state().dialogs().about_open());
}
