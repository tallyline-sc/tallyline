// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Accessible secret input with transient explicit visibility.

#[derive(Default, PartialEq, Eq)]
pub(super) enum Visibility {
    #[default]
    Masked,
    Revealed,
}

pub(super) enum SecretKind {
    Key,
    Token,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    value: &mut String,
    visibility: &mut Visibility,
    catalogue: &crate::api::Catalogue,
    kind: SecretKind,
) {
    let (label_key, show_key, hide_key, salt) = match kind {
        SecretKind::Token => (
            "credentials.token",
            "credentials.show_token",
            "credentials.hide_token",
            "credential-token",
        ),
        SecretKind::Key => (
            "credentials.key",
            "credentials.show_key",
            "credentials.hide_key",
            "credential-key",
        ),
    };
    let label = ui.label(catalogue.text(label_key));
    let response = ui
        .add(
            egui::TextEdit::singleline(value)
                .id_salt(salt)
                .desired_width(ui.available_width())
                .password(*visibility == Visibility::Masked)
                .char_limit(1024),
        )
        .labelled_by(label.id);
    crate::control_focus::reveal(ui, &response);
    if value.is_empty() {
        *visibility = Visibility::Masked;
    }
    if matches!(kind, SecretKind::Key) {
        ui.label(tallyline_egui::TypeStyle::BODY.text(catalogue.text("credentials.key_hint")));
    }
    let response = ui.button(tallyline_egui::button_text(catalogue.text(
        if *visibility == Visibility::Revealed {
            hide_key
        } else {
            show_key
        },
    )));
    crate::control_focus::reveal(ui, &response);
    if response.clicked() {
        *visibility = if *visibility == Visibility::Masked {
            Visibility::Revealed
        } else {
            Visibility::Masked
        };
    }
}
