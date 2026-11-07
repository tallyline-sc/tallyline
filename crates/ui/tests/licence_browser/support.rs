// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn legal-browser fixtures through the public shell boundary.

use crate::resources;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tallyline_ui::api::model::BackgroundEvent;
use tallyline_ui::api::{Catalogue, ShellState};

/// Supplies exact synthetic legal documents and upstream metadata.
///
/// # Errors
/// Returns a catalogue parse failure.
pub fn catalogue() -> Result<
    tallyline_ui::api::model::LicenceCatalogue,
    tallyline_ui::api::model::LicenceCatalogueError,
> {
    tallyline_ui::api::parse_licence_catalogue(
        r#"{"schema_version":1,"components":[{"name":"Alpha","version":"1.2.3","licence":"MIT","declared_licence":"MIT OR Apache-2.0","source":"https://example.invalid/alpha","copyright":{"status":"provided","notices":["Copyright 2025 Alpha Authors"]},"notices":[{"path":"Alpha/LICENSE","text":"Exact Alpha licence\n\nSecond paragraph."},{"path":"Alpha/NOTICE","text":"Exact Alpha attribution."}]},{"name":"Beta","version":"4.5.6","licence":"Apache-2.0","source":"https://example.invalid/beta","copyright":{"status":"not_stated_by_upstream"},"notices":[{"path":"Beta/LICENSE","text":"Exact Beta licence."}]}] }"#,
    )
}

/// Opens the default licence browser through the existing About action.
///
/// # Errors
/// Returns a catalogue or bundled-resource fixture failure.
pub fn browser() -> Result<Harness<'static, ShellState>, String> {
    browser_with(
        Some(self::catalogue().map_err(|error| format!("{error:?}"))?),
        tallyline_egui::Theme::Dark,
        1.0,
    )
}

/// Opens a themed and scaled legal browser with a supplied offline catalogue.
///
/// # Errors
/// Returns a bundled catalogue, font or artwork failure.
pub fn browser_with(
    licences: Option<tallyline_ui::api::model::LicenceCatalogue>,
    theme: tallyline_egui::Theme,
    zoom: f32,
) -> Result<Harness<'static, ShellState>, String> {
    let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
    let mut fonts = Some(resources::font_files()?);
    let artwork: &[u8] = match theme {
        tallyline_egui::Theme::Light => {
            include_bytes!("../../../../docs/images/fankit/made-by-the-community.png")
        }
        tallyline_egui::Theme::Dark | tallyline_egui::Theme::HighContrast => {
            include_bytes!("../../../../docs/images/fankit/MadeByTheCommunity_White.png")
        }
    };
    let mut pixels = Some(resources::decode_logo(artwork).map_err(|error| error.to_string())?);
    let mut unit = None;
    let mut shell = ShellState::default();
    if let Some(licences) = licences {
        shell.receive(BackgroundEvent::LicencesLoaded(licences));
    }
    let preference = match theme {
        tallyline_egui::Theme::Dark => tallyline_workflows::api::model::Theme::Dark,
        tallyline_egui::Theme::Light => tallyline_workflows::api::model::Theme::Light,
        tallyline_egui::Theme::HighContrast => tallyline_workflows::api::model::Theme::HighContrast,
    };
    shell.receive(BackgroundEvent::SettingsLoaded(
        tallyline_workflows::api::model::AppSettings::from_persisted(preference, false),
    ));
    shell.open_about();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 640.0))
        .with_max_steps(20)
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                    tallyline_egui::apply_theme(
                        ui.ctx(),
                        theme,
                        tallyline_egui::Density::Comfortable,
                        tallyline_egui::Motion::Reduced,
                    );
                    ui.ctx().set_zoom_factor(zoom);
                }
                if let Some(pixels) = pixels.take() {
                    unit = Some(tallyline_ui::api::FanKitUnit::new(
                        ui.ctx(),
                        &catalogue,
                        pixels,
                    ));
                }
                if let Some(unit) = &unit {
                    state.show_dialogs(ui.ctx(), &catalogue, unit);
                }
            },
            shell,
        );
    harness.run();
    harness.get_by_label("Open-source licences").focus();
    harness.run();
    harness.key_press(egui::Key::Space);
    harness.run();
    Ok(harness)
}

/// Returns the painted baseline origin of an exact legal-text prefix.
pub fn text_origin(harness: &Harness<'_, ShellState>, prefix: &str) -> Option<f32> {
    harness.output().shapes.iter().find_map(|shape| {
        if let egui::epaint::Shape::Text(text) = &shape.shape
            && text.galley.job.text.starts_with(prefix)
        {
            Some(text.pos.y)
        } else {
            None
        }
    })
}

/// Copies the focused document through its keyboard and clipboard output.
pub fn copy_document<'a>(harness: &'a mut Harness<'_, ShellState>) -> Option<&'a str> {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    harness.input_mut().events.push(egui::Event::Copy);
    harness.step();
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|command| {
            if let egui::OutputCommand::CopyText(text) = command {
                Some(text.as_str())
            } else {
                None
            }
        })
}
