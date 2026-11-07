// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawn shell regions and preserved navigation.

mod control_paint;
mod input_frame_paint;
mod resources;

use egui_kittest::{Harness, kittest::Queryable};
use tallyline_ui::api::model::BackgroundEvent;
use tallyline_ui::api::{Catalogue, ShellState, configure_fonts, show_shell};
use tallyline_workflows::api::model::AppSettings;

struct Fixture {
    shell: ShellState,
    commands: Vec<tallyline_ui::api::model::SettingsCommand>,
    setup_error: Option<String>,
}

fn shell(size: egui::Vec2) -> Result<Harness<'static, Fixture>, String> {
    configured_shell(size, tallyline_workflows::api::model::Theme::Light, 1.0)
}

fn configured_shell(
    size: egui::Vec2,
    theme: tallyline_workflows::api::model::Theme,
    zoom: f32,
) -> Result<Harness<'static, Fixture>, String> {
    let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
    let mut fonts = Some(resources::font_files()?);
    let mut unit = None;
    let mut state = ShellState::default();
    state.receive(BackgroundEvent::SettingsLoaded(
        AppSettings::from_persisted(theme, false),
    ));
    state.show_reference();
    let ui_theme = match theme {
        tallyline_workflows::api::model::Theme::Light => tallyline_egui::Theme::Light,
        tallyline_workflows::api::model::Theme::HighContrast => tallyline_egui::Theme::HighContrast,
        tallyline_workflows::api::model::Theme::Dark
        | tallyline_workflows::api::model::Theme::System => tallyline_egui::Theme::Dark,
    };
    let harness = Harness::builder().with_size(size).build_ui_state(
        move |ui, fixture: &mut Fixture| {
            if let Some(fonts) = fonts.take() {
                configure_fonts(ui.ctx(), fonts);
                tallyline_egui::apply_theme(
                    ui.ctx(),
                    ui_theme,
                    tallyline_egui::Density::Comfortable,
                    tallyline_egui::Motion::Reduced,
                );
                ui.ctx().set_zoom_factor(zoom);
                match resources::load_unit(ui.ctx(), &catalogue, ui_theme) {
                    Ok(loaded) => unit = Some(loaded),
                    Err(error) => {
                        fixture.setup_error = Some(error.to_string());
                        return;
                    }
                }
            }
            ui.set_style(ui.ctx().global_style());
            if let Some(unit) = &unit {
                fixture
                    .commands
                    .extend(show_shell(ui, &mut fixture.shell, &catalogue, unit));
            }
        },
        Fixture {
            shell: state,
            commands: Vec::new(),
            setup_error: None,
        },
    );
    if let Some(error) = &harness.state().setup_error {
        return Err(error.clone());
    }
    Ok(harness)
}
#[test]
fn shell_keeps_navigation_left_and_status_below_the_content() {
    let mut harness = shell(egui::vec2(1100.0, 800.0)).unwrap();
    harness.run();
    let title = harness.get_by_label("Tallyline").rect();
    let settings = harness.get_by_label("Settings").rect();
    let search = harness.get_by_label("Search terminals").rect();
    let status = harness.get_by_label("Saved submissions: 0").rect();
    assert!(title.bottom() < search.top());
    assert!(settings.right() < search.left());
    assert!(status.top() > search.bottom());
    assert!(status.bottom() <= 800.0);
}

#[test]
fn narrow_shell_retains_a_content_pane_and_every_navigation_action() {
    let mut harness = shell(egui::vec2(800.0, 640.0)).unwrap();
    harness.run();
    let settings = harness.get_by_label("Settings").rect();
    let search = harness.get_by_label("Search terminals").rect();
    assert!(
        settings.right() <= 64.0,
        "The narrow rail preserves content width: {settings:?}"
    );
    assert!(
        search.left() < 100.0,
        "Reference content follows the rail: {search:?}"
    );
    harness.get_by_label("Manual Report").click();
    harness.run();
    assert_eq!(
        harness.state().shell.view(),
        tallyline_ui::api::model::View::Manual
    );
    harness.get_by_label("Back to setup").click();
    harness.run();
    assert_eq!(
        harness.state().shell.view(),
        tallyline_ui::api::model::View::Onboarding
    );
}

#[path = "shell_layout/age.rs"]
mod age;

#[path = "shell_layout/keyboard.rs"]
mod keyboard;

#[path = "shell_layout/scale.rs"]
mod scale;

#[path = "shell_layout/notices.rs"]
mod notices;

#[path = "shell_layout/paint.rs"]
mod paint;

#[path = "shell_layout/settings.rs"]
mod settings;
