// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn credential fixture through the shared public shell boundary.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tallyline_submission::api::model::{AccountState, Persistence};
use tallyline_ui::api::model::BackgroundEvent;
use tallyline_ui::api::{Catalogue, ShellState};

/// Opens the shared credential form with synthetic inputs.
///
/// # Errors
/// Returns catalogue, font or artwork fixture failures.
pub fn form(
    theme: tallyline_egui::Theme,
    zoom: f32,
) -> Result<Harness<'static, ShellState>, String> {
    let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
    let mut fonts = Some(crate::resources::font_files()?);
    let artwork: &[u8] = match theme {
        tallyline_egui::Theme::Light => {
            include_bytes!("../../../../docs/images/fankit/made-by-the-community.png")
        }
        tallyline_egui::Theme::Dark | tallyline_egui::Theme::HighContrast => {
            include_bytes!("../../../../docs/images/fankit/MadeByTheCommunity_White.png")
        }
    };
    let mut pixels =
        Some(crate::resources::decode_logo(artwork).map_err(|error| error.to_string())?);
    let mut unit = None;
    let mut shell = ShellState::default();
    let preference = match theme {
        tallyline_egui::Theme::Light => tallyline_workflows::api::model::Theme::Light,
        tallyline_egui::Theme::Dark => tallyline_workflows::api::model::Theme::Dark,
        tallyline_egui::Theme::HighContrast => tallyline_workflows::api::model::Theme::HighContrast,
    };
    shell.receive(BackgroundEvent::SettingsLoaded(
        tallyline_workflows::api::model::AppSettings::from_persisted(preference, false),
    ));
    shell.receive(BackgroundEvent::CredentialsUpdated {
        account: AccountState::Unconfigured,
        failure: None,
        available_token: false,
        fallback_available: true,
    });
    shell.credentials_mut().set_input(
        "dummy-key-mask-probe".to_owned(),
        Some("dummy-token-mask-probe".to_owned()),
        Persistence::Session,
    );
    shell.show_settings();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 640.0))
        .with_max_steps(30)
        .build_ui_state(
            move |ui, shell| {
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
                ui.set_style(ui.ctx().global_style());
                if let Some(unit) = &unit {
                    tallyline_ui::api::show_shell(ui, shell, &catalogue, unit);
                }
            },
            shell,
        );
    harness.run();
    harness.get_by_label("UEX account").focus();
    harness.run();
    super::visibility::assert_focused_visible(&harness, "UEX account")?;
    harness.key_press(egui::Key::Space);
    harness.run();
    Ok(harness)
}

/// Returns whether literal fixture content appears in drawn or accessible output.
pub fn exposed(harness: &Harness<'_, ShellState>, value: &str) -> bool {
    format!(
        "{:?} {:?}",
        harness.output().shapes,
        harness.output().platform_output.accesskit_update
    )
    .contains(value)
}
