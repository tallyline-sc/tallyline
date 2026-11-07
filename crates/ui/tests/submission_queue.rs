// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn immutable queue modes and cancellation limits.

use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    JobId, PayloadHash, SubmissionHeader, SubmissionJob, SubmissionRequest, SubmissionRow,
};
use tallyline_submission::api::model::{
    NotProcessed, SendReturnReason, SubmissionAccountCheck, SubmissionCorrection,
    SubmissionPauseReason, SubmissionRejection, SubmissionResult, SubmissionServiceCause,
    SubmissionServiceFence,
};
use tallyline_ui::api::model::{QueueCommand, SubmissionQueue};
use tallyline_ui::api::{Catalogue, show_submission_queue};
mod submission_support;
use submission_support::font_files;
fn queued(id: u64, mode: SubmissionMode) -> Option<SubmissionJob> {
    SubmissionJob::queued(
        JobId(id),
        SubmissionRequest::new(
            SubmissionHeader {
                report: ReportId(id + 100),
                report_version: 3,
                account: "test-account".to_owned(),
                mode,
                terminal: TerminalId(89),
                side: TradeSide::Sell,
                environment: UexEnvironment::Live,
                game_version: "4.10.2".to_owned(),
                faction_affinity: None,
                observed_at: Timestamp::from_seconds(100),
            },
            vec![
                SubmissionRow::new(
                    RowId(1),
                    CommodityId(7),
                    "120".parse().ok()?,
                    "0".parse().ok()?,
                    1,
                )
                .ok()?,
            ],
        )
        .ok()?,
    )
    .ok()
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_ui_16__drawn_queue_keeps_mode_account_and_sending_cancel_limit_visible() {
    let first = queued(1, SubmissionMode::DryRun).unwrap();
    let sending = queued(2, SubmissionMode::Production)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap();
    let mut state = SubmissionQueue::default();
    let returned = queued(3, SubmissionMode::DryRun)
        .unwrap()
        .return_for_review(SendReturnReason::GameVersionChanged)
        .unwrap();
    let held = queued(4, SubmissionMode::Production)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::NotProcessed(NotProcessed::ConnectFailure),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    state.received(vec![first, sending, returned, held]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 1000.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) = show_submission_queue(
                    ui,
                    &mut state.0,
                    &catalogue,
                    tallyline_egui::Theme::Light,
                ) {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    harness.get_by_label("Report 101 · Submission 1 · test-account");
    assert_eq!(harness.query_all_by_label("Dry run").count(), 2);
    assert_eq!(harness.query_all_by_label("Production").count(), 2);
    harness.get_by_label("Sending");
    harness.get_by_label("The connection failed before sending. The queue retries with bounded backoff; Retry now is an explicit action.");
    harness.get_by_label(
        "The game version changed. Review the captured version before releasing the Report again.",
    );
    assert!(
        harness
            .get_by_label("Cancel submission 2")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_label("Cancel submission 1").focus();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().1,
        vec![QueueCommand::Cancel {
            id: JobId(1),
            expected_revision: 1
        }]
    );
    assert!(
        harness
            .get_by_label("Cancel submission 1")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__stale_queue_results_cannot_reopen_sending_or_unknown_for_cancellation() {
    let first = queued(1, SubmissionMode::DryRun).unwrap();
    let sending = first
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![first.clone()]);
    assert!(state.request_cancel(JobId(1)));
    state.received(vec![sending.clone()]);
    assert!(state.take_command().is_none());
    state.received(vec![first]);
    assert_eq!(state.jobs()[0], sending);
    assert!(!state.request_cancel(JobId(1)));
    let unknown = sending.recover(Timestamp::from_seconds(120)).unwrap();
    state.received(vec![unknown.clone(), sending]);
    assert_eq!(state.jobs()[0], unknown);
    assert!(!state.request_cancel(JobId(1)));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_ui_16__failed_cancel_retains_committed_state_and_only_exact_feedback_unlocks_the_action()
 {
    use tallyline_submission::api::model::{SubmissionApplicationError, SubmissionRepositoryError};
    let first = queued(1, SubmissionMode::DryRun).unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![first.clone()]);
    assert!(state.request_cancel(JobId(1)));
    assert_eq!(
        state.take_command(),
        Some(QueueCommand::Cancel {
            id: JobId(1),
            expected_revision: 1
        })
    );
    let failure = SubmissionApplicationError::Repository(SubmissionRepositoryError::Unavailable);
    state.failed(JobId(2), 1, failure);
    assert!(!state.can_cancel(JobId(1)));
    state.failed(JobId(1), 2, failure);
    assert!(!state.can_cancel(JobId(1)));
    state.failed(JobId(1), 1, failure);
    assert!(state.can_cancel(JobId(1)));
    assert_eq!(state.jobs(), &[first]);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__queue_publishes_committed_receipt_mapping_only_for_its_exact_successful_job() {
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::model::{
        SubmissionResult, VerifiedReceiptMapping, VerifiedUexReport,
    };
    let queued = queued(4, SubmissionMode::Production).unwrap();
    let accepted = queued
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(100)).unwrap(),
            ]),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    let receipt = VerifiedUexReport::from_lookup(
        42,
        TerminalId(89),
        CommodityId(7),
        Timestamp::from_seconds(100),
        accepted.request(),
    )
    .unwrap();
    let mapping = VerifiedReceiptMapping::new(&accepted, vec![receipt]).unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![queued.clone()]);
    assert!(!state.received_mapping(mapping.clone()));
    assert!(state.verified_receipts(JobId(4)).is_none());
    state.received(vec![accepted.clone()]);
    assert!(state.received_mapping(mapping));
    assert_eq!(state.verified_receipts(JobId(4)).unwrap()[0].id(), 42);
    state.received(vec![queued]);
    assert_eq!(state.jobs()[0], accepted);
    assert_eq!(state.verified_receipts(JobId(4)).unwrap()[0].id(), 42);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_ui_1_q_reports__drawn_waits_show_exact_cooldown_budget_and_account_reason_and_ignore_stale_sending_views()
 {
    use tallyline_submission::api::model::{SubmissionWait, SubmissionWaitReason};
    let first = queued(11, SubmissionMode::DryRun).unwrap();
    let second = queued(12, SubmissionMode::Production).unwrap();
    let third = queued(13, SubmissionMode::Production).unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![first.clone(), second.clone(), third.clone()]);
    state.received_waiting(vec![
        SubmissionWait {
            job: first.id(),
            revision: first.revision(),
            reason: SubmissionWaitReason::CooldownUntil(Timestamp::from_seconds(300)),
            next_check: None,
        },
        SubmissionWait {
            job: second.id(),
            revision: second.revision(),
            reason: SubmissionWaitReason::RowBudget,
            next_check: None,
        },
        SubmissionWait {
            job: third.id(),
            revision: third.revision(),
            reason: SubmissionWaitReason::AccountChanged,
            next_check: None,
        },
    ]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 1600.0))
        .build_ui_state(
            move |ui, state: &mut SubmissionQueue| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_submission_queue(ui, state, &catalogue, tallyline_egui::Theme::Light);
            },
            state,
        );
    harness.run();
    harness.get_by_label("Waiting for cooldown until 1970-01-01 00:05:00 UTC");
    harness.get_by_label("Waiting for the complete Report to fit the 1,000-row production window.");
    harness.get_by_label("Paused: another account is active. Keep pending for the released account or cancel and review.");
    let sending = first
        .begin(Timestamp::from_seconds(301), PayloadHash::new([1; 32]))
        .unwrap();
    harness.state_mut().received(vec![sending]);
    harness.state_mut().received_waiting(vec![SubmissionWait {
        job: first.id(),
        revision: first.revision(),
        reason: SubmissionWaitReason::CooldownUntil(Timestamp::from_seconds(300)),
        next_check: None,
    }]);
    harness.run();
    assert!(
        harness
            .query_by_label("Waiting for cooldown until 1970-01-01 00:05:00 UTC")
            .is_none()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_12_r_ui_16__account_switch_offers_keep_or_review_without_rebinding_jobs_or_opening_a_dialog()
 {
    use tallyline_submission::api::model::{AccountState, Persistence};
    use tallyline_ui::api::model::{BackgroundEvent, SubmissionUpdate, View};
    use tallyline_ui::api::{ShellState, show_submission_account_choice};
    let job = queued(31, SubmissionMode::Production).unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(
        tallyline_workflows::api::model::AppSettings::default(),
    ));
    let account = |name: &str, at| BackgroundEvent::CredentialsUpdated {
        account: AccountState::Validated {
            username: name.to_owned(),
            validated_at: Timestamp::from_seconds(at),
            persistence: Persistence::Session,
        },
        failure: None,
        available_token: true,
        fallback_available: false,
    };
    shell.receive(account("test-account", 1000));
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
            waiting: vec![],
            mappings: vec![],
            reports: vec![],
            feedback: None,
            background_feedback: None,
            completed: false,
        },
    )));
    shell.show_settings();
    shell.receive(account("other-account", 1001));
    assert_eq!(shell.view(), View::Settings);
    assert!(!shell.dialogs().about_open());
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 500.0))
        .build_ui_state(
            move |ui, state: &mut ShellState| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_submission_account_choice(ui, state, &catalogue);
            },
            shell,
        );
    harness.run();
    harness.get_by_label("Queued submissions belong to another account. Keep them for their released accounts or review and cancel them explicitly.");
    harness.get_by_label("Review queued submissions").click();
    harness.run();
    assert_eq!(harness.state().view(), View::Queue);
    harness
        .get_by_label("Keep pending for their released accounts")
        .click();
    harness.run();
    assert_eq!(harness.state().submission_jobs(), &[job]);
    assert!(harness.state_mut().take_submission_command().is_none());
    harness.state_mut().receive(account("other-account", 1002));
    harness.run();
    assert!(
        harness
            .query_by_label("Keep pending for their released accounts")
            .is_none()
    );
    assert_eq!(harness.state().view(), View::Queue);
    assert!(!harness.state().dialogs().about_open());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_ui_16__held_job_offers_a_keyboard_retry_for_its_exact_displayed_revision() {
    safe_code_presentation().unwrap();
    let held = queued(1, SubmissionMode::DryRun)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::NotProcessed(NotProcessed::ServiceUnavailable),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![held]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) = show_submission_queue(
                    ui,
                    &mut state.0,
                    &catalogue,
                    tallyline_egui::Theme::Light,
                ) {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    harness.get_by_label("Next retry/check: 1970-01-01 00:02:00 UTC");
    harness.get_by_label("Retry now 1").focus();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        format!("{:?}", harness.state().1),
        "[Retry { id: SubmissionJobId(1), expected_revision: 3 }]"
    );
    assert!(
        harness
            .get_by_label("Retry now 1")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_ui_16__durable_global_quota_reason_is_visible_without_an_active_report() {
    let mut state = SubmissionQueue::default();
    state.received_service_fences(vec![
        SubmissionServiceFence::new(SubmissionServiceCause::DailyQuota, JobId(17), 3).unwrap(),
    ]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 500.0))
        .build_ui_state(
            move |ui, state: &mut SubmissionQueue| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_submission_queue(ui, state, &catalogue, tallyline_egui::Theme::Light);
            },
            state,
        );
    harness.run();
    harness.get_by_label("Requests using this app token are held: its daily quota is exhausted.");
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_ui_16__updated_release_offers_queue_acknowledgement_without_replaying_cancelled_report()
 {
    let held = queued(1, SubmissionMode::DryRun)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::VersionLockedOut(
                tallyline_shared_kernel::api::model::ReleaseVersion::parse("v1.2.0"),
            ),
            Timestamp::from_seconds(115),
        )
        .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![held.cancel().unwrap()]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .build_ui_state(
            move |ui, state: &mut (SubmissionQueue, Vec<QueueCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                if let Some(command) = show_submission_queue(
                    ui,
                    &mut state.0,
                    &catalogue,
                    tallyline_egui::Theme::Light,
                ) {
                    state.1.push(command);
                }
            },
            (state, Vec::new()),
        );
    harness.run();
    assert!(harness.query_by_label("Retry queue now 1").is_none());
    harness.state_mut().0.received_client_version(
        tallyline_shared_kernel::api::model::ReleaseVersion::parse("v1.2.1"),
    );
    harness.run();
    harness.get_by_label("Retry queue now 1").focus();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        format!("{:?}", harness.state().1),
        "[Retry { id: SubmissionJobId(1), expected_revision: 4 }]"
    );
    assert!(
        harness
            .get_by_label("Retry queue now 1")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}

fn safe_code_presentation() -> Result<(), Box<dyn std::error::Error>> {
    use tallyline_submission::api::model::{SubmissionResult, UexErrorCode};
    for (code, correction) in [
        ("Future_Code", SubmissionCorrection::ReviewClient),
        ("future-code", SubmissionCorrection::ReviewClient),
        ("invalid_secret_key", SubmissionCorrection::ReviewClient),
        ("invalid_game_version", SubmissionCorrection::NewCapture),
        ("invalid_game_version", SubmissionCorrection::ReviewClient),
    ] {
        let rejected = queued(1, SubmissionMode::DryRun)
            .ok_or("fixture job")?
            .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?
            .complete(
                if code == "invalid_secret_key" {
                    SubmissionResult::AccountRefusal {
                        code: UexErrorCode::new(code.to_owned()),
                        http_status: 400,
                        reason: SubmissionPauseReason::KeyCheckUnavailable,
                        check: SubmissionAccountCheck::Pending,
                        outcome: Box::new(SubmissionResult::Paused(
                            SubmissionPauseReason::KeyCheckUnavailable,
                        )),
                    }
                } else {
                    SubmissionResult::Rejected(SubmissionRejection::new(
                        UexErrorCode::new(code.to_owned()).ok_or("fixture code")?,
                        correction,
                    ))
                },
                Timestamp::from_seconds(115),
            )
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        let mut state = SubmissionQueue::default();
        state.received(vec![rejected]);
        let catalogue = Catalogue::english()?;
        let mut fonts = Some(font_files()?);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(900.0, 600.0))
            .build_ui_state(
                move |ui, state: &mut SubmissionQueue| {
                    if let Some(fonts) = fonts.take() {
                        tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                    }
                    let _command =
                        show_submission_queue(ui, state, &catalogue, tallyline_egui::Theme::Light);
                },
                state,
            );
        harness.run();
        if code == "invalid_secret_key" {
            harness.get_by_label("A read-only account check is pending. Sending remains paused.");
        } else {
            harness.get_by_label(&format!("UEX permanently refused this Report: {code}"));
            harness.get_by_label(if correction == SubmissionCorrection::NewCapture { "UEX no longer accepts the captured game version. Capture a new observation with a valid game version. This Report cannot be retried or duplicated with its captured version." } else { "Update the app or report this technical code with the app version. Review the original data before preparing another Draft." });
        }
        assert!(harness.query_by_label("Retry now 1").is_none());
    }
    Ok(())
}

#[path = "submission_queue/unknown_resolution.rs"]
mod unknown_resolution;

#[path = "submission_queue/partial_resolution.rs"]
mod partial_resolution;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11__authenticated_quota_shows_bounded_reset_check_without_a_report() {
    let mut state = SubmissionQueue::default();
    state.received_authenticated_quotas(vec![
        tallyline_submission::api::model::TokenQuotaObservation::new(
            None,
            Some(tallyline_submission::api::model::TokenQuotaReset::new(
                Timestamp::from_seconds(1010),
                Some(Timestamp::from_seconds(1000)),
            )),
        ),
    ]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 500.0))
        .build_ui_state(
            move |ui, state: &mut SubmissionQueue| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_submission_queue(ui, state, &catalogue, tallyline_egui::Theme::Light);
            },
            state,
        );
    harness.run();
    harness.get_by_label("Requests using this app token are held: its daily quota is exhausted.");
    harness.get_by_label("Quota reset check: 1970-01-01 00:16:50 UTC");
    harness.get_by_label("Sending still requires all current checks to pass.");
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11__quota_attempt_limit_offers_cancel_for_review_without_a_retry_bypass() {
    let mut job = queued(1, SubmissionMode::DryRun).unwrap();
    for (at, reset) in [(1000, 1010), (1010, 1020), (1020, 1030)] {
        job = job
            .begin(Timestamp::from_seconds(at), PayloadHash::new([1; 32]))
            .unwrap()
            .complete(
                SubmissionResult::NotProcessed(NotProcessed::DailyQuotaUntil(
                    tallyline_submission::api::model::TokenQuotaReset::new(
                        Timestamp::from_seconds(reset),
                        Some(Timestamp::from_seconds(at)),
                    ),
                )),
                Timestamp::from_seconds(at),
            )
            .unwrap();
        if reset < 1030 {
            job = job.resume_quota(Timestamp::from_seconds(reset)).unwrap();
        }
    }
    let mut state = SubmissionQueue::default();
    state.received(vec![job]);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .build_ui_state(
            move |ui, state: &mut SubmissionQueue| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_submission_queue(ui, state, &catalogue, tallyline_egui::Theme::Light);
            },
            state,
        );
    harness.run();
    harness.get_by_label(
        "Automatic attempts are exhausted. Cancel this report to review and release it again.",
    );
    assert!(harness.query_by_label("Retry now 1").is_none());
    assert!(
        !harness
            .get_by_label("Cancel submission 1")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_ui_16__drawn_mixed_split_shows_both_groups_and_emits_only_keyboard_version_bound_intent()
 {
    use tallyline_submission::api::model::{SubmissionWait, SubmissionWaitReason};
    let prior = queued(71, SubmissionMode::DryRun)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(115),
        )
        .unwrap();
    let template = queued(72, SubmissionMode::DryRun).unwrap();
    let first = template.request().rows()[0].clone();
    let second = SubmissionRow::new(
        RowId(2),
        CommodityId(8),
        first.price(),
        first.stock(),
        first.status(),
    )
    .unwrap();
    let candidate = SubmissionJob::queued(
        template.id(),
        SubmissionRequest::new(template.request().header().clone(), vec![first, second]).unwrap(),
    )
    .unwrap();
    let mut state = SubmissionQueue::default();
    state.received(vec![prior, candidate.clone()]);
    state.received_waiting(vec![SubmissionWait {
        job: candidate.id(),
        revision: candidate.revision(),
        reason: SubmissionWaitReason::CooldownUntil(Timestamp::from_seconds(415)),
        next_check: None,
    }]);
    state.received_schedule_time(Timestamp::from_seconds(200));
    assert!(state.can_split(candidate.id()));
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder().build_ui_state(
        move |ui, (state, command): &mut (SubmissionQueue, Option<QueueCommand>)| {
            if let Some(fonts) = fonts.take() {
                tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
            }
            let next = show_submission_queue(ui, state, &catalogue, tallyline_egui::Theme::Light);
            if next.is_some() {
                *command = next;
            }
        },
        (state, None),
    );
    harness.run();
    harness.get_by_label("Rows ready now: 2");
    harness.get_by_label("Rows waiting for cooldown: 1");
    let label = "Send the others now 72";
    harness.get_by_label(label).focus();
    harness.run();
    assert!(harness.get_by_label(label).is_focused());
    assert!(harness.state().0.can_split(candidate.id()));
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(!harness.state().0.can_split(candidate.id()));
    assert_eq!(
        harness.state().1,
        Some(QueueCommand::Split {
            id: candidate.id(),
            expected_revision: candidate.revision()
        })
    );
    assert_eq!(
        harness
            .state()
            .0
            .jobs()
            .iter()
            .find(|job| job.id() == candidate.id())
            .unwrap(),
        &candidate
    );
    harness.state_mut().0.received(vec![
        candidate
            .begin(Timestamp::from_seconds(200), PayloadHash::new([3; 32]))
            .unwrap(),
    ]);
    harness.run();
    assert!(harness.query_by_label(label).is_none());
}

#[path = "submission_queue/rejected_copy.rs"]
mod rejected_copy;

#[path = "submission_queue/lifecycle.rs"]
mod lifecycle;
