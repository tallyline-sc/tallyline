// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The coupled and unmodified Star Citizen Fan Kit unit.

use crate::api::Catalogue;
use egui::{ColorImage, FontId, Label, RichText, TextureHandle, TextureOptions, Ui};

const NOTICE_FONT_SIZE: f32 = 14.0;
const LOGO_SIZE: f32 = 96.0;

/// Accessible name of the prescribed logo.
pub const FAN_KIT_LOGO_ALT_TEXT: &str = "Made By The Community";
/// Fixed texture name used by the paint pinning tests.
pub const FAN_KIT_LOGO_TEXTURE_NAME: &str = "fan-kit-logo";

/// A required legal-unit placement.
#[derive(Clone, Copy)]
pub enum LegalPlacement {
    /// The About dialog.
    About,
    /// The first onboarding screen.
    Onboarding,
}

/// One component that always renders all three prescribed parts.
pub struct FanKitUnit {
    logo: TextureHandle,
    trademark: String,
    agreement: String,
}

impl FanKitUnit {
    /// Uploads the composition root's verified, unchanged logo pixels and base notices.
    #[must_use]
    pub fn new(context: &egui::Context, catalogue: &Catalogue, pixels: ColorImage) -> Self {
        Self {
            logo: context.load_texture(FAN_KIT_LOGO_TEXTURE_NAME, pixels, TextureOptions::LINEAR),
            trademark: catalogue.text("fan_kit.trademark").to_owned(),
            agreement: catalogue.text("fan_kit.agreement").to_owned(),
        }
    }
    /// Draws the entire legal unit as the only three children of one group.
    pub fn show(&self, ui: &mut Ui) {
        let font_size = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(NOTICE_FONT_SIZE / ui.ctx().zoom_factor());
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.add(
                    egui::Image::new((self.logo.id(), egui::vec2(LOGO_SIZE, LOGO_SIZE)))
                        .alt_text(FAN_KIT_LOGO_ALT_TEXT),
                );
                for notice in [&self.trademark, &self.agreement] {
                    ui.add(
                        Label::new(RichText::new(notice).font(FontId::proportional(font_size)))
                            .wrap(),
                    );
                }
            });
        });
    }

    /// Draws a required placement without separating or hiding the legal unit.
    pub fn show_placement(&self, ui: &mut Ui, placement: LegalPlacement, catalogue: &Catalogue) {
        ui.heading(catalogue.text(match placement {
            LegalPlacement::About => "about.title",
            LegalPlacement::Onboarding => "onboarding.title",
        }));
        self.show(ui);
    }
}
