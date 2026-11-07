// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Critical shell flows through the drawn UI.

use egui_kittest::{Harness, kittest::Queryable};
use tallyline_ui::api::model::{BackgroundEvent, SettingsCommand};
use tallyline_ui::api::{Catalogue, FanKitUnit, ShellState, configure_fonts, show_shell};
use tallyline_workflows::api::model::AppSettings;

mod control_paint;
mod resources;

type Outcome = Result<(), String>;

struct Fixture {
    shell: ShellState,
    catalogue: Catalogue,
    unit: Option<FanKitUnit>,
    commands: Vec<SettingsCommand>,
    setup_error: Option<String>,
}

fn harness() -> Result<Harness<'static, Fixture>, String> {
    let catalogue = Catalogue::english().map_err(|error| error.to_string())?;
    let mut fixture = Fixture {
        shell: ShellState::default(),
        catalogue,
        unit: None,
        commands: Vec::new(),
        setup_error: None,
    };
    fixture
        .shell
        .receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    let mut fonts = Some(resources::font_files()?);
    Ok(Harness::builder()
        .with_size(egui::vec2(1000.0, 900.0))
        .build_ui_state(
            move |ui, fixture: &mut Fixture| {
                if fixture.unit.is_none() {
                    if let Some(fonts) = fonts.take() {
                        configure_fonts(ui.ctx(), fonts);
                    }
                    match resources::load_unit(
                        ui.ctx(),
                        &fixture.catalogue,
                        tallyline_egui::Theme::Light,
                    ) {
                        Ok(unit) => fixture.unit = Some(unit),
                        Err(error) => {
                            fixture.setup_error = Some(error.to_string());
                            return;
                        }
                    }
                }
                if let Some(unit) = &fixture.unit {
                    fixture.commands.extend(show_shell(
                        ui,
                        &mut fixture.shell,
                        &fixture.catalogue,
                        unit,
                    ));
                }
            },
            fixture,
        ))
}

fn assert_focused_control_visible(harness: &Harness<'_, Fixture>, label: &str) -> Outcome {
    crate::control_paint::assert_focused_visible(harness, &harness.get_by_label(label), label)
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_19_q_app__onboarding_issues_a_save_command_and_waits_for_persistence() -> Outcome {
    let mut harness = harness()?;
    harness.run();
    assert_eq!(harness.state().setup_error, None);
    harness.get_by_label("Continue").click();
    harness.run();
    assert_eq!(
        harness.state().commands,
        vec![SettingsCommand::CompleteOnboarding]
    );
    assert!(harness.state().shell.busy());
    assert!(!harness.state().shell.settings().onboarding_complete());
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_16__background_failure_while_unfocused_and_minimised_creates_no_viewport_commands()
-> Outcome {
    let mut harness = harness()?;
    harness.run();
    harness.input_mut().focused = false;
    if let Some(viewport) = harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
    {
        viewport.minimized = Some(true);
        viewport.focused = Some(false);
    }
    harness.state_mut().shell.receive(BackgroundEvent::Failure(
        "Background storage failure".to_owned(),
    ));
    harness.step();
    assert_eq!(
        harness.state().shell.notice(),
        Some("Background storage failure")
    );
    assert!(!harness.state().shell.dialogs().about_open());
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
    assert_eq!(harness.output().viewport_output.len(), 1);
    assert!(!harness.input().focused);
    assert_eq!(
        harness
            .input()
            .viewports
            .get(&egui::ViewportId::ROOT)
            .and_then(|viewport| viewport.minimized),
        Some(true)
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_3__idle_shell_requests_no_continuous_repaints() -> Outcome {
    let mut harness = harness()?;
    harness.run();
    assert_eq!(harness.run(), 1);
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|viewport| viewport.repaint_delay > std::time::Duration::ZERO)
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_8_r_ui_19_q_app__done_leaves_settings_and_preserves_unfinished_onboarding() -> Outcome {
    use tallyline_workflows::api::model::Theme;
    for complete in [false, true] {
        let mut harness = harness()?;
        harness
            .state_mut()
            .shell
            .receive(BackgroundEvent::SettingsLoaded(
                AppSettings::from_persisted(Theme::System, complete),
            ));
        harness.state_mut().shell.show_settings();
        harness.run();
        harness.get_by_label("Done").focus();
        harness.run();
        assert_focused_control_visible(&harness, "Done")?;
        harness.key_press(egui::Key::Space);
        harness.run();
        let destination = if complete {
            "Workspace"
        } else {
            "Welcome to Tallyline"
        };
        assert!(harness.query_by_label(destination).is_some());
        assert_eq!(
            harness.state().shell.settings().onboarding_complete(),
            complete
        );
    }
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_18__about_identifies_the_fixed_development_version_and_platform() -> Outcome {
    let mut harness = harness()?;
    harness.state_mut().shell.set_build_presentation(
        tallyline_ui::api::model::BuildPresentation::Development {
            version: "0.0.0".to_owned(),
            platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        },
    );
    harness.run();
    harness.get_by_label("Help").click();
    harness.run();
    harness.get_by_label("About").click();
    harness.run();
    let expected = format!(
        "Development build {} · {}/{}",
        "0.0.0",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    assert!(
        harness.query_by_label(&expected).is_some(),
        "About must identify {expected}"
    );
    assert_opened_link(
        &mut harness,
        "Source code",
        "https://github.com/tallyline-sc/tallyline",
    );
    assert_opened_link(
        &mut harness,
        "Release and source archives",
        "https://github.com/tallyline-sc/tallyline/releases",
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__settings_shows_default_off_and_emits_only_explicit_witness_commands() -> Outcome {
    use tallyline_game::api::model::{
        GameSettings, WitnessMode, WitnessSnapshot, WitnessUnavailable,
    };
    use tallyline_ui::api::model::GameWitnessView;
    let mut harness = harness()?;
    harness.state_mut().shell.show_settings();
    harness.run();
    assert!(harness.query_by_label("Game log witness off").is_some());
    assert!(
        harness
            .query_by_role_and_label(egui::Role::TextInput, "Game channel directory")
            .is_some()
    );
    harness
        .get_by_label("Enable read-only Game.log witness")
        .focus();
    harness.run();
    assert_focused_control_visible(&harness, "Enable read-only Game.log witness")?;
    harness.key_press(egui::Key::Space);
    harness.run();
    assert_eq!(
        harness.state().commands,
        vec![SettingsCommand::ChangeWitnessMode(WitnessMode::On)]
    );
    assert_eq!(
        harness.state().shell.game_witness().settings.witness,
        WitnessMode::Off
    );
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::GameWitnessLoaded(GameWitnessView {
            settings: GameSettings {
                witness: WitnessMode::On,
                ..GameSettings::default()
            },
            channel_directory: None,
            snapshot: WitnessSnapshot::Unavailable(WitnessUnavailable::UnvalidatedBuild),
        }));
    harness.run();
    assert!(
        harness
            .query_by_label(
                "Game log witness unavailable: this exact build and branch have not been validated."
            )
            .is_some()
    );
    assert!(!harness.state().shell.dialogs().about_open());
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7_r_ui_16__release_notice_is_passive_actionable_and_dismissible() -> Outcome {
    use tallyline_workflows::api::model::{ReleaseVersion, UpdateStatus};
    let mut harness = harness()?;
    harness.run();
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::SettingsLoaded(
            AppSettings::default()
                .with_update_state(tallyline_workflows::api::model::UpdateChecks::Enabled, None),
        ));
    harness.input_mut().focused = false;
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::UpdateChecked(UpdateStatus::Available(
            ReleaseVersion::parse("v2.0.1").unwrap(),
        )));
    harness.step();
    assert!(
        harness
            .query_by_label("A new Tallyline release is available:")
            .is_some()
    );
    assert!(!harness.state().shell.dialogs().about_open());
    assert!(!harness.input().focused);
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
    assert_eq!(
        harness.state().shell.view(),
        tallyline_ui::api::model::View::Onboarding
    );
    harness.get_by_label("Open release notes").click();
    harness.step();
    assert_eq!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::OpenUrl(url) => Some(url.url.as_str()),
                egui::OutputCommand::CopyText(_)
                | egui::OutputCommand::CopyImage(_)
                | egui::OutputCommand::TextSelectionSettled(_) => None,
            }),
        Some("https://github.com/tallyline-sc/tallyline/releases/tag/v2.0.1")
    );
    harness.get_by_label("Dismiss update notice").click();
    harness.run();
    assert!(
        harness
            .state()
            .commands
            .contains(&SettingsCommand::DismissUpdate)
    );
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::UpdateChecked(UpdateStatus::Dismissed));
    harness.run();
    assert!(
        harness
            .query_by_label("A new Tallyline release is available:")
            .is_none()
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_7_r_ui_16__daily_setting_waits_for_save_and_disabled_results_remain_hidden() -> Outcome {
    use tallyline_workflows::api::model::{ReleaseVersion, UpdateChecks, UpdateStatus};
    let mut harness = harness()?;
    harness.state_mut().shell.show_settings();
    harness.run();
    harness
        .get_by_label("Check for release updates daily")
        .focus();
    harness.run();
    assert_focused_control_visible(&harness, "Check for release updates daily")?;
    harness.key_press(egui::Key::Space);
    harness.run();
    assert_eq!(
        harness.state().commands,
        vec![SettingsCommand::ChangeUpdateChecks(UpdateChecks::Enabled)]
    );
    assert_eq!(
        harness.state().shell.settings().update_checks(),
        UpdateChecks::Disabled
    );
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::SettingsLoaded(
            AppSettings::default().with_update_state(UpdateChecks::Enabled, None),
        ));
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::UpdateChecked(UpdateStatus::Unavailable));
    harness.run();
    assert!(harness.query_by_label("Release updates are unavailable. Work can continue. Checks will resume after the daily interval; Settings can turn them off.").is_some());
    assert_eq!(
        harness.state().shell.view(),
        tallyline_ui::api::model::View::Settings
    );
    assert!(!harness.state().shell.dialogs().about_open());
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::UpdateChecked(UpdateStatus::Available(
            ReleaseVersion::parse("v2.0.1").unwrap(),
        )));
    harness.run();
    assert!(
        harness
            .query_by_label("A new Tallyline release is available:")
            .is_none()
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_5_r_val_2_q_manual__drawn_stock_settings_reject_loose_input_and_save_explicit_valid_choice()
-> Outcome {
    use egui_kittest::kittest::NodeT;
    let mut harness = harness()?;
    harness.state_mut().shell.show_settings();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::TextInput, "SCU percentage (0 to 20) (%)")
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(egui::Role::TextInput, "SCU percentage (0 to 20) (%)")
        .type_text("5000");
    harness.run();
    assert!(harness.state().commands.is_empty());
    harness.get_by_label("Save SCU tolerance").click();
    harness.run();
    assert!(harness.query_by_label("Enter a whole percentage from 0 to 20 and an exact SCU minimum from 0 to 16. Looser or invalid values are not applied.").is_some());
    assert!(harness.state().commands.is_empty());
    assert!(!harness.state().shell.busy());
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    harness.run();
    assert!(harness.state().commands.is_empty());
    harness.get_by_label("Save SCU tolerance").click();
    harness.run();
    assert_eq!(
        harness.state().commands,
        vec![SettingsCommand::ChangeStockSettings(Default::default())]
    );
    assert!(harness.state().shell.busy());
    harness.run();
    for label in [
        "SCU percentage (0 to 20) (%)",
        "Absolute SCU minimum (0 to 16) (SCU)",
    ] {
        assert!(
            harness
                .get_by_role_and_label(egui::Role::TextInput, label)
                .accesskit_node()
                .is_disabled()
        );
    }
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]
fn r_ui_18_r_nf_11__about_uses_the_injected_release_identity_instead_of_cargo_metadata() -> Outcome
{
    let mut harness = harness()?;
    harness.state_mut().shell.set_build_presentation(
        tallyline_ui::api::model::BuildPresentation::Release {
            version: "9.8.7".to_owned(),
            platform: "windows/x86_64".to_owned(),
            tag: "v9.8.7".to_owned(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        },
    );
    harness.run();
    harness.get_by_label("Help").click();
    harness.run();
    harness.get_by_label("About").click();
    harness.run();
    assert!(
        harness
            .query_by_label("Version 9.8.7 · windows/x86_64")
            .is_some()
    );
    assert!(
        harness
            .query_by_label("Development build 0.1.0 · windows/x86_64")
            .is_none()
    );
    for (label, expected) in [
        (
            "Source code",
            "https://github.com/tallyline-sc/tallyline/tree/0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "Release and source archives",
            "https://github.com/tallyline-sc/tallyline/releases/tag/v9.8.7",
        ),
    ] {
        assert_opened_link(&mut harness, label, expected);
    }
    Ok(())
}

fn assert_opened_link(harness: &mut Harness<'_, Fixture>, label: &str, expected: &str) {
    harness.get_by_label(label).click_accesskit();
    harness.step();
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|command| {
                matches!(command, egui::OutputCommand::OpenUrl(url) if url.url == expected)
            }),
        "expected {expected}, received {:?}",
        harness.output().platform_output.commands
    );
}
