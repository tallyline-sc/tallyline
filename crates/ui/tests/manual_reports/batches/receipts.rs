// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Editor request ownership across selection, delivery failures and retries.

use super::support;
use egui_kittest::kittest::{NodeT, Queryable};
use tallyline_reporting::api::RepositoryError;
use tallyline_reporting::api::model::ReportId;
use tallyline_ui::api::model::{BackgroundEvent, ManualCommand};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__edit_remains_owned_while_another_draft_is_selected() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    support::enter(&mut harness, 43, "12.5");
    harness.key_press(egui::Key::Tab);
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    let mut committed = harness.state().manual().selected().unwrap().clone();
    let row = committed.rows()[1].id();
    committed
        .set_price(row, Some("12.5".parse().unwrap()))
        .unwrap();
    harness.get_by_label("Resume Draft 2 · 11 · Live").click();
    harness.run();
    assert_eq!(
        harness.state().manual().selected().unwrap().id(),
        ReportId(2)
    );
    assert!(
        harness
            .get_by_role_and_label(egui::Role::TextInput, &support::price_label(99))
            .accesskit_node()
            .is_disabled()
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::ReportFailed(RepositoryError::Unavailable));
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(egui::Role::TextInput, &support::price_label(99))
            .accesskit_node()
            .is_disabled()
    );
    support::result(harness.state_mut(), &command, Ok(committed)).unwrap();
    harness.run();
    assert_eq!(
        harness.state().manual().selected().unwrap().id(),
        ReportId(2)
    );
    assert_eq!(support::value(&harness, 99).unwrap(), "");
    assert!(
        !harness
            .get_by_role_and_label(egui::Role::TextInput, &support::price_label(99))
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__late_same_version_success_and_failure_cannot_replace_a_new_attempt() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    support::enter(&mut harness, 43, "12.5");
    harness.key_press(egui::Key::Tab);
    harness.run();
    let first = harness.state_mut().manual_mut().take_command().unwrap();
    let before = harness.state().manual().selected().unwrap().clone();
    support::result(
        harness.state_mut(),
        &first,
        Err(RepositoryError::Unavailable),
    )
    .unwrap();
    harness.run();
    assert!(
        harness.state_mut().manual_mut().take_command().is_none(),
        "failure must not automatically retry a dirty input"
    );
    support::enter(&mut harness, 43, "22.5");
    assert_eq!(support::value(&harness, 43).unwrap(), "22.5");
    harness.key_press(egui::Key::Tab);
    harness.run();
    let second = harness.state_mut().manual_mut().take_command().unwrap();
    let ManualCommand::Edit {
        request: first_request,
        expected: first_version,
        ..
    } = &first
    else {
        panic!("expected an edit");
    };
    let ManualCommand::Edit {
        request: second_request,
        expected: second_version,
        ..
    } = &second
    else {
        panic!("expected an edit");
    };
    assert_ne!(first_request, second_request);
    assert_eq!(first_version, second_version);
    let mut old_result = before.clone();
    old_result
        .set_price(old_result.rows()[1].id(), Some("12.5".parse().unwrap()))
        .unwrap();
    support::result(harness.state_mut(), &first, Ok(old_result)).unwrap();
    support::result(harness.state_mut(), &first, Err(RepositoryError::Conflict)).unwrap();
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "22.5");
    assert_eq!(harness.state().manual().selected(), Some(&before));
    assert!(
        harness
            .get_by_role_and_label(egui::Role::TextInput, &support::price_label(43))
            .accesskit_node()
            .is_disabled()
    );
    let mut committed = before;
    committed
        .set_price(committed.rows()[1].id(), Some("22.5".parse().unwrap()))
        .unwrap();
    support::result(harness.state_mut(), &second, Ok(committed)).unwrap();
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "22.5");
    assert!(matches!(
        harness.state_mut().manual_mut().take_command(),
        Some(ManualCommand::Review(_))
    ));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__failed_transport_releases_only_its_owned_request_and_keeps_input() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    support::enter(&mut harness, 43, "12.5");
    harness.key_press(egui::Key::Tab);
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    harness.state_mut().manual_mut().failed_delivery(command);
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "12.5");
    assert!(
        !harness
            .get_by_role_and_label(egui::Role::TextInput, &support::price_label(43))
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__passive_new_values_cannot_rebase_unfinished_input_without_reopening() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    let before = harness.state().manual().selected().unwrap().clone();
    support::enter(&mut harness, 43, "12.5");
    let mut newer = before.clone();
    newer
        .set_price(newer.rows()[1].id(), Some("99".parse().unwrap()))
        .unwrap();
    harness.state_mut().receive(BackgroundEvent::ReportsLoaded {
        reports: vec![newer.clone()],
        selected: None,
    });
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "12.5");
    harness.key_press(egui::Key::Tab);
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    assert!(
        matches!(&command, ManualCommand::Edit { expected, .. } if *expected == before.version())
    );
    support::result(
        harness.state_mut(),
        &command,
        Err(RepositoryError::Conflict),
    )
    .unwrap();
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "12.5");
    assert!(harness.state_mut().manual_mut().take_command().is_none());
    harness.get_by_label("Resume Draft 1 · 11 · Live").click();
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "99");
    if let Some(ManualCommand::Review(review)) = harness.state_mut().manual_mut().take_command() {
        harness.state_mut().receive(BackgroundEvent::ReviewFailed {
            id: review.id,
            expected: review.expected,
            error: RepositoryError::Unavailable,
        });
        harness.run();
    }
    support::enter(&mut harness, 43, "22.5");
    harness.key_press(egui::Key::Tab);
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    assert!(matches!(command, ManualCommand::Edit { expected, .. } if expected == newer.version()));
}
