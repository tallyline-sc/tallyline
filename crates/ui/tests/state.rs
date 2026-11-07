// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shell state and command acceptance tests.

use tallyline_ui::api::ShellState;
use tallyline_ui::api::model::{BackgroundEvent, View};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_8_r_ui_16__background_failures_are_persistent_state_without_dialog_requests() {
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::Failure(
        "Storage self-test failed".to_owned(),
    ));
    assert_eq!(shell.notice(), Some("Storage self-test failed"));
    assert_eq!(shell.view(), View::Onboarding);
    assert!(!shell.dialogs().about_open());
    shell.show_settings();
    assert_eq!(shell.notice(), Some("Storage self-test failed"));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_7_q_theme_r_nf_5__theme_is_applied_only_when_the_worker_confirms_the_save() {
    use tallyline_ui::api::model::{SettingsCommand, UserAction};
    use tallyline_workflows::api::model::{AppSettings, Theme};
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    assert_eq!(
        shell.act(UserAction::ChooseTheme(Theme::Light)),
        Some(SettingsCommand::ChangeTheme(Theme::Light))
    );
    assert_eq!(shell.settings().theme(), Theme::System);
    shell.receive(BackgroundEvent::SettingsLoaded(
        AppSettings::from_persisted(Theme::Light, false),
    ));
    assert_eq!(shell.settings().theme(), Theme::Light);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18_r_ui_16__offline_licences_load_without_opening_about() {
    let catalogue = tallyline_ui::api::parse_licence_catalogue(r#"{"schema_version":1,"components":[{"name":"Example","version":"1.2.3","licence":"MIT","source":"https://example.invalid/source","copyright":{"status":"provided","notices":["Copyright 2025 Example Authors"]},"notices":[{"path":"example/LICENSE","text":"Exact licence text"}]}]}"#).unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::LicencesLoaded(catalogue));
    assert!(!shell.dialogs().about_open());
    let component = shell
        .dialogs()
        .licences()
        .unwrap()
        .components()
        .first()
        .unwrap();
    assert_eq!(component.name, "Example");
    assert_eq!(component.version, "1.2.3");
    assert_eq!(component.licence, "MIT");
    assert_eq!(component.source, "https://example.invalid/source");
    assert_eq!(
        component.copyright,
        tallyline_ui::api::model::CopyrightInformation::Provided {
            notices: vec!["Copyright 2025 Example Authors".to_owned()]
        }
    );
    assert_eq!(
        component.notices.first().unwrap().text,
        "Exact licence text"
    );
    shell.receive(BackgroundEvent::LicencesUnavailable);
    assert!(shell.dialogs().licences().is_none());
    assert!(!shell.dialogs().about_open());
    assert!(
        tallyline_ui::api::parse_licence_catalogue(r#"{"schema_version":1,"components":[]}"#)
            .is_err()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_8_r_nf_5__settings_commands_wait_for_successful_background_startup() {
    use tallyline_ui::api::model::UserAction;
    use tallyline_workflows::api::model::AppSettings;
    let mut shell = ShellState::default();
    assert!(!shell.ready());
    assert_eq!(shell.act(UserAction::ContinueOnboarding), None);
    shell.receive(BackgroundEvent::Failure(
        "Directory self-test failed".to_owned(),
    ));
    assert_eq!(shell.act(UserAction::ContinueOnboarding), None);
    shell.receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    assert!(shell.ready());
    assert_eq!(shell.notice(), None);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_7_q_theme__system_theme_events_resolve_preferences_without_overriding_explicit_choices() {
    use tallyline_egui::Theme as Resolved;
    use tallyline_workflows::api::model::Theme;
    assert_eq!(
        tallyline_ui::api::resolve_theme(Theme::System, Some(egui::Theme::Light)),
        Resolved::Light
    );
    assert_eq!(
        tallyline_ui::api::resolve_theme(Theme::System, Some(egui::Theme::Dark)),
        Resolved::Dark
    );
    assert_eq!(
        tallyline_ui::api::resolve_theme(Theme::Dark, Some(egui::Theme::Light)),
        Resolved::Dark
    );
    assert_eq!(
        tallyline_ui::api::resolve_theme(Theme::Light, Some(egui::Theme::Dark)),
        Resolved::Light
    );
    assert_eq!(
        tallyline_ui::api::resolve_theme(Theme::HighContrast, Some(egui::Theme::Light)),
        Resolved::HighContrast
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18__the_catalogue_distinguishes_verified_absence_from_provided_copyright() {
    let catalogue = tallyline_ui::api::parse_licence_catalogue(r#"{"schema_version":1,"components":[{"name":"Example","version":"1.2.3","licence":"MIT","source":"https://example.invalid/source","copyright":{"status":"not_stated_by_upstream"},"notices":[{"path":"LICENSE","text":"Permission text without a named holder"}]}]}"#).unwrap();
    assert!(matches!(
        catalogue.components().first().unwrap().copyright,
        tallyline_ui::api::model::CopyrightInformation::NotStatedByUpstream
    ));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_8__completed_onboarding_enters_the_workspace_and_settings_has_a_return_route() {
    use tallyline_workflows::api::model::{AppSettings, Theme};
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(
        AppSettings::from_persisted(Theme::System, true),
    ));
    assert_eq!(shell.view(), View::Home);
    shell.show_settings();
    assert_eq!(shell.view(), View::Settings);
    shell.leave_settings();
    assert_eq!(shell.view(), View::Home);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_19_q_app__leaving_settings_before_acknowledgement_returns_to_onboarding() {
    use tallyline_workflows::api::model::AppSettings;
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    shell.show_settings();
    shell.leave_settings();
    assert_eq!(shell.view(), View::Onboarding);
    assert!(!shell.settings().onboarding_complete());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_16__a_background_save_does_not_pull_the_user_back_into_settings() {
    use tallyline_ui::api::model::UserAction;
    use tallyline_workflows::api::model::{AppSettings, Theme};
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(
        AppSettings::from_persisted(Theme::System, true),
    ));
    shell.show_settings();
    let _command = shell.act(UserAction::ChooseTheme(Theme::Light));
    shell.leave_settings();
    shell.receive(BackgroundEvent::SettingsLoaded(
        AppSettings::from_persisted(Theme::Light, true),
    ));
    assert_eq!(shell.view(), View::Home);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18__the_elected_licence_keeps_its_separate_upstream_declaration() {
    let parsed = tallyline_ui::api::parse_licence_catalogue(
        r#"{"schema_version":1,"components":[{"name":"Example","version":"1.2.3","licence":"MIT","declared_licence":"MIT OR Apache-2.0","source":"https://example.invalid/source","copyright":{"status":"not_stated_by_upstream"},"notices":[{"path":"LICENSE","text":"Original licence text"}]}]}"#,
    );
    assert!(parsed.is_ok(), "{parsed:?}");
    let catalogue = parsed.unwrap();
    let component = catalogue.components().first().unwrap();
    assert_eq!(component.licence, "MIT");
    assert_eq!(
        component.declared_licence.as_deref(),
        Some("MIT OR Apache-2.0")
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20_r_ui_16__witness_opt_in_waits_for_worker_confirmation_and_never_opens_a_dialog() {
    use tallyline_game::api::model::{
        GameSettings, WitnessMode, WitnessSnapshot, WitnessUnavailable,
    };
    use tallyline_ui::api::model::{GameWitnessView, SettingsCommand, UserAction};
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(
        tallyline_workflows::api::model::AppSettings::default(),
    ));
    shell.show_settings();
    assert_eq!(
        shell.act(UserAction::ChangeWitnessMode(WitnessMode::On)),
        Some(SettingsCommand::ChangeWitnessMode(WitnessMode::On))
    );
    assert_eq!(shell.game_witness().settings.witness, WitnessMode::Off);
    let view = GameWitnessView {
        settings: GameSettings {
            witness: WitnessMode::On,
            ..GameSettings::default()
        },
        channel_directory: Some("SYNTHETIC_PRIVATE_PATH".to_owned()),
        snapshot: WitnessSnapshot::Unavailable(WitnessUnavailable::UnvalidatedBuild),
    };
    assert!(!format!("{view:?}").contains("SYNTHETIC_"));
    shell.receive(BackgroundEvent::GameWitnessLoaded(view));
    assert_eq!(shell.game_witness().settings.witness, WitnessMode::On);
    assert_eq!(shell.view(), View::Settings);
    assert!(!shell.dialogs().about_open());
    assert!(!shell.busy());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__channel_selection_and_refresh_are_explicit_worker_commands() {
    use tallyline_ui::api::model::{GameWitnessView, SettingsCommand, UserAction};
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(
        tallyline_workflows::api::model::AppSettings::default(),
    ));
    assert_eq!(
        shell.act(UserAction::ChooseGameChannel(
            "synthetic-channel".to_owned()
        )),
        Some(SettingsCommand::ChangeGameChannel(
            "synthetic-channel".to_owned()
        ))
    );
    shell.receive(BackgroundEvent::GameWitnessLoaded(
        GameWitnessView::default(),
    ));
    assert_eq!(
        shell.act(UserAction::RefreshWitness),
        Some(SettingsCommand::RefreshWitness)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__channel_selection_commands_do_not_disclose_the_local_path_in_debug_output() {
    let command = tallyline_ui::api::model::SettingsCommand::ChangeGameChannel(
        "SYNTHETIC_PRIVATE_PATH".to_owned(),
    );
    assert!(!format!("{command:?}").contains("SYNTHETIC_"));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5_r_ui_16__directory_permission_warning_survives_settings_load_and_save_feedback() {
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::StorageWarning(
        "A data directory is accessible to another user".to_owned(),
    ));
    shell.receive(BackgroundEvent::SettingsLoaded(
        tallyline_workflows::api::model::AppSettings::default(),
    ));
    assert_eq!(
        shell.notice(),
        Some("A data directory is accessible to another user")
    );
    shell.show_settings();
    shell.receive(BackgroundEvent::Failure("Settings save failed".to_owned()));
    assert_eq!(shell.notice(), Some("Settings save failed"));
    shell.receive(BackgroundEvent::SettingsLoaded(
        tallyline_workflows::api::model::AppSettings::default(),
    ));
    assert_eq!(
        shell.notice(),
        Some("A data directory is accessible to another user")
    );
    assert!(!shell.dialogs().about_open());
}
