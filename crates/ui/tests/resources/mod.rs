// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared legal placement fixture construction.

mod common;

pub use common::{decode_logo, font_files};

pub fn load_unit(
    context: &egui::Context,
    catalogue: &tallyline_ui::api::Catalogue,
    theme: tallyline_egui::Theme,
) -> Result<tallyline_ui::api::FanKitUnit, png::DecodingError> {
    let bytes: &[u8] = match theme {
        tallyline_egui::Theme::Light => {
            include_bytes!("../../../../docs/images/fankit/made-by-the-community.png")
        }
        tallyline_egui::Theme::Dark | tallyline_egui::Theme::HighContrast => {
            include_bytes!("../../../../docs/images/fankit/MadeByTheCommunity_White.png")
        }
    };
    Ok(tallyline_ui::api::FanKitUnit::new(
        context,
        catalogue,
        decode_logo(bytes)?,
    ))
}
