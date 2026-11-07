// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public drawn credential presentation contracts.

use super::support;
use egui_kittest::kittest::Queryable;
use tallyline_submission::api::model::{
    AccountState, CredentialFailure, Persistence, StorageFailure,
};
use tallyline_ui::api::model::BackgroundEvent;
#[test]
fn typed_account_states_have_distinct_glyphs_and_semantic_text_in_every_theme() {
    for theme in [
        tallyline_egui::Theme::Light,
        tallyline_egui::Theme::Dark,
        tallyline_egui::Theme::HighContrast,
    ] {
        let mut harness = support::form(theme, 1.0).unwrap();
        harness.set_size(egui::vec2(1000.0, 1800.0));
        for (account, label, glyph) in [
            (
                AccountState::Unconfigured,
                "No validated UEX secret key",
                tallyline_egui::symbols::KEY_OFF,
            ),
            (
                AccountState::Rejected(CredentialFailure::InvalidKey),
                "UEX refused the secret key. Check the key in your UEX profile.",
                tallyline_egui::symbols::VPN_KEY_OFF,
            ),
            (
                AccountState::Rejected(CredentialFailure::NotAllowed),
                "This UEX account cannot submit DataRunner reports. Check your account status with UEX.",
                tallyline_egui::symbols::BLOCK,
            ),
        ] {
            harness
                .state_mut()
                .receive(BackgroundEvent::CredentialsUpdated {
                    account,
                    failure: None,
                    available_token: false,
                    fallback_available: false,
                });
            harness.run();
            harness.get_by_label(label);
            harness.get_by_label("Show key").focus();
            harness.run();
            let foreground = theme.palette().color("critical-fg").unwrap();
            assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.job.text == glyph && text.galley.job.sections.iter().all(|section| section.format.color == foreground))), "{theme:?}: glyph for {label}");
        }
    }
}

#[test]
fn operational_failures_keep_the_accepted_account_and_distinct_actionable_feedback() {
    for theme in [
        tallyline_egui::Theme::Light,
        tallyline_egui::Theme::Dark,
        tallyline_egui::Theme::HighContrast,
    ] {
        let mut harness = support::form(theme, 1.0).unwrap();
        harness.set_size(egui::vec2(1000.0, 1800.0));
        for (failure, label, glyph, token) in [
            (
                CredentialFailure::InvalidToken,
                "UEX refused the app token. Install a current Tallyline release or enter your own token using the expert option.",
                tallyline_egui::symbols::VPN_KEY_ALERT,
                "critical-fg",
            ),
            (
                CredentialFailure::Unavailable,
                "UEX could not be reached. Check your connection and try again. Your accepted account is unchanged.",
                tallyline_egui::symbols::CLOUD_OFF,
                "caution-fg",
            ),
            (
                CredentialFailure::Storage(StorageFailure::Locked),
                "The credential store is locked or access was declined. Unlock it yourself or choose session-only.",
                tallyline_egui::symbols::LOCK_RESET,
                "caution-fg",
            ),
        ] {
            let account = AccountState::Validated {
                username: "dummy-account".to_owned(),
                validated_at: tallyline_shared_kernel::api::model::Timestamp::from_seconds(100),
                persistence: Persistence::Session,
            };
            harness
                .state_mut()
                .receive(BackgroundEvent::CredentialsUpdated {
                    account,
                    failure: Some(failure),
                    available_token: false,
                    fallback_available: false,
                });
            harness.run();
            harness.get_by_label("Show key").focus();
            harness.run();
            assert!(
                matches!(harness.state().credentials().account(), AccountState::Validated { username, persistence: Persistence::Session, .. } if username == "dummy-account")
            );
            harness.get_by_label(label);
            harness.get_by_label("Validated account: dummy-account");
            let foreground = theme.palette().color(token).unwrap();
            for text in [glyph, label] {
                assert!(harness.output().shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(paint) if paint.galley.job.text == text && paint.galley.job.sections.iter().all(|section| section.format.color == foreground))), "{theme:?}: {text}");
            }
            assert!(!support::exposed(&harness, "banned"));
        }
    }
}
