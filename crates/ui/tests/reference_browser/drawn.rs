// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public reference-browser drawn regressions.

use super::{
    BackgroundEvent, CachedReference, EndpointData, FixedClock, MemoryCache, ReferenceApplication,
    ShellState, Timestamp, UexEnvironment, resources,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_2_r_api_6_r_cap_3a__drawn_browser_selects_ptu_only_records_and_dispatches_terminal_prices()
{
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_reference_data::api::model::{Terminal, TerminalText};
    use tallyline_shared_kernel::api::model::TerminalId;
    use tallyline_ui::api::model::ReferenceCommand;
    use tallyline_ui::api::{Catalogue, show_reference_browser};
    let app = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(60),
            EndpointData::Terminals(vec![Terminal::new(
                TerminalId(12),
                TerminalText {
                    name: "Orbital Depot".to_owned(),
                    nickname: "Depot".to_owned(),
                    display_name: "Orbital Depot".to_owned(),
                    location: "Pyro".to_owned(),
                },
                false,
            )]),
        )]),
        FixedClock,
    )
    .unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    });
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(resources::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 900.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_reference_browser(ui, state, &catalogue);
            },
            shell,
        );
    harness.run();
    assert!(
        harness
            .query_by_label("Orbital Depot · Pyro · UEX ID 12")
            .is_none()
    );
    harness.get_by_label("EPTU").click();
    harness.run();
    harness.get_by_label("Search terminals").focus();
    harness.run();
    harness
        .get_by_label("Search terminals")
        .type_text("missing");
    harness.run();
    assert!(
        harness
            .query_by_label("Orbital Depot · Pyro · UEX ID 12")
            .is_none()
    );
    harness
        .state_mut()
        .references_mut()
        .terminal_query()
        .clear();
    harness.run();
    harness.get_by_label("Search terminals").type_text("pYrO");
    harness.run();
    harness
        .get_by_label("Orbital Depot · Pyro · UEX ID 12")
        .click();
    harness.run();
    assert_eq!(
        harness.state_mut().references_mut().take_command(),
        Some(ReferenceCommand::OpenTerminal(TerminalId(12)))
    );
    assert_eq!(
        harness.state().references().selected_terminal(),
        Some(TerminalId(12))
    );
    assert_eq!(
        harness.state().game_witness().settings.witness,
        tallyline_game::api::model::WitnessMode::Off
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_16_r_ui_17__background_refresh_keeps_the_search_field_focused_and_accepting_text() {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_ui::api::{Catalogue, show_reference_browser};
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(resources::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 900.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_reference_browser(ui, state, &catalogue);
            },
            ShellState::default(),
        );
    harness.run();
    harness.get_by_label("Search terminals").focus();
    harness.run();
    harness.get_by_label("Search terminals").type_text("Area");
    harness.run();
    assert_eq!(harness.state().references().terminal_search(), "Area");
    harness.key_press(egui::Key::End);
    harness.run();
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceRefreshStarted);
    harness.run();
    assert!(harness.get_by_label("Search terminals").is_focused());
    harness.get_by_label("Search terminals").type_text("18");
    harness.run();
    assert_eq!(harness.state().references().terminal_search(), "Area18");
}
