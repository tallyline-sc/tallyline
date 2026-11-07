// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dialog state changed only by direct user actions.

/// The single owner of user-requested dialogs.
#[derive(Default)]
pub struct DialogPresenter {
    about: bool,
    build: crate::api::model::BuildPresentation,
    gpl: bool,
    gpl_viewer: crate::licence_text_viewer::LicenceTextViewer,
    licences: bool,
    licence_browser: crate::licence_browser::LicenceBrowser,
    catalogue: Option<crate::api::model::LicenceCatalogue>,
    pub(crate) manual_confirmation: Option<crate::api::model::ManualReviewCommand>,
}

impl DialogPresenter {
    pub(crate) fn set_build_presentation(&mut self, build: crate::api::model::BuildPresentation) {
        self.build = build;
    }

    /// Returns the validated offline component catalogue.
    #[must_use]
    pub const fn licences(&self) -> Option<&crate::api::model::LicenceCatalogue> {
        self.catalogue.as_ref()
    }

    pub(crate) fn set_licences(&mut self, catalogue: Option<crate::api::model::LicenceCatalogue>) {
        self.catalogue = catalogue;
    }
    /// Opens About after a direct user action.
    pub fn open_about(&mut self) {
        self.about = true;
    }

    /// Draws user-requested legal dialogs without background-triggered window commands.
    pub fn show(
        &mut self,
        context: &egui::Context,
        catalogue: &crate::api::Catalogue,
        unit: &crate::api::FanKitUnit,
        theme: tallyline_egui::Theme,
    ) {
        egui::Window::new(catalogue.text("about.title"))
            .open(&mut self.about)
            .resizable(true)
            .vscroll(true)
            .default_width(760.0)
            .show(context, |ui| {
                for action in
                    crate::legal_views::show_about_view_with_build(ui, catalogue, unit, &self.build)
                {
                    match action {
                        crate::api::model::AboutAction::ShowGpl => self.gpl = true,
                        crate::api::model::AboutAction::ShowLicences => self.licences = true,
                    }
                }
            });
        egui::Window::new(catalogue.text("about.gpl"))
            .open(&mut self.gpl)
            .default_size([720.0, 500.0])
            .show(context, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.gpl_viewer
                            .show(ui, catalogue, include_str!("../../../LICENSE"));
                    });
            });
        egui::Window::new(catalogue.text("about.licences"))
            .open(&mut self.licences)
            .default_size([720.0, 520.0])
            .show(context, |ui| {
                if let Some(licences) = &self.catalogue {
                    self.licence_browser
                        .show(ui, catalogue, licences, theme.palette());
                } else {
                    ui.label(catalogue.text("about.missing_licences"));
                }
            });
    }
    /// Returns whether the user opened About.
    #[must_use]
    pub const fn about_open(&self) -> bool {
        self.about
    }
}
