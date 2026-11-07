// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Concurrent global notices keep their explicit actions reachable.

use egui_kittest::kittest::Queryable;
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    AccountState, JobId, Persistence, SubmissionHeader, SubmissionJob, SubmissionRequest,
    SubmissionRow,
};
use tallyline_ui::api::model::{BackgroundEvent, SettingsCommand, SubmissionUpdate, View};
use tallyline_workflows::api::model::{
    AppSettings, ReleaseVersion, Theme, UpdateChecks, UpdateStatus,
};

#[test]
fn concurrent_account_update_and_storage_notices_keep_actions_visible_at_double_scale() {
    for theme in [Theme::Dark, Theme::Light, Theme::HighContrast] {
        let mut harness = super::configured_shell(egui::vec2(800.0, 640.0), theme, 2.0).unwrap();
        harness
            .state_mut()
            .shell
            .receive(BackgroundEvent::SettingsLoaded(
                AppSettings::from_persisted(theme, false)
                    .with_update_state(UpdateChecks::Enabled, None),
            ));
        let job = SubmissionJob::queued(
            JobId(1),
            SubmissionRequest::new(
                SubmissionHeader {
                    report: ReportId(1),
                    report_version: 1,
                    account: "fixture-original".to_owned(),
                    mode: SubmissionMode::DryRun,
                    terminal: TerminalId(11),
                    side: TradeSide::Buy,
                    environment: UexEnvironment::Live,
                    game_version: "4.10.2".to_owned(),
                    faction_affinity: None,
                    observed_at: Timestamp::from_seconds(100),
                },
                vec![
                    SubmissionRow::new(
                        RowId(1),
                        CommodityId(42),
                        "1".parse().unwrap(),
                        "0".parse().unwrap(),
                        1,
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap();
        let job = job
            .begin(
                Timestamp::from_seconds(110),
                tallyline_submission::api::model::PayloadHash::new([1; 32]),
            )
            .unwrap()
            .complete(
                tallyline_submission::api::model::SubmissionResult::NotProcessed(
                    tallyline_submission::api::model::NotProcessed::ConnectFailure,
                ),
                Timestamp::from_seconds(115),
            )
            .unwrap();
        harness
            .state_mut()
            .shell
            .receive(BackgroundEvent::SubmissionUpdated(Box::new(
                SubmissionUpdate {
                    lifecycles: vec![],
                    lifecycle_busy: false,
                    lifecycle_account: None,
                    assessed_at: tallyline_shared_kernel::api::model::Timestamp::from_seconds(0),
                    client_version: None,
                    service_fences: vec![],
                    authenticated_quotas: vec![],
                    jobs: vec![job],
                    mappings: vec![],
                    waiting: vec![],
                    reports: vec![],
                    feedback: None,
                    background_feedback: None,
                    completed: false,
                },
            )));
        harness
            .state_mut()
            .shell
            .receive(BackgroundEvent::CredentialsUpdated {
                account: AccountState::Validated {
                    username: "fixture-current".to_owned(),
                    validated_at: Timestamp::from_seconds(120),
                    persistence: Persistence::Session,
                },
                failure: None,
                available_token: true,
                fallback_available: false,
            });
        harness
            .state_mut()
            .shell
            .receive(BackgroundEvent::UpdateChecked(UpdateStatus::Available(
                ReleaseVersion::parse("v2.0.1").unwrap(),
            )));
        harness.state_mut().shell.receive(BackgroundEvent::Failure(
            "Storage self-test failed".to_owned(),
        ));
        harness.run();
        assert!(harness.query_by_label("Held: 1").is_some());
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 320.0));
        for label in [
            "Review queued submissions",
            "Keep pending for their released accounts",
            "Dismiss update notice",
        ] {
            harness
                .get_by_role_and_label(egui::Role::Button, label)
                .focus();
            harness.run();
            let action = harness.get_by_role_and_label(egui::Role::Button, label);
            assert!(action.is_focused());
            assert!(
                viewport.contains_rect(action.rect()),
                "{theme:?} {label}: {:?}",
                action.rect()
            );
            assert!(
                action.rect().bottom() <= harness.get_by_label("Saved submissions: 1").rect().top(),
                "Notice action stays above status"
            );
            let search = harness.query_by_label("Search terminals");
            if let Some(search) = search {
                assert!(
                    action.rect().bottom() <= search.rect().top(),
                    "Notice focus must stay in its own visible region: {label} {:?} {:?}",
                    action.rect(),
                    search.rect()
                );
            }
            harness.key_press(egui::Key::Space);
            harness.run();
        }
        assert_eq!(harness.state().shell.view(), View::Queue);
        assert!(
            harness
                .query_by_label("Review queued submissions")
                .is_none()
        );
        assert!(
            harness
                .state()
                .commands
                .contains(&SettingsCommand::DismissUpdate)
        );
        assert_eq!(
            harness.state().shell.notice(),
            Some("Storage self-test failed")
        );
        assert_eq!(harness.state().shell.submission_jobs().len(), 1);
        assert!(!harness.state().shell.dialogs().about_open());
    }
}
