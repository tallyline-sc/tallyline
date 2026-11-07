// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public reference-browser status regressions.

use super::{
    BackgroundEvent, CachedReference, Commodity, CommodityAverages, CommodityId, EndpointData,
    FixedClock, MemoryCache, ReferenceApplication, ShellState, Timestamp, UexEnvironment, View,
    resources,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_ui_8_r_ui_13__empty_cache_and_offline_refresh_report_real_data_age() {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_reference_data::api::model::Endpoint;
    use tallyline_ui::api::model::ReferenceCommand;
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
    assert!(harness.query_by_label("No reference data yet").is_some());
    let mut app = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(60),
            EndpointData::Commodities(vec![Commodity::new(
                CommodityId(42),
                "Agricium",
                "AGRI",
                true,
                CommodityAverages::default(),
            )]),
        )]),
        FixedClock,
    )
    .unwrap();
    app.refresh_failed(Endpoint::Commodities);
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceUpdated {
            live: app.snapshot(UexEnvironment::Live),
            ptu: app.snapshot(UexEnvironment::Ptu),
            observed_at: Timestamp::from_seconds(120),
        });
    harness.run();
    assert!(
        harness
            .query_by_label("Offline - cached reference data remains available")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Commodities: Age at last update: 60 seconds")
            .is_some()
    );
    harness.get_by_label("Refresh reference data").click();
    harness.run();
    assert_eq!(
        harness.state_mut().references_mut().take_command(),
        Some(ReferenceCommand::Refresh)
    );
    assert!(
        harness
            .query_by_label("Agricium · AGRI · UEX ID 42")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Refreshing reference data (1 request)")
            .is_some()
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceUpdated {
            live: app.snapshot(UexEnvironment::Live),
            ptu: app.snapshot(UexEnvironment::Ptu),
            observed_at: Timestamp::from_seconds(30),
        });
    harness.run();
    assert!(
        harness
            .query_by_label("Commodities: Age unknown - clock mismatch")
            .is_some()
    );
    assert!(!harness.state().references().refreshing());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_8__reference_view_shows_the_global_cache_status_once() {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_ui::api::{Catalogue, configure_fonts, show_shell};
    let app = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(60),
            EndpointData::Commodities(Vec::new()),
        )]),
        FixedClock,
    )
    .unwrap();
    let mut state = ShellState::default();
    state.receive(BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    });
    state.show_reference();
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(resources::font_files().unwrap());
    let mut unit = None;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 900.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    configure_fonts(ui.ctx(), fonts);
                }
                let unit = unit.get_or_insert_with(|| {
                    resources::load_unit(ui.ctx(), &catalogue, tallyline_egui::Theme::Light)
                        .unwrap()
                });
                show_shell(ui, state, &catalogue, unit);
            },
            state,
        );
    harness.run();
    assert_eq!(
        harness
            .get_all_by_label("Cached reference data available")
            .count(),
        1
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_8_r_api_6__staff_notice_is_capped_literal_text_and_closed_submissions_leave_browsing_available()
 {
    use egui_kittest::{Harness, kittest::Queryable};
    use std::num::NonZeroU32;
    use tallyline_reference_data::api::model::{
        AcceptanceSwitch, CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
    };
    use tallyline_ui::api::{Catalogue, configure_fonts, show_shell};
    let text = format!("<b>Maintenance</b>{}", "\u{00e9}".repeat(300));
    let expected = format!("<b>Maintenance</b>{}", "\u{00e9}".repeat(262));
    let parameters = ReferenceParameters::new(
        SubmissionAcceptance {
            datacenter: AcceptanceSwitch::Disabled,
            reports: AcceptanceSwitch::Enabled,
            commodity: AcceptanceSwitch::Enabled,
            ptu: AcceptanceSwitch::Enabled,
        },
        CommodityConstraints {
            price_variation_percent: 25,
            prior_ttl_days: NonZeroU32::new(15).unwrap(),
            scu_variation_percent: 5000,
        },
        Some(&text),
    );
    let app = ReferenceApplication::load(
        MemoryCache(vec![
            CachedReference::new(
                Timestamp::from_seconds(60),
                EndpointData::Parameters(parameters),
            ),
            CachedReference::new(
                Timestamp::from_seconds(60),
                EndpointData::Commodities(vec![Commodity::new(
                    CommodityId(42),
                    "Agricium",
                    "AGRI",
                    true,
                    CommodityAverages::default(),
                )]),
            ),
        ]),
        FixedClock,
    )
    .unwrap();
    let mut state = ShellState::default();
    state.receive(BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    });
    state.show_reference();
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(resources::font_files().unwrap());
    let mut unit = None;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 1000.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    configure_fonts(ui.ctx(), fonts);
                }
                let unit = unit.get_or_insert_with(|| {
                    resources::load_unit(ui.ctx(), &catalogue, tallyline_egui::Theme::Light)
                        .unwrap()
                });
                show_shell(ui, state, &catalogue, unit);
            },
            state,
        );
    harness.run();
    assert!(harness.query_by_label(&expected).is_some());
    harness.get_by_label("Reference details").click();
    harness.run();
    assert!(
        harness
            .query_by_label("Parameters: Age at last update: 60 seconds")
            .is_some()
    );
    harness.get_by_label("Agricium · AGRI · UEX ID 42").click();
    harness.run();
    assert_eq!(
        harness.state().references().selected_commodity(),
        Some(CommodityId(42))
    );
    harness.state_mut().show_settings();
    harness.run();
    assert!(harness.query_by_label(&expected).is_some());
    assert_eq!(harness.state().view(), View::Settings);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_3_r_ui_16__historical_ray_and_time_are_plain_global_text_and_preserve_browsing() {
    use egui_kittest::{Harness, kittest::Queryable};
    use tallyline_ui::api::model::ReferenceDiagnostic;
    use tallyline_ui::api::{Catalogue, configure_fonts, show_shell};
    let app = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(60),
            EndpointData::Commodities(vec![Commodity::new(
                CommodityId(42),
                "Agricium",
                "AGRI",
                true,
                CommodityAverages::default(),
            )]),
        )]),
        FixedClock,
    )
    .unwrap();
    let mut state = ShellState::default();
    state.receive(BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    });
    state.show_reference();
    state
        .references_mut()
        .choose_channel(tallyline_game::api::model::GameEnvironment::Eptu);
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(resources::font_files().unwrap());
    let mut unit = None;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 1100.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    configure_fonts(ui.ctx(), fonts);
                }
                let unit = unit.get_or_insert_with(|| {
                    resources::load_unit(ui.ctx(), &catalogue, tallyline_egui::Theme::Light)
                        .unwrap()
                });
                show_shell(ui, state, &catalogue, unit);
            },
            state,
        );
    harness.run();
    harness.get_by_label("Agricium · AGRI · UEX ID 42").click();
    harness.run();
    harness.get_by_label("Search commodities").focus();
    harness.run();
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceDiagnostic(ReferenceDiagnostic {
            http_status: 503,
            cf_ray: Some("230b030023ae2822-SJC".to_owned()),
            observed_at: Timestamp::from_millis(123456),
        }));
    harness.run();
    let expected = "Last non-JSON reference response: HTTP 503; cf-ray: 230b030023ae2822-SJC; UTC Unix ms: 123456";
    assert!(harness.query_by_label(expected).is_some());
    assert!(harness.get_by_label("Search commodities").is_focused());
    assert_eq!(
        harness.state().references().channel(),
        tallyline_game::api::model::GameEnvironment::Eptu
    );
    assert_eq!(
        harness.state().references().selected_commodity(),
        Some(CommodityId(42))
    );
    assert_eq!(harness.state().view(), View::Reference);
    assert!(
        harness
            .state_mut()
            .references_mut()
            .take_command()
            .is_none()
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::ReferenceUpdated {
            live: app.snapshot(UexEnvironment::Live),
            ptu: app.snapshot(UexEnvironment::Ptu),
            observed_at: Timestamp::from_seconds(121),
        });
    harness.state_mut().show_settings();
    harness.run();
    assert!(harness.query_by_label(expected).is_some());
    assert_eq!(harness.state().view(), View::Settings);
}
