// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Masked credential input and safe account feedback.

use super::feedback;
use super::secret_field;
use super::secret_field::{SecretKind, Visibility};

use tallyline_submission::api::model::{
    AccountState, CredentialCandidate, CredentialFailure, CredentialValues, Persistence,
};

/// A secret-bearing request without diagnostic or serialisation support.
pub enum CredentialCommand {
    /// Confirms the displayed Linux plaintext fallback warning.
    ConfirmFallback,
    /// Validate and explicitly accept the selected lifetime.
    Validate(CredentialCandidate),
}

/// Short-lived input and safe credential worker results.
#[derive(Default)]
pub struct CredentialSetup {
    key: String,
    token: String,
    own_token: bool,
    key_visibility: Visibility,
    token_visibility: Visibility,
    persistence: Option<Persistence>,
    phase: SetupPhase,
    available_token: bool,
    fallback_available: bool,
    account: AccountState,
    failure: Option<CredentialFailure>,
    pending: Option<CredentialCommand>,
}
impl CredentialSetup {
    /// Returns the accepted account independently of current input.
    #[must_use]
    pub const fn account(&self) -> &AccountState {
        &self.account
    }
    /// Returns the closed actionable failure.
    #[must_use]
    pub const fn failure(&self) -> Option<CredentialFailure> {
        self.failure
    }
    /// Returns whether validation is pending.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.phase == SetupPhase::Checking
    }
    /// Sets only unsaved masked input after a user edit.
    pub fn set_input(&mut self, key: String, token: Option<String>, persistence: Persistence) {
        self.key_visibility = Visibility::Masked;
        self.token_visibility = Visibility::Masked;
        self.key = key;
        self.own_token = token.is_some();
        self.token = token.unwrap_or_default();
        self.persistence = Some(persistence);
    }
    /// Queues one candidate and clears the view's input copies.
    pub fn validate(&mut self) {
        if self.phase != SetupPhase::Ready {
            return;
        }
        self.key_visibility = Visibility::Masked;
        self.token_visibility = Visibility::Masked;
        let token = if self.own_token || !self.available_token {
            Some(std::mem::take(&mut self.token))
        } else {
            self.token.clear();
            None
        };
        match CredentialValues::new(std::mem::take(&mut self.key), token) {
            Ok(values) => {
                self.pending = Some(CredentialCommand::Validate(CredentialCandidate::new(
                    values,
                    self.persistence.unwrap_or(Persistence::Durable),
                )));
                self.phase = SetupPhase::Checking;
                self.failure = None;
            }
            Err(failure) => self.failure = Some(failure),
        }
    }
    /// Drains the bounded command for the app-owned worker.
    pub fn take_command(&mut self) -> Option<CredentialCommand> {
        self.pending.take()
    }
    pub(crate) fn updated(
        &mut self,
        account: AccountState,
        failure: Option<CredentialFailure>,
        available_token: bool,
        fallback_available: bool,
    ) {
        self.account = account;
        self.failure = failure;
        self.available_token = available_token;
        self.fallback_available = fallback_available;
        self.phase = SetupPhase::Ready;
    }
    pub(crate) fn failed_delivery(&mut self) {
        self.failure = Some(CredentialFailure::Unavailable);
        self.phase = SetupPhase::Ready;
    }
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    setup: &mut CredentialSetup,
    catalogue: &crate::api::Catalogue,
    theme: tallyline_egui::Theme,
) {
    ui.heading(catalogue.text("credentials.title"));
    ui.label(catalogue.text("credentials.local_use"));
    feedback::show(ui, &setup.account, setup.failure, catalogue, theme);
    if setup.fallback_available {
        ui.label(catalogue.text("credentials.fallback_warning"));
        let response = ui.add_enabled(
            !setup.busy(),
            egui::Button::new(tallyline_egui::button_text(
                catalogue.text("credentials.fallback_confirm"),
            ))
            .wrap(),
        );
        crate::control_focus::reveal(ui, &response);
        if response.clicked() {
            setup.pending = Some(CredentialCommand::ConfirmFallback);
            setup.phase = SetupPhase::Checking;
        }
    }
    ui.scope_builder(
        egui::UiBuilder::new().scope_id(ui.make_persistent_id("credential-inputs")),
        |ui| {
            ui.add_enabled_ui(setup.phase == SetupPhase::Ready, |ui| {
                secret_field::show(
                    ui,
                    &mut setup.key,
                    &mut setup.key_visibility,
                    catalogue,
                    SecretKind::Key,
                );
                if setup.available_token {
                    let response =
                        ui.checkbox(&mut setup.own_token, catalogue.text("credentials.expert"));
                    crate::control_focus::reveal(ui, &response);
                    if response.changed() {
                        setup.token_visibility = Visibility::Masked;
                    }
                } else {
                    ui.label(catalogue.text("credentials.token_missing"));
                }
                if setup.own_token || !setup.available_token {
                    secret_field::show(
                        ui,
                        &mut setup.token,
                        &mut setup.token_visibility,
                        catalogue,
                        SecretKind::Token,
                    );
                }
                let mut session = setup.persistence == Some(Persistence::Session);
                let response =
                    ui.checkbox(&mut session, catalogue.text("credentials.session_choice"));
                crate::control_focus::reveal(ui, &response);
                if response.changed() {
                    setup.persistence = Some(if session {
                        Persistence::Session
                    } else {
                        Persistence::Durable
                    });
                }
                let response = ui.button(tallyline_egui::button_text(
                    catalogue.text("credentials.validate"),
                ));
                crate::control_focus::reveal(ui, &response);
                if response.clicked() {
                    setup.validate();
                }
            });
        },
    );
    if setup.busy() {
        ui.label(catalogue.text("credentials.checking"));
    }
}
#[derive(Default, PartialEq, Eq)]
enum SetupPhase {
    #[default]
    Starting,
    Ready,
    Checking,
}
