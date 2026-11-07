// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Numeric review through the composed manual editor.

use super::support;
use egui_kittest::{Harness, kittest::Queryable};
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::model::ReviewField;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_ui::api::model::{BackgroundEvent, ManualCommand, ManualReviewAction};
use tallyline_ui::api::{Catalogue, ShellState, show_manual_editor};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_10_q_manual_r_ui_11_q_manual_r_ui_12_q_manual_r_ui_16__manual_editor_passively_requests_review_and_keyboard_confirmation_is_explicit()
 {
    let (review, snapshot) = support::review().unwrap();
    let report = review.report().clone();
    let row = report.rows()[0].id();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: snapshot.clone(),
        ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
        observed_at: Timestamp::from_seconds(1000),
    });
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(support::fonts().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1900.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_manual_editor(ui, state, &catalogue);
            },
            shell,
        );
    harness.run();
    let Some(ManualCommand::Review(request)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected passive reassessment");
    };
    assert_eq!(request.action, ManualReviewAction::Reassess);
    assert_eq!(request.expected, report.version());
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: request.expected,
            review: (review.clone()).into(),
            snapshot: snapshot.clone(),
        });
    harness.run();
    assert!(!harness.state().dialogs().manual_confirmation_open());
    assert!(
        harness
            .query_by_label("Price, stock and status review")
            .is_some()
    );
    harness.key_press(egui::Key::F8);
    harness.run();
    let Some(ManualCommand::Review(focus)) = harness.state_mut().manual_mut().take_command() else {
        panic!("expected durable keyboard focus");
    };
    assert_eq!(
        focus.action,
        ManualReviewAction::Focus(ReviewField::Price(row))
    );
    let focused = support::execute(review, focus.clone()).unwrap();
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: focus.expected,
            review: (focused).into(),
            snapshot: snapshot.clone(),
        });
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(harness.state().dialogs().manual_confirmation_open());
    assert!(harness.state_mut().manual_mut().take_command().is_none());
    harness.get_by_label("Approve this exact value").focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    let Some(ManualCommand::Review(approval)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected explicit exact-value approval");
    };
    assert_eq!(
        approval.action,
        ManualReviewAction::Confirm(ReviewField::Price(row))
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual_r_ui_16__rapid_keyboard_focus_is_saved_after_the_pending_request_without_replacing_it()
 {
    let (review, snapshot) = support::review().unwrap();
    let report = review.report().clone();
    let row = report.rows()[0].id();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report],
        selected: Some(review.report().id()),
    });
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: snapshot.clone(),
        ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
        observed_at: Timestamp::from_seconds(1000),
    });
    let mut harness = support::harness(shell).unwrap();
    harness.run();
    let Some(ManualCommand::Review(request)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected initial review");
    };
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: request.expected,
            review: (review.clone()).into(),
            snapshot: snapshot.clone(),
        });
    harness.run();
    harness.key_press(egui::Key::F8);
    harness.run();
    let Some(ManualCommand::Review(first)) = harness.state_mut().manual_mut().take_command() else {
        panic!("expected first focus request");
    };
    harness.key_press(egui::Key::F8);
    harness.run();
    assert!(harness.state_mut().manual_mut().take_command().is_none());
    let focused = support::execute(review, first.clone()).unwrap();
    let version = focused.report().version();
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: first.expected,
            review: (focused).into(),
            snapshot,
        });
    harness.run();
    let Some(ManualCommand::Review(next)) = harness.state_mut().manual_mut().take_command() else {
        panic!("expected deferred explicit focus after acknowledgement");
    };
    assert_eq!(next.expected, version);
    assert_eq!(
        next.action,
        ManualReviewAction::Focus(ReviewField::Status(row))
    );
    assert!(!harness.state().dialogs().manual_confirmation_open());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_16__passive_reference_reassessment_keeps_existing_keyboard_focus_while_waiting()
 {
    let (review, snapshot) = support::review().unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![review.report().clone()],
        selected: Some(review.report().id()),
    });
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: snapshot.clone(),
        ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
        observed_at: Timestamp::from_seconds(1000),
    });
    let mut harness = support::harness(shell).unwrap();
    harness.run();
    let Some(ManualCommand::Review(initial)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected initial review");
    };
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: initial.expected,
            review: (review.clone()).into(),
            snapshot: snapshot.clone(),
        });
    harness.run();
    harness.key_press(egui::Key::F8);
    harness.run();
    let Some(ManualCommand::Review(focus)) = harness.state_mut().manual_mut().take_command() else {
        panic!("expected focus request");
    };
    let focused = support::execute(review, focus.clone()).unwrap();
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: focus.expected,
            review: (focused).into(),
            snapshot,
        });
    harness.run();
    let keyboard = harness.ctx.memory(|memory| memory.focused());
    assert!(keyboard.is_some());
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceUpdated {
            live: support::empty_snapshot(UexEnvironment::Live).unwrap(),
            ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
            observed_at: Timestamp::from_seconds(1100),
        });
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), keyboard);
    assert!(!harness.state().dialogs().manual_confirmation_open());
    assert!(
        matches!(harness.state_mut().manual_mut().take_command(), Some(ManualCommand::Review(request)) if request.action == ManualReviewAction::Reassess)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_12_q_manual_r_ui_16__failed_passive_review_waits_for_an_explicit_retry_without_losing_the_draft()
 {
    let (review, snapshot) = support::review().unwrap();
    let report = review.report().clone();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: snapshot,
        ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
        observed_at: Timestamp::from_seconds(1000),
    });
    let mut harness = support::harness(shell).unwrap();
    harness.run();
    let Some(ManualCommand::Review(request)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected initial review");
    };
    harness.state_mut().receive(BackgroundEvent::ReviewFailed {
        id: request.id,
        expected: request.expected,
        error: tallyline_reporting::api::RepositoryError::Unavailable,
    });
    harness.run();
    harness.run();
    assert!(harness.state_mut().manual_mut().take_command().is_none());
    assert_eq!(harness.state().manual().selected(), Some(&report));
    assert!(
        harness
            .query_by_label("Draft storage is unavailable")
            .is_some()
    );
    harness.get_by_label("Review latest saved values").focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(
        matches!(harness.state_mut().manual_mut().take_command(), Some(ManualCommand::Review(retry)) if retry.action == ManualReviewAction::Reassess && retry.expected == request.expected)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual_r_sub_7_q_manual__value_confirmation_shows_only_a_current_privacy_confirmed_private_preview()
 {
    use tallyline_reporting::api::AttachmentEdit;
    use tallyline_reporting::api::model::{
        AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash,
    };
    use tallyline_shared_kernel::api::model::ImageRaster;
    use tallyline_workflows::api::ManualAttachmentPreview;
    for confirmed in [false, true] {
        let (base, snapshot) = support::review().unwrap();
        let mut report = base.report().clone();
        let binding = AttachmentBinding::new(
            ImageHash::new([1; 32]),
            AttachmentRegion::new(0, 10, 4, 2).unwrap(),
        );
        report
            .edit_attachment(AttachmentEdit::Select(
                binding,
                AttachmentTime::User(Timestamp::from_seconds(900)),
            ))
            .unwrap();
        if confirmed {
            report
                .edit_attachment(AttachmentEdit::ConfirmPreview(binding))
                .unwrap();
        }
        let preview = ManualAttachmentPreview::new(
            binding,
            AttachmentTime::User(Timestamp::from_seconds(900)),
            ImageRaster::new(4, 2, vec![0xff00_c800; 8]).unwrap(),
        )
        .unwrap();
        let review = support::review_report(report.clone(), &snapshot).unwrap();
        let mut shell = ShellState::default();
        shell.receive(BackgroundEvent::ReportsLoaded {
            reports: vec![report.clone()],
            selected: Some(report.id()),
        });
        shell.receive(BackgroundEvent::AttachmentPreview {
            id: report.id(),
            expected: report.version(),
            preview,
        });
        shell.receive(BackgroundEvent::ReferenceUpdated {
            live: snapshot.clone(),
            ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
            observed_at: Timestamp::from_seconds(1000),
        });
        let mut harness = support::harness(shell).unwrap();
        harness.run();
        let Some(ManualCommand::Review(request)) = harness.state_mut().manual_mut().take_command()
        else {
            panic!("expected initial numeric review");
        };
        harness
            .state_mut()
            .receive(BackgroundEvent::ReportReviewed {
                expected: request.expected,
                review: review.into(),
                snapshot,
            });
        harness.run();
        harness
            .get_by_label("Confirm price for commodity 42")
            .click();
        harness.run();
        assert!(harness.state().dialogs().manual_confirmation_open());
        assert_eq!(
            harness
                .query_by_label("No private screenshot preview is available for this Draft.")
                .is_none(),
            confirmed
        );
        assert!(harness.state_mut().manual_mut().take_command().is_none());
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_16__invalid_uncommitted_optional_input_delays_passive_review_and_preserves_keyboard_focus()
 {
    let (review, snapshot) = support::review().unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![review.report().clone()],
        selected: Some(review.report().id()),
    });
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: snapshot.clone(),
        ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
        observed_at: Timestamp::from_seconds(1000),
    });
    let mut harness = support::harness(shell).unwrap();
    harness.run();
    let Some(ManualCommand::Review(request)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected initial review");
    };
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: request.expected,
            review: review.into(),
            snapshot,
        });
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::TextInput, "SCU for commodity 42 (SCU)")
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::TextInput, "SCU for commodity 42 (SCU)")
        .type_text("invalid");
    harness.run();
    let keyboard = harness.ctx.memory(|memory| memory.focused());
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceUpdated {
            live: support::empty_snapshot(UexEnvironment::Live).unwrap(),
            ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
            observed_at: Timestamp::from_seconds(1100),
        });
    harness.run();
    assert!(harness.state_mut().manual_mut().take_command().is_none());
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), keyboard);
    assert!(
        harness.state().manual().selected().unwrap().rows()[0]
            .stock()
            .is_none()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_q_manual_r_ui_16__late_review_or_failure_never_replaces_newer_attachment_state_or_a_newer_review_request()
 {
    use tallyline_reporting::api::AttachmentEdit;
    use tallyline_reporting::api::model::{
        AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash,
    };
    let (review, snapshot) = support::review().unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![review.report().clone()],
        selected: Some(review.report().id()),
    });
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: snapshot.clone(),
        ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
        observed_at: Timestamp::from_seconds(1000),
    });
    let mut harness = support::harness(shell).unwrap();
    harness.run();
    let Some(ManualCommand::Review(old)) = harness.state_mut().manual_mut().take_command() else {
        panic!("expected initial review");
    };
    let mut changed = review.report().clone();
    changed
        .edit_attachment(AttachmentEdit::Select(
            AttachmentBinding::new(
                ImageHash::new([2; 32]),
                AttachmentRegion::new(0, 10, 4, 2).unwrap(),
            ),
            AttachmentTime::User(Timestamp::from_seconds(900)),
        ))
        .unwrap();
    harness.state_mut().receive(BackgroundEvent::ReportsLoaded {
        reports: vec![changed.clone()],
        selected: None,
    });
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: old.expected,
            review: review.into(),
            snapshot,
        });
    assert_eq!(harness.state().manual().selected(), Some(&changed));
    assert!(harness.state().manual().review().is_none());
    harness.run();
    harness.state_mut().receive(BackgroundEvent::ReviewFailed {
        id: old.id,
        expected: old.expected,
        error: tallyline_reporting::api::RepositoryError::Unavailable,
    });
    harness.run();
    let Some(ManualCommand::Review(new)) = harness.state_mut().manual_mut().take_command() else {
        panic!("older failure cleared the newer request");
    };
    assert_eq!(new.expected, changed.version());
    assert!(new.expected > old.expected);
    assert!(!harness.state().dialogs().manual_confirmation_open());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5_r_ui_16_r_val_7_q_manual__old_policy_result_never_reopens_or_replaces_current_review() {
    use tallyline_reporting::api::model::StockSettings;
    use tallyline_shared_kernel::api::model::Percent;
    use tallyline_workflows::api::model::AppSettings;
    let (review, snapshot) = support::stock_review().unwrap();
    let report = review.report().clone();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    shell.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: snapshot.clone(),
        ptu: support::empty_snapshot(UexEnvironment::Ptu).unwrap(),
        observed_at: Timestamp::from_seconds(1000),
    });
    let mut harness = support::harness(shell).unwrap();
    harness.run();
    let Some(ManualCommand::Review(request)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected old policy request");
    };
    let tightened = StockSettings::new(Percent::whole(10), "8".parse().unwrap()).unwrap();
    harness.state_mut().receive(BackgroundEvent::SettingsLoaded(
        AppSettings::default().with_stock_settings(tightened),
    ));
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportReviewed {
            expected: request.expected,
            review: review.into(),
            snapshot,
        });
    assert!(harness.state().manual().review().is_none());
    assert!(!harness.state().dialogs().manual_confirmation_open());
    harness.run();
    let Some(ManualCommand::Review(request)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected current policy request");
    };
    assert_eq!(request.stock_settings, tightened);
}
