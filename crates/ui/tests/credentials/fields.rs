// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public drawn credential presentation contracts.

use super::support;
use egui_kittest::kittest::Queryable;
use tallyline_submission::api::model::{AccountState, CredentialFailure, Persistence};
use tallyline_ui::api::model::{BackgroundEvent, CredentialCommand};
#[test]
fn explicit_reveal_is_labelled_and_replacement_resets_both_fields() {
    let mut harness = support::form(tallyline_egui::Theme::Light, 1.0).unwrap();
    assert!(!support::exposed(&harness, "dummy-key-mask-probe"));
    assert!(!support::exposed(&harness, "dummy-token-mask-probe"));
    harness.get_by_label("Use your UEX secret key, not your RSI password or UEX app token.");
    super::visibility::activate_visible(&mut harness, "Show key").unwrap();
    harness.get_by_label("Hide key");
    assert!(support::exposed(&harness, "dummy-key-mask-probe"));
    assert!(!support::exposed(&harness, "dummy-token-mask-probe"));
    super::visibility::activate_visible(&mut harness, "Show token").unwrap();
    assert!(support::exposed(&harness, "dummy-token-mask-probe"));
    harness.state_mut().credentials_mut().set_input(
        "dummy-key-mask-probe".to_owned(),
        Some("dummy-token-mask-probe".to_owned()),
        Persistence::Session,
    );
    harness.run();
    harness.get_by_label("Show key");
    harness.get_by_label("Show token");
    assert!(!support::exposed(&harness, "dummy-key-mask-probe"));
    assert!(!support::exposed(&harness, "dummy-token-mask-probe"));
}

#[test]
fn reveal_hide_validation_and_busy_controls_preserve_session_and_clear_inputs() {
    use egui_kittest::kittest::NodeT as _;
    let mut harness = support::form(tallyline_egui::Theme::Dark, 1.0).unwrap();
    for (show, hide) in [("Show key", "Hide key"), ("Show token", "Hide token")] {
        super::visibility::activate_visible(&mut harness, show).unwrap();
        super::visibility::activate_visible(&mut harness, hide).unwrap();
        harness.get_by_label(show);
    }
    assert!(!support::exposed(&harness, "dummy-key-mask-probe"));
    assert!(!support::exposed(&harness, "dummy-token-mask-probe"));
    super::visibility::activate_visible(&mut harness, "Show key").unwrap();
    super::visibility::activate_visible(&mut harness, "Validate credentials").unwrap();
    assert!(harness.state().credentials().busy());
    assert!(!support::exposed(&harness, "dummy-key-mask-probe"));
    assert!(!support::exposed(&harness, "dummy-token-mask-probe"));
    for label in [
        "Show key",
        "Show token",
        "Validate credentials",
        "Keep credentials for this session only",
        "I understand: use the protected local file",
    ] {
        assert!(
            harness.get_by_label(label).accesskit_node().is_disabled(),
            "{label}"
        );
    }
    let Some(CredentialCommand::Validate(candidate)) =
        harness.state_mut().credentials_mut().take_command()
    else {
        panic!("explicit validation queues one candidate");
    };
    assert_eq!(candidate.persistence(), Persistence::Session);
    assert!(
        harness
            .state_mut()
            .credentials_mut()
            .take_command()
            .is_none()
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::CredentialsUpdated {
            account: AccountState::Unconfigured,
            failure: Some(CredentialFailure::Unavailable),
            available_token: false,
            fallback_available: true,
        });
    harness.state_mut().credentials_mut().validate();
    assert_eq!(
        harness.state().credentials().failure(),
        Some(CredentialFailure::InvalidInput)
    );
    assert!(
        harness
            .state_mut()
            .credentials_mut()
            .take_command()
            .is_none()
    );
}

#[test]
fn fallback_confirmation_still_queues_only_the_explicit_warning_choice() {
    let mut harness = support::form(tallyline_egui::Theme::HighContrast, 2.0).unwrap();
    assert!(
        harness
            .state_mut()
            .credentials_mut()
            .take_command()
            .is_none()
    );
    super::visibility::activate_visible(&mut harness, "I understand: use the protected local file")
        .unwrap();
    assert!(matches!(
        harness.state_mut().credentials_mut().take_command(),
        Some(CredentialCommand::ConfirmFallback)
    ));
    assert!(harness.state().credentials().busy());
}

#[test]
fn clearing_revealed_inputs_masks_the_next_edit_and_packaged_token_stays_private() {
    let mut harness = support::form(tallyline_egui::Theme::Light, 1.0).unwrap();
    for (field, reveal, value) in [
        ("UEX secret key", "Show key", "dummy-key-mask-probe"),
        ("Own UEX app token", "Show token", "dummy-token-mask-probe"),
    ] {
        super::visibility::activate_visible(&mut harness, reveal).unwrap();
        harness.get_by_label(field).focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness.key_press(egui::Key::Backspace);
        harness.run();
        harness.get_by_label(reveal);
        harness.get_by_label(field).type_text(value);
        harness.run();
        assert!(!support::exposed(&harness, value));
    }
    harness
        .state_mut()
        .receive(BackgroundEvent::CredentialsUpdated {
            account: AccountState::Unconfigured,
            failure: None,
            available_token: true,
            fallback_available: false,
        });
    harness.state_mut().credentials_mut().set_input(
        "dummy-key-mask-probe".to_owned(),
        None,
        Persistence::Durable,
    );
    harness.run();
    assert!(harness.query_by_label("Show token").is_none());
    super::visibility::activate_visible(&mut harness, "Expert: use my own app token").unwrap();
    harness.get_by_label("Show token");
    harness.get_by_label("Own UEX app token");
}
