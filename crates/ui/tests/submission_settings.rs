// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit mode and tighten-only age choices wait for durable settings acknowledgement.

use egui_kittest::{Harness, kittest::Queryable};
use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode};
use tallyline_ui::api::model::{BackgroundEvent, SettingsCommand};
use tallyline_ui::api::{Catalogue, ShellState, show_submission_settings};
use tallyline_workflows::api::model::AppSettings;

mod submission_support;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_8_r_sub_9_r_nf_5__drawn_mode_and_age_changes_wait_for_persistence_and_keep_the_hard_limit_fixed()
 {
    let catalogue = Catalogue::english().unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::SettingsLoaded(AppSettings::default()));
    let mut fonts = Some(submission_support::font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 900.0))
        .build_ui_state(
            move |ui, state: &mut (ShellState, Vec<SettingsCommand>)| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                state
                    .1
                    .extend(show_submission_settings(ui, &mut state.0, &catalogue));
            },
            (shell, Vec::new()),
        );
    harness.run();
    harness
        .get_by_label("Production for future releases")
        .click();
    harness.run();
    assert_eq!(
        harness.state().1,
        vec![SettingsCommand::ChangeSubmissionMode(Some(
            SubmissionMode::Production
        ))]
    );
    assert_eq!(harness.state().0.submission_mode(), SubmissionMode::DryRun);
    harness
        .state_mut()
        .0
        .receive(BackgroundEvent::SettingsLoaded(
            AppSettings::default().with_submission_mode(Some(SubmissionMode::Production)),
        ));
    harness.run();
    harness.get_by_label("15 minutes").click();
    harness.run();
    let age = ObservationAgeSettings::new(std::time::Duration::from_secs(900)).unwrap();
    assert_eq!(
        harness.state().1.last(),
        Some(&SettingsCommand::ChangeObservationAge(age))
    );
    assert_eq!(
        harness.state().0.settings().observation_age(),
        ObservationAgeSettings::default()
    );
    harness
        .state_mut()
        .0
        .receive(BackgroundEvent::SettingsLoaded(
            AppSettings::default()
                .with_submission_mode(Some(SubmissionMode::Production))
                .with_observation_age(age),
        ));
    assert_eq!(
        harness.state().0.submission_mode(),
        SubmissionMode::Production
    );
    assert_eq!(harness.state().0.settings().observation_age(), age);
    assert!(ObservationAgeSettings::new(std::time::Duration::from_secs(3601)).is_err());
}
