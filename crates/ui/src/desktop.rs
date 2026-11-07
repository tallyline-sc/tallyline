// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native frame rendering and bounded UI message delivery.

use crate::api::model::{BackgroundEvent, SettingsCommand};
use crate::api::{Catalogue, FanKitUnit, ShellState, show_shell};
use std::sync::mpsc::{Receiver, SyncSender};
use tallyline_egui::{Density, Motion, Theme as UiTheme, apply_theme};

/// The native rendering host with owned view state and injected worker channels.
pub struct DesktopShell {
    state: ShellState,
    catalogue: Catalogue,
    unit: FanKitUnit,
    dark_unit: FanKitUnit,
    commands: Option<SyncSender<SettingsCommand>>,
    reference_commands: Option<SyncSender<crate::api::model::ReferenceCommand>>,
    manual_commands: Option<SyncSender<crate::api::model::ManualCommand>>,
    credential_commands: Option<SyncSender<crate::api::model::CredentialCommand>>,
    submission_commands: Option<SyncSender<crate::api::model::SubmissionControl>>,
    events: Receiver<BackgroundEvent>,
    theme: Option<UiTheme>,
    window_commands: Option<Receiver<crate::api::model::WindowCommand>>,
}

impl DesktopShell {
    /// Creates the view host from predecoded artwork and bounded worker channels.
    #[must_use]
    pub fn new(
        context: &egui::Context,
        catalogue: Catalogue,
        light_pixels: egui::ColorImage,
        dark_pixels: egui::ColorImage,
        commands: SyncSender<SettingsCommand>,
        events: Receiver<BackgroundEvent>,
    ) -> Self {
        Self {
            state: ShellState::default(),
            unit: FanKitUnit::new(context, &catalogue, light_pixels),
            dark_unit: FanKitUnit::new(context, &catalogue, dark_pixels),
            catalogue,
            commands: Some(commands),
            reference_commands: None,
            manual_commands: None,
            credential_commands: None,
            submission_commands: None,
            events,
            theme: None,
            window_commands: None,
        }
    }
}
impl DesktopShell {
    /// Attaches a bounded channel for explicit second-launch window commands.
    #[must_use]
    pub fn with_window_commands(
        mut self,
        commands: Receiver<crate::api::model::WindowCommand>,
    ) -> Self {
        self.window_commands = Some(commands);
        self
    }
}

impl DesktopShell {
    /// Attaches the bounded worker channel for explicit reference-data requests.
    #[must_use]
    pub fn with_reference_commands(
        mut self,
        commands: SyncSender<crate::api::model::ReferenceCommand>,
    ) -> Self {
        self.reference_commands = Some(commands);
        self
    }
}

impl eframe::App for DesktopShell {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
    fn logic(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
        if let Some(commands) = &self.window_commands {
            for command in commands.try_iter() {
                crate::api::apply_window_command(context, command);
            }
        }
        for event in self.events.try_iter() {
            self.state.receive(event);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let theme =
            crate::api::resolve_theme(self.state.settings().theme(), ui.ctx().system_theme());
        if self.theme != Some(theme) {
            ui.ctx().set_theme(match theme {
                UiTheme::Light => egui::ThemePreference::Light,
                UiTheme::Dark | UiTheme::HighContrast => egui::ThemePreference::Dark,
            });
            apply_theme(ui.ctx(), theme, Density::Comfortable, Motion::Reduced);
            self.theme = Some(theme);
        }
        let unit = if theme == UiTheme::Light {
            &self.unit
        } else {
            &self.dark_unit
        };
        ui.set_style(ui.ctx().global_style());
        let commands = egui::CentralPanel::default()
            .show(ui, |ui| {
                show_shell(ui, &mut self.state, &self.catalogue, unit)
            })
            .inner;
        if let Some(command) = self.state.references_mut().take_command() {
            let sent = self
                .reference_commands
                .as_ref()
                .is_some_and(|sender| sender.try_send(command).is_ok());
            if !sent {
                self.state.reference_command_failed(
                    self.catalogue
                        .text("reference.worker_unavailable")
                        .to_owned(),
                );
            }
        }
        if let Some(command) = self.state.manual_mut().take_command() {
            let undelivered = match self.manual_commands.as_ref() {
                Some(sender) => match sender.try_send(command) {
                    Ok(()) => None,
                    Err(
                        std::sync::mpsc::TrySendError::Full(command)
                        | std::sync::mpsc::TrySendError::Disconnected(command),
                    ) => Some(command),
                },
                None => Some(command),
            };
            if let Some(command) = undelivered {
                self.state.manual_mut().failed_delivery(command);
            }
        }
        if let Some(command) = self.state.credentials_mut().take_command() {
            let sent = self
                .credential_commands
                .as_ref()
                .is_some_and(|sender| sender.try_send(command).is_ok());
            if !sent {
                self.state.credentials_mut().failed_delivery();
            }
        }
        if let Some(command) = self.state.take_submission_command() {
            let sent = self
                .submission_commands
                .as_ref()
                .is_some_and(|sender| sender.try_send(command).is_ok());
            if !sent {
                self.state.submission_delivery_failed();
            }
        }
        for command in commands {
            if let Some(sender) = &self.commands
                && sender.try_send(command).is_err()
            {
                self.state.receive(BackgroundEvent::Failure(
                    self.catalogue
                        .text("settings.worker_unavailable")
                        .to_owned(),
                ));
            }
        }
    }
}

impl Drop for DesktopShell {
    fn drop(&mut self) {
        drop(self.commands.take());
        drop(self.reference_commands.take());
        drop(self.manual_commands.take());
        drop(self.credential_commands.take());
        drop(self.submission_commands.take());
    }
}

impl DesktopShell {
    /// Attaches the bounded app-owned manual Report storage channel.
    #[must_use]
    pub fn with_manual_commands(
        mut self,
        commands: SyncSender<crate::api::model::ManualCommand>,
    ) -> Self {
        self.manual_commands = Some(commands);
        self
    }
}

impl DesktopShell {
    /// Attaches the bounded credential validation channel.
    #[must_use]
    pub fn with_credential_commands(
        mut self,
        commands: SyncSender<crate::api::model::CredentialCommand>,
    ) -> Self {
        self.credential_commands = Some(commands);
        self
    }
}

impl DesktopShell {
    /// Supplies immutable compiled build display data before the first native frame.
    #[must_use]
    pub fn with_build_presentation(mut self, build: crate::api::model::BuildPresentation) -> Self {
        self.state.set_build_presentation(build);
        self
    }
}

impl DesktopShell {
    /// Attaches the bounded queue channel owned by the existing session credential worker.
    #[must_use]
    pub fn with_submission_commands(
        mut self,
        commands: SyncSender<crate::api::model::SubmissionControl>,
    ) -> Self {
        self.submission_commands = Some(commands);
        self
    }
}
