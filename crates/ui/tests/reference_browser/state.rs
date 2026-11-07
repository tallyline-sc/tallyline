// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public reference-browser state regressions.

use super::{
    BackgroundEvent, CachedReference, Commodity, CommodityAverages, CommodityId, EndpointData,
    FixedClock, GameEnvironment, MemoryCache, ReferenceApplication, ShellState, Timestamp,
    UexEnvironment, View,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_ui_16__passive_refresh_preserves_environment_search_selection_and_settings_view() {
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
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    });
    shell.references_mut().choose_channel(GameEnvironment::Eptu);
    *shell.references_mut().commodity_query() = "AGRI".to_owned();
    assert!(shell.select_reference_commodity(CommodityId(42)));
    shell.show_settings();
    shell.receive(BackgroundEvent::ReferenceRefreshStarted);
    shell.receive(BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(180),
    });
    assert_eq!(shell.view(), View::Settings);
    assert_eq!(shell.references().channel(), GameEnvironment::Eptu);
    assert_eq!(shell.references().commodity_search(), "AGRI");
    assert_eq!(
        shell.references().selected_commodity(),
        Some(CommodityId(42))
    );
    assert_eq!(
        shell.reference_snapshot().unwrap().environment(),
        UexEnvironment::Ptu
    );
    assert!(!shell.dialogs().about_open());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_cap_3a_r_nf_5__mapping_changes_wait_for_durable_confirmation_without_enabling_witness() {
    use tallyline_ui::api::model::{GameWitnessView, SettingsCommand, UserAction};
    use tallyline_workflows::api::model::AppSettings;
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    assert_eq!(
        shell.act(UserAction::ChangeEnvironmentMapping(
            GameEnvironment::Live,
            UexEnvironment::Ptu
        )),
        Some(SettingsCommand::ChangeEnvironmentMapping(
            GameEnvironment::Live,
            UexEnvironment::Ptu
        ))
    );
    assert_eq!(
        shell
            .game_witness()
            .settings
            .environment_mapping
            .environment(GameEnvironment::Live),
        UexEnvironment::Live
    );
    shell.receive(BackgroundEvent::Failure("Save failed".to_owned()));
    assert_eq!(
        shell
            .game_witness()
            .settings
            .environment_mapping
            .environment(GameEnvironment::Live),
        UexEnvironment::Live
    );
    assert!(
        shell
            .act(UserAction::ChangeEnvironmentMapping(
                GameEnvironment::Live,
                UexEnvironment::Ptu
            ))
            .is_some()
    );
    let mut durable = GameWitnessView::default();
    durable.settings.environment_mapping = durable
        .settings
        .environment_mapping
        .with_environment(GameEnvironment::Live, UexEnvironment::Ptu);
    shell.receive(BackgroundEvent::GameWitnessLoaded(durable));
    assert_eq!(
        shell
            .game_witness()
            .settings
            .environment_mapping
            .environment(GameEnvironment::Live),
        UexEnvironment::Ptu
    );
    assert_eq!(
        shell.game_witness().settings.witness,
        tallyline_game::api::model::WitnessMode::Off
    );
    assert!(!shell.busy());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_8_r_ui_16__reference_delivery_recovery_preserves_unrelated_action_notices() {
    let app = ReferenceApplication::load(MemoryCache(Vec::new()), FixedClock).unwrap();
    let publish = || BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    };
    let mut shell = ShellState::default();
    shell.reference_command_failed("Reference data refresh unavailable".to_owned());
    assert_eq!(shell.notice(), Some("Reference data refresh unavailable"));
    shell.receive(publish());
    assert_eq!(shell.notice(), None);
    shell.reference_command_failed("Reference data refresh unavailable".to_owned());
    shell.receive(BackgroundEvent::Failure(
        "Settings could not be saved".to_owned(),
    ));
    shell.receive(publish());
    assert_eq!(shell.notice(), Some("Settings could not be saved"));
}
