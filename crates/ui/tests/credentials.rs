// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Critical masked credential setup and passive failure flows.

use egui_kittest::{Harness, kittest::Queryable};
use tallyline_submission::api::model::{
    AccountState, CredentialFailure, Persistence, StorageFailure,
};
use tallyline_ui::api::model::{BackgroundEvent, CredentialCommand};
use tallyline_ui::api::{Catalogue, ShellState, show_shell};
mod control_paint;
mod input_frame_paint;
mod resources;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_4_r_nf_4_r_ui_16__masked_setup_keeps_failed_save_passive_and_allows_session_choice() {
    let catalogue = Catalogue::english().unwrap();
    let mut shell = ShellState::default();
    shell.receive(BackgroundEvent::CredentialsUpdated {
        account: AccountState::Unconfigured,
        failure: None,
        available_token: false,
        fallback_available: false,
    });
    shell.credentials_mut().set_input(
        "dummy-key-mask-probe".to_owned(),
        Some("dummy-token-mask-probe".to_owned()),
        Persistence::Session,
    );
    shell.show_settings();
    let mut initial = true;
    let mut unit = None;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 1600.0))
        .build_ui_state(
            move |ui, shell: &mut ShellState| {
                if initial {
                    tallyline_ui::api::configure_fonts(ui.ctx(), resources::font_files().unwrap());
                    unit = Some(
                        resources::load_unit(ui.ctx(), &catalogue, tallyline_egui::Theme::Light)
                            .unwrap(),
                    );
                    initial = false;
                }
                show_shell(ui, shell, &catalogue, unit.as_ref().unwrap());
            },
            shell,
        );
    harness.run();
    visibility::activate_visible(&mut harness, "UEX account").unwrap();
    let initial_output = format!(
        "{:?} {:?}",
        harness.output().shapes,
        harness.output().platform_output.accesskit_update
    );
    assert!(!initial_output.contains("dummy-key-mask-probe"));
    assert!(!initial_output.contains("dummy-token-mask-probe"));
    visibility::activate_visible(&mut harness, "Validate credentials").unwrap();
    assert!(matches!(
        harness.state_mut().credentials_mut().take_command(),
        Some(CredentialCommand::Validate(_))
    ));
    assert!(harness.state().credentials().busy());
    harness
        .state_mut()
        .receive(BackgroundEvent::CredentialsUpdated {
            account: AccountState::Unconfigured,
            failure: Some(CredentialFailure::Storage(StorageFailure::Locked)),
            available_token: false,
            fallback_available: false,
        });
    harness.run();
    harness.get_by_label("The credential store is locked or access was declined. Unlock it yourself or choose session-only.");
    assert!(!harness.state().dialogs().about_open());
    let output = format!("{:?}", harness.output().shapes);
    assert!(!output.contains("dummy-key-mask-probe"));
    assert!(!output.contains("dummy-token-mask-probe"));
    assert!(
        harness
            .output()
            .viewport_output
            .values()
            .all(|output| output.commands.is_empty())
    );
}

#[path = "credentials/feedback.rs"]
mod feedback;
#[path = "credentials/fields.rs"]
mod fields;
#[path = "credentials/reach.rs"]
mod reach;
#[path = "credentials/support.rs"]
mod support;

#[path = "credentials/visibility.rs"]
mod visibility;
