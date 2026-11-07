// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! First-written catalogue and legal-placement pinning tests.

mod resources;
mod support;

use egui_kittest::Harness;
use support::{
    RecordedTextures, RecordingRenderer, check_fan_kit_unit_survives_every_control, sha256_hex,
};
use tallyline_ui::api::model::LegalPlacement;
use tallyline_ui::api::{Catalogue, configure_fonts};

type Outcome = Result<(), String>;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_l10n_1_r_ui_19_q_app__every_catalogue_keeps_the_prescribed_notice_bytes() -> Outcome {
    let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
    assert_eq!(
        sha256_hex(catalogue.text("fan_kit.trademark").as_bytes()),
        support::TRADEMARK_LINE_SHA256
    );
    assert_eq!(
        sha256_hex(catalogue.text("fan_kit.agreement").as_bytes()),
        support::AGREEMENT_NOTICE_SHA256
    );
    assert!(
        catalogue
            .text("fan_kit.trademark")
            .contains("Cloud Imperium ®")
    );
    assert!(catalogue.text("fan_kit.agreement").contains("Ltd.."));
    assert_eq!(
        catalogue.text("about.copyright"),
        include_str!("../../../NOTICE")
            .lines()
            .nth(1)
            .ok_or("NOTICE has no copyright line")?
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_19_q_app__both_placements_keep_the_complete_legible_unmodified_unit() -> Outcome {
    for placement in [LegalPlacement::About, LegalPlacement::Onboarding] {
        let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
        let textures = RecordedTextures::default();
        let mut unit = None;
        let mut fonts = Some(resources::font_files()?);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(800.0, 900.0))
            .renderer(RecordingRenderer(textures.clone()))
            .build_ui(move |ui| {
                if unit.is_none() {
                    configure_fonts(ui.ctx(), fonts.take().unwrap());
                }
                let unit = unit.get_or_insert_with(|| {
                    resources::load_unit(ui.ctx(), &catalogue, tallyline_egui::Theme::Light)
                        .unwrap()
                });
                match placement {
                    LegalPlacement::About => {
                        let _actions = tallyline_ui::api::show_about_view(ui, &catalogue, unit);
                    }
                    LegalPlacement::Onboarding => {
                        let mut shell = tallyline_ui::api::ShellState::default();
                        shell.receive(tallyline_ui::api::model::BackgroundEvent::SettingsLoaded(
                            tallyline_workflows::api::model::AppSettings::default(),
                        ));
                        let _action =
                            tallyline_ui::api::show_onboarding_view(ui, &shell, &catalogue, unit);
                    }
                }
            });
        harness.run();
        check_fan_kit_unit_survives_every_control(&mut harness, &textures)?;
    }
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_7_q_theme_r_ui_19_q_app__legal_unit_is_legible_and_untinted_in_every_theme() -> Outcome {
    use tallyline_egui::{Density, Motion, Theme, apply_theme};
    for theme in [Theme::Light, Theme::Dark, Theme::HighContrast] {
        let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
        let textures = RecordedTextures::default();
        let mut unit = None;
        let mut fonts = Some(resources::font_files()?);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(800.0, 900.0))
            .renderer(RecordingRenderer(textures.clone()))
            .build_ui(move |ui| {
                if unit.is_none() {
                    configure_fonts(ui.ctx(), fonts.take().unwrap());
                    apply_theme(ui.ctx(), theme, Density::Comfortable, Motion::Reduced);
                }
                let unit = unit.get_or_insert_with(|| {
                    resources::load_unit(ui.ctx(), &catalogue, theme).unwrap()
                });
                unit.show_placement(ui, LegalPlacement::Onboarding, &catalogue);
            });
        harness.run();
        check_fan_kit_unit_survives_every_control(&mut harness, &textures)?;
    }
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_19_q_app__zoom_and_body_size_cannot_reduce_notice_legibility() -> Outcome {
    for (zoom, body_size) in [(0.5, 14.0), (1.0, 24.0)] {
        let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
        let textures = RecordedTextures::default();
        let mut unit = None;
        let mut fonts = Some(resources::font_files()?);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(800.0, 900.0))
            .renderer(RecordingRenderer(textures.clone()))
            .build_ui(move |ui| {
                if unit.is_none() {
                    configure_fonts(ui.ctx(), fonts.take().unwrap());
                    ui.ctx().set_zoom_factor(zoom);
                    let mut style = (*ui.ctx().global_style()).clone();
                    style
                        .text_styles
                        .insert(egui::TextStyle::Body, egui::FontId::proportional(body_size));
                    ui.ctx().set_style_of(ui.ctx().theme(), style);
                }
                let unit = unit.get_or_insert_with(|| {
                    resources::load_unit(ui.ctx(), &catalogue, tallyline_egui::Theme::Light)
                        .unwrap()
                });
                unit.show_placement(ui, LegalPlacement::Onboarding, &catalogue);
            });
        harness.run();
        check_fan_kit_unit_survives_every_control(&mut harness, &textures)?;
    }
    Ok(())
}
