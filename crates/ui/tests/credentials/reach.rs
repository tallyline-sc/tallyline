// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public drawn credential presentation contracts.

use egui_kittest::kittest::Queryable;

use super::support;
#[test]
fn minimum_viewport_reveals_complete_focused_credential_controls_at_double_zoom() {
    for theme in [
        tallyline_egui::Theme::Light,
        tallyline_egui::Theme::Dark,
        tallyline_egui::Theme::HighContrast,
    ] {
        for zoom in [1.0, 2.0] {
            for onboarding in [false, true] {
                let mut harness = support::form(theme, zoom).unwrap();
                if onboarding {
                    harness.state_mut().leave_settings();
                    harness.run();
                }
                let viewport = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0 / zoom, 640.0 / zoom),
                );
                for label in [
                    "I understand: use the protected local file",
                    "UEX secret key",
                    "Show key",
                    "Own UEX app token",
                    "Show token",
                    "Keep credentials for this session only",
                    "Validate credentials",
                ] {
                    harness.get_by_label(label).focus();
                    harness.run();
                    if matches!(label, "UEX secret key" | "Own UEX app token") {
                        super::visibility::assert_input_visible(&harness, label).unwrap();
                    } else {
                        super::visibility::assert_focused_visible(&harness, label).unwrap();
                    }
                    let node = harness.get_by_label(label);
                    assert!(node.is_focused(), "{theme:?} {zoom}: {label}");
                    assert!(
                        node.rect().bottom()
                            <= harness.get_by_label("Saved submissions: 0").rect().top(),
                        "{theme:?} {zoom}: {label} remains above status"
                    );
                    assert!(
                        viewport.contains_rect(node.rect()),
                        "{theme:?} {zoom}: {label}: {:?}",
                        node.rect()
                    );
                }
                harness.get_by_label("UEX secret key").focus();
                harness.run();
                harness.key_press(egui::Key::Tab);
                harness.run();
                assert!(harness.get_by_label("Show key").is_focused());
                super::visibility::assert_focused_visible(&harness, "Show key").unwrap();
                harness.key_press(egui::Key::Space);
                harness.run();
                harness.get_by_label("Hide key");
            }
        }
    }
}

#[test]
fn passive_credential_failure_preserves_focus_and_page_position_in_every_theme() {
    use tallyline_submission::api::model::{AccountState, CredentialFailure, StorageFailure};
    use tallyline_ui::api::model::BackgroundEvent;
    for theme in [
        tallyline_egui::Theme::Light,
        tallyline_egui::Theme::Dark,
        tallyline_egui::Theme::HighContrast,
    ] {
        for zoom in [1.0, 2.0] {
            let mut harness = support::form(theme, zoom).unwrap();
            harness.get_by_label("Validate credentials").focus();
            harness.run();
            super::visibility::assert_focused_visible(&harness, "Validate credentials").unwrap();
            let account = harness
                .get_by_role_and_label(egui::Role::DisclosureTriangle, "UEX account")
                .rect();
            harness
                .state_mut()
                .receive(BackgroundEvent::CredentialsUpdated {
                    account: AccountState::Unconfigured,
                    failure: Some(CredentialFailure::Storage(StorageFailure::Locked)),
                    available_token: false,
                    fallback_available: true,
                });
            harness.run();
            assert!(harness.get_by_label("Validate credentials").is_focused());
            assert_eq!(
                harness
                    .get_by_role_and_label(egui::Role::DisclosureTriangle, "UEX account")
                    .rect(),
                account
            );
            assert!(!harness.state().dialogs().about_open());
            assert!(
                harness
                    .output()
                    .viewport_output
                    .values()
                    .all(|output| output.commands.is_empty())
            );
            assert!(
                harness
                    .state_mut()
                    .credentials_mut()
                    .take_command()
                    .is_none()
            );
        }
    }
}
