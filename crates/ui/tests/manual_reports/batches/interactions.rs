// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic user input and explicit manual actions through drawn controls.

use super::support;
use super::support::{Cache, Fixed};
use egui_kittest::kittest::Queryable;
use tallyline_reference_data::api::ReferenceApplication;
use tallyline_reference_data::api::model::UexEnvironment;
use tallyline_reporting::api::{ManualEdit, RepositoryError};
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
use tallyline_ui::api::model::{BackgroundEvent, ManualCommand};
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_3__cross_row_focus_commit_and_takeover_share_one_request_in_both_row_orders() {
    for order in [[42, 43], [43, 42]] {
        let mut harness = support::harness(&order).unwrap();
        support::enter(&mut harness, 43, "12345678901234567890.125");
        harness
            .get_by_label("Take over UEX values explicitly")
            .click();
        harness.run();
        let command = harness.state_mut().manual_mut().take_command().unwrap();
        let ManualCommand::Edit { edits, .. } = &command else {
            panic!("expected an edit");
        };
        assert!(
            matches!(edits.as_slice(), [ManualEdit::Price(_, Some(price)), ManualEdit::TakeOver(_)] if price.to_string() == "12345678901234567890.125")
        );
        assert!(harness.state_mut().manual_mut().take_command().is_none());
        let mut committed = harness.state().manual().selected().unwrap().clone();
        let row = committed
            .rows()
            .iter()
            .find(|row| row.commodity() == CommodityId(43))
            .unwrap()
            .id();
        committed
            .set_price(row, Some("12345678901234567890.125".parse().unwrap()))
            .unwrap();
        support::takeover(&mut committed).unwrap();
        support::result(harness.state_mut(), &command, Ok(committed)).unwrap();
        harness.run();
        assert_eq!(support::value(&harness, 42).unwrap(), "100");
        assert_eq!(
            support::value(&harness, 43).unwrap(),
            "12345678901234567890.125"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_3__unrelated_invalid_input_survives_a_successful_takeover_receipt() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    support::enter(&mut harness, 43, "-");
    harness
        .get_by_label("Take over UEX values explicitly")
        .click();
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    assert!(
        matches!(&command, ManualCommand::Edit { edits, .. } if matches!(edits.as_slice(), [ManualEdit::TakeOver(_)]))
    );
    let mut committed = harness.state().manual().selected().unwrap().clone();
    support::takeover(&mut committed).unwrap();
    support::result(harness.state_mut(), &command, Ok(committed)).unwrap();
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "-");
    assert_eq!(support::value(&harness, 42).unwrap(), "100");
    assert!(harness.state_mut().manual_mut().take_command().is_none());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_3__explicit_row_takeover_replaces_its_uncommitted_invalid_price() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    support::enter(&mut harness, 42, "0");
    harness
        .get_by_label("Take over UEX values explicitly")
        .click();
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    assert!(
        matches!(&command, ManualCommand::Edit { edits, .. } if matches!(edits.as_slice(), [ManualEdit::TakeOver(_)]))
    );
    let mut committed = harness.state().manual().selected().unwrap().clone();
    support::takeover(&mut committed).unwrap();
    support::result(harness.state_mut(), &command, Ok(committed)).unwrap();
    harness.run();
    assert_eq!(support::value(&harness, 42).unwrap(), "100");
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_man_3__add_commodity_and_context_changes_include_uncommitted_numeric_input() {
    for action in [
        "Add commodity Copper · UEX ID 44",
        "Sell",
        "Depot 12 · Pyro · UEX ID 12",
    ] {
        let mut harness = support::harness(&[42, 43]).unwrap();
        harness
            .get_by_label("Add commodity from vocabulary")
            .focus();
        harness.run();
        harness
            .get_by_label("Add commodity from vocabulary")
            .type_text("Copper");
        harness.run();
        support::enter(&mut harness, 43, "12.5");
        harness.get_by_label(action).click();
        harness.run();
        let command = harness.state_mut().manual_mut().take_command().unwrap();
        let ManualCommand::Edit { edits, .. } = command else {
            panic!("expected an edit");
        };
        assert!(edits.iter().any(
            |edit| matches!(edit, ManualEdit::Price(_, Some(price)) if price.to_string() == "12.5")
        ));
        assert_eq!(edits.len(), 2);
        match action {
            "Add commodity Copper · UEX ID 44" => assert!(matches!(
                edits.last(),
                Some(ManualEdit::AddCommodity(CommodityId(44)))
            )),
            "Sell" => assert!(
                matches!(edits.first(), Some(ManualEdit::ChangeKey(key)) if key.side() == TradeSide::Sell)
            ),
            _ => assert!(
                matches!(edits.first(), Some(ManualEdit::ChangeKey(key)) if key.terminal() == TerminalId(12))
            ),
        }
        assert!(harness.state_mut().manual_mut().take_command().is_none());
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_val_7_q_manual__reference_refresh_keeps_the_requested_snapshot_and_user_values() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    harness
        .get_by_label("Take over UEX values explicitly")
        .click();
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    let refreshed = ReferenceApplication::load(Cache("200"), Fixed).unwrap();
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceUpdated {
            live: refreshed.snapshot(UexEnvironment::Live),
            ptu: refreshed.snapshot(UexEnvironment::Ptu),
            observed_at: Timestamp::from_seconds(120),
        });
    harness.run();
    assert_eq!(support::value(&harness, 42).unwrap(), "");
    let ManualCommand::Edit { snapshot, .. } = &command else {
        panic!("expected an edit");
    };
    assert_eq!(
        snapshot.terminal_priors(TerminalId(11)).unwrap().rows()[0]
            .values()
            .price
            .to_string(),
        "100"
    );
    let mut committed = harness.state().manual().selected().unwrap().clone();
    support::takeover(&mut committed).unwrap();
    support::result(harness.state_mut(), &command, Ok(committed)).unwrap();
    harness.run();
    assert_eq!(support::value(&harness, 42).unwrap(), "100");
    let Some(ManualCommand::Review(review)) = harness.state_mut().manual_mut().take_command()
    else {
        panic!("expected current-reference reassessment");
    };
    assert_eq!(
        review
            .snapshot
            .terminal_priors(TerminalId(11))
            .unwrap()
            .rows()[0]
            .values()
            .price
            .to_string(),
        "200"
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_2_r_ui_16__context_refusal_restores_saved_choices_without_erasing_or_retrying_input() {
    let mut harness = support::harness(&[42, 43]).unwrap();
    support::enter(&mut harness, 43, "12.5");
    harness.get_by_label("Sell").click();
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    support::result(harness.state_mut(), &command, Err(RepositoryError::Invalid)).unwrap();
    harness.run();
    assert_eq!(support::value(&harness, 43).unwrap(), "12.5");
    assert_eq!(
        harness.state().manual().selected().unwrap().key().side(),
        TradeSide::Buy
    );
    assert!(harness.state_mut().manual_mut().take_command().is_none());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_13__drawn_affinity_starts_empty_rejects_invalid_values_and_commits_explicit_zero() {
    use egui_kittest::kittest::NodeT;
    let label = "Faction affinity (optional, -100 to 100)";
    let mut harness = support::harness(&[42]).unwrap();
    assert_eq!(
        harness
            .get_by_role_and_label(egui::Role::TextInput, label)
            .accesskit_node()
            .value()
            .unwrap(),
        ""
    );
    assert_eq!(
        harness
            .state()
            .manual()
            .selected()
            .unwrap()
            .faction_affinity(),
        None
    );
    for invalid in ["-101", "101", "1.5", "no"] {
        harness
            .get_by_role_and_label(egui::Role::TextInput, label)
            .focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness
            .get_by_role_and_label(egui::Role::TextInput, label)
            .type_text(invalid);
        harness.run();
        harness.key_press(egui::Key::Tab);
        harness.run();
        assert!(harness.state_mut().manual_mut().take_command().is_none());
        assert_eq!(
            harness
                .state()
                .manual()
                .selected()
                .unwrap()
                .faction_affinity(),
            None
        );
    }
    harness
        .get_by_role_and_label(egui::Role::TextInput, label)
        .focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::TextInput, label)
        .type_text("0");
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
    let command = harness.state_mut().manual_mut().take_command().unwrap();
    assert!(
        matches!(&command, ManualCommand::Edit { edits, .. } if matches!(edits.as_slice(), [ManualEdit::FactionAffinity(Some(value))] if value.value() == 0))
    );
    let committed = harness
        .state()
        .manual()
        .selected()
        .unwrap()
        .set_faction_affinity(Some("0".parse().unwrap()))
        .unwrap();
    support::result(harness.state_mut(), &command, Ok(committed)).unwrap();
    harness.run();
    assert_eq!(
        harness
            .get_by_role_and_label(egui::Role::TextInput, label)
            .accesskit_node()
            .value()
            .unwrap(),
        "0"
    );
}
