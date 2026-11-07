// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Window effects requested by an explicit second launch.

/// A direct user action delivered independently of background worker results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCommand {
    /// Restore the existing window and request native focus or taskbar attention.
    Activate,
}

/// Applies an explicit window command on the UI thread.
pub fn apply_window_command(context: &egui::Context, command: WindowCommand) {
    match command {
        WindowCommand::Activate => {
            context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            context.send_viewport_cmd(egui::ViewportCommand::Focus);
            context.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }
    }
}

/// A localised refusal notice that never starts application writers or workers.
pub struct AlreadyRunningNotice {
    catalogue: crate::api::Catalogue,
}

impl AlreadyRunningNotice {
    /// Creates the notice from the selected message catalogue.
    #[must_use]
    pub const fn new(catalogue: crate::api::Catalogue) -> Self {
        Self { catalogue }
    }
}

impl eframe::App for AlreadyRunningNotice {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label(self.catalogue.text("instance.already_running"));
            if ui
                .button(tallyline_egui::button_text(
                    self.catalogue.text("action.close"),
                ))
                .clicked()
            {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}
