// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed account and operational credential feedback.

use tallyline_egui::symbols;
use tallyline_submission::api::model::{AccountState, CredentialFailure, Persistence};

const ICON_SIZE: f32 = 20.0;

pub(super) fn show(
    ui: &mut egui::Ui,
    account: &AccountState,
    failure: Option<CredentialFailure>,
    catalogue: &crate::api::Catalogue,
    theme: tallyline_egui::Theme,
) {
    match account {
        AccountState::Unconfigured => self::line(
            ui,
            catalogue.text("credentials.unconfigured"),
            symbols::KEY_OFF,
            "critical-fg",
            theme,
        ),
        AccountState::Rejected(reason) => self::failure(ui, *reason, catalogue, theme),
        AccountState::Validated {
            username,
            persistence,
            ..
        } => {
            self::line(
                ui,
                &format!("{}: {username}", catalogue.text("credentials.validated")),
                symbols::CHECK_CIRCLE,
                "success-fg",
                theme,
            );
            ui.label(catalogue.text(match persistence {
                Persistence::Durable => "credentials.durable",
                Persistence::Session => "credentials.session",
            }));
        }
    }
    if let Some(failure) = failure {
        self::failure(ui, failure, catalogue, theme);
    }
}

fn failure(
    ui: &mut egui::Ui,
    failure: CredentialFailure,
    catalogue: &crate::api::Catalogue,
    theme: tallyline_egui::Theme,
) {
    let (glyph, tone) = match failure {
        CredentialFailure::InvalidKey => (symbols::VPN_KEY_OFF, "critical-fg"),
        CredentialFailure::NotAllowed => (symbols::BLOCK, "critical-fg"),
        CredentialFailure::InvalidInput => (symbols::EDIT_SQUARE, "critical-fg"),
        CredentialFailure::InvalidToken | CredentialFailure::MissingToken => {
            (symbols::VPN_KEY_ALERT, "critical-fg")
        }
        CredentialFailure::UpdateRequired | CredentialFailure::ClientVersionMissing => {
            (symbols::UPDATE, "caution-fg")
        }
        CredentialFailure::Unavailable | CredentialFailure::Superseded => {
            (symbols::CLOUD_OFF, "caution-fg")
        }
        CredentialFailure::Storage(_) => (symbols::LOCK_RESET, "caution-fg"),
    };
    self::line(
        ui,
        catalogue.text(self::failure_key(failure)),
        glyph,
        tone,
        theme,
    );
}

fn line(ui: &mut egui::Ui, text: &str, glyph: &str, tone: &str, theme: tallyline_egui::Theme) {
    let colour = theme
        .palette()
        .color(tone)
        .unwrap_or_else(|| ui.visuals().text_color());
    ui.horizontal_top(|ui| {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE), egui::Sense::hover());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::new(ICON_SIZE, egui::FontFamily::Name(symbols::FAMILY.into())),
            colour,
        );
        ui.add(egui::Label::new(tallyline_egui::TypeStyle::BODY.text(text).color(colour)).wrap());
    });
}
fn failure_key(failure: CredentialFailure) -> &'static str {
    use tallyline_submission::api::model::StorageFailure;
    match failure {
        CredentialFailure::InvalidInput => "credentials.invalid_input",
        CredentialFailure::InvalidKey => "credentials.invalid_key",
        CredentialFailure::NotAllowed => "credentials.not_allowed",
        CredentialFailure::InvalidToken => "credentials.invalid_token",
        CredentialFailure::MissingToken => "credentials.token_missing",
        CredentialFailure::UpdateRequired | CredentialFailure::ClientVersionMissing => {
            "credentials.update"
        }
        CredentialFailure::Unavailable | CredentialFailure::Superseded => "credentials.unavailable",
        CredentialFailure::Storage(StorageFailure::Unavailable) => {
            "credentials.storage_unavailable"
        }
        CredentialFailure::Storage(StorageFailure::Locked) => "credentials.locked",
        CredentialFailure::Storage(StorageFailure::Corrupt) => "credentials.corrupt",
        CredentialFailure::Storage(StorageFailure::OutcomeUnknown) => "credentials.uncertain",
        CredentialFailure::Storage(StorageFailure::WriteFailed) => "credentials.save_failed",
    }
}
