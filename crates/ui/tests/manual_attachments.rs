// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Critical drawn private-preview and current-Report contracts without a native window.

use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tallyline_game::api::model::GameEnvironment;
use tallyline_reporting::api::AttachmentEdit;
use tallyline_reporting::api::model::{
    AttachmentBinding, AttachmentRegion, AttachmentTime, ImageHash, Report, ReportId, ReportKey,
};
use tallyline_shared_kernel::api::model::{ImageRaster, TerminalId, Timestamp, TradeSide};
use tallyline_submission::api::model::{AccountState, Persistence, ScreenshotPolicy};
use tallyline_ui::api::model::{BackgroundEvent, ManualAttachmentAction, ManualCommand};
use tallyline_ui::api::{Catalogue, ShellState, show_manual_attachment_editor};
use tallyline_workflows::api::{ManualAttachmentBlock, ManualAttachmentPreview};

fn selected() -> Result<(ShellState, Report, ManualAttachmentPreview), String> {
    let mut report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live)
            .map_err(|error| format!("{error:?}"))?,
        Timestamp::from_seconds(100),
        Some("4.10.2"),
    )
    .map_err(|error| format!("{error:?}"))?;
    let binding = AttachmentBinding::new(
        ImageHash::new([1; 32]),
        AttachmentRegion::new(0, 10, 4, 2).map_err(|error| format!("{error:?}"))?,
    );
    report
        .edit_attachment(AttachmentEdit::Select(
            binding,
            AttachmentTime::ImportedFile(Timestamp::from_seconds(90)),
        ))
        .map_err(|error| format!("{error:?}"))?;
    let preview = ManualAttachmentPreview::new(
        binding,
        AttachmentTime::ImportedFile(Timestamp::from_seconds(90)),
        ImageRaster::new(4, 2, vec![0xff00_c800; 8]).map_err(|error| format!("{error:?}"))?,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut state = ShellState::default();
    state.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    Ok((state, report, preview))
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7__changed_content_geometry_and_removal_reject_delayed_preview_results() {
    for action in [
        AttachmentEdit::Select(
            AttachmentBinding::new(
                ImageHash::new([2; 32]),
                AttachmentRegion::new(0, 10, 4, 2).unwrap(),
            ),
            AttachmentTime::ImportedFile(Timestamp::from_seconds(90)),
        ),
        AttachmentEdit::Select(
            AttachmentBinding::new(
                ImageHash::new([1; 32]),
                AttachmentRegion::new(0, 11, 4, 2).unwrap(),
            ),
            AttachmentTime::ImportedFile(Timestamp::from_seconds(90)),
        ),
        AttachmentEdit::Remove,
    ] {
        let (mut state, mut report, preview) = selected().unwrap();
        state.receive(BackgroundEvent::AttachmentPreview {
            id: report.id(),
            expected: report.version(),
            preview: preview.clone(),
        });
        assert!(state.manual().attachment_preview().is_some());
        let expected = report.version();
        report.edit_attachment(action).unwrap();
        state.receive(BackgroundEvent::ReportsLoaded {
            reports: vec![report.clone()],
            selected: Some(report.id()),
        });
        state.receive(BackgroundEvent::AttachmentPreview {
            id: report.id(),
            expected,
            preview,
        });
        assert!(state.manual().attachment_preview().is_none());
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7__dated_requirement_and_full_size_preview_have_explicit_clear_and_privacy_actions()
 {
    let (mut state, report, preview) = selected().unwrap();
    let account = AccountState::Validated {
        username: "dummy-account".to_owned(),
        validated_at: Timestamp::from_seconds(100),
        persistence: Persistence::Session,
    };
    state.receive(BackgroundEvent::AttachmentPreview {
        id: report.id(),
        expected: report.version(),
        preview,
    });
    state.receive(BackgroundEvent::AttachmentStatus {
        id: report.id(),
        expected: report.version(),
        account: account.clone(),
        policy: Ok(ScreenshotPolicy::Required {
            observed_at: Timestamp::from_seconds(0),
        }),
        readiness: Err(ManualAttachmentBlock::PreviewUnconfirmed),
    });
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 1000.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_manual_attachment_editor(ui, state, &catalogue, &account);
            },
            state,
        );
    harness.run();
    assert!(
        harness
            .query_by_label("Screenshot required since 1970-01-01 00:00:00 UTC")
            .is_some()
    );
    harness.get_by_label("Try without screenshot").click();
    harness.run();
    assert!(
        matches!(harness.state_mut().manual_mut().take_command(), Some(ManualCommand::Attachment(command)) if matches!(&command.action, ManualAttachmentAction::TryWithoutScreenshot(AccountState::Validated { username, .. }) if username == "dummy-account"))
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::AttachmentStatus {
            id: report.id(),
            expected: report.version(),
            account: AccountState::Validated {
                username: "dummy-account".to_owned(),
                validated_at: Timestamp::from_seconds(100),
                persistence: Persistence::Session,
            },
            policy: Ok(ScreenshotPolicy::Unknown),
            readiness: Err(ManualAttachmentBlock::PreviewUnconfirmed),
        });
    harness.run();
    assert!(
        harness
            .query_by_label("Screenshot required since 1970-01-01 00:00:00 UTC")
            .is_none()
    );
    harness
        .get_by_label("Contains no balance or personal data")
        .click();
    harness.run();
    assert!(
        matches!(harness.state_mut().manual_mut().take_command(), Some(ManualCommand::Attachment(command)) if matches!(command.action, ManualAttachmentAction::ConfirmPreview(_)))
    );
}

fn font_files() -> Result<tallyline_ui::api::model::FontFiles, &'static str> {
    Ok(tallyline_ui::api::model::FontFiles {
        material_symbols: include_bytes!(
            "../assets/fonts/material-symbols/MaterialSymbolsRounded-Tallyline.ttf"
        )
        .to_vec()
        .try_into()?,
        plex_regular: include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf").to_vec(),
        plex_semibold: include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-SemiBold.ttf").to_vec(),
        plex_bold: include_bytes!("../assets/fonts/ibm-plex/IBMPlexSans-Bold.ttf").to_vec(),
        mono_regular: include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMonoNL-Regular.ttf")
            .to_vec(),
        mono_bold: include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMonoNL-Bold.ttf")
            .to_vec(),
    })
}

struct Empty;
impl tallyline_reference_data::api::ReferenceDataCache for Empty {
    type Error = std::convert::Infallible;
    fn load(
        &mut self,
    ) -> Result<Vec<tallyline_reference_data::api::model::CachedReference>, Self::Error> {
        Ok(vec![])
    }
    fn replace(
        &mut self,
        _: &tallyline_reference_data::api::model::CachedReference,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
struct Fixed;
impl tallyline_shared_kernel::api::Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(100)
    }
}
fn references(state: &mut ShellState) -> Result<(), std::convert::Infallible> {
    use tallyline_shared_kernel::api::model::UexEnvironment;
    let reference = tallyline_reference_data::api::ReferenceApplication::load(Empty, Fixed)?;
    state.receive(BackgroundEvent::ReferenceUpdated {
        live: reference.snapshot(UexEnvironment::Live),
        ptu: reference.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(100),
    });
    Ok(())
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_q_manual_r_ui_16__passive_other_draft_commit_preserves_unfinished_price_input() {
    let (mut state, mut first, _) = selected().unwrap();
    let mut second = Report::manual(
        ReportId(2),
        first.key(),
        Timestamp::from_seconds(100),
        Some("4.10.2"),
    )
    .unwrap();
    second
        .add_commodity(tallyline_shared_kernel::api::model::CommodityId(42))
        .unwrap();
    state.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![first.clone(), second.clone()],
        selected: Some(second.id()),
    });
    references(&mut state).unwrap();
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1400.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                tallyline_ui::api::show_manual_editor(ui, state, &catalogue);
            },
            state,
        );
    harness.run();
    harness
        .get_by_role_and_label(
            egui::Role::TextInput,
            "Price per SCU for commodity 42 (aUEC/SCU)",
        )
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(
            egui::Role::TextInput,
            "Price per SCU for commodity 42 (aUEC/SCU)",
        )
        .type_text("123.45");
    harness.run();
    first
        .edit_attachment(AttachmentEdit::ConfirmTime(Timestamp::from_seconds(90)))
        .unwrap();
    harness.state_mut().receive(BackgroundEvent::ReportsLoaded {
        reports: vec![first, second],
        selected: None,
    });
    harness.run();
    assert_eq!(
        harness.state().manual().selected().unwrap().id(),
        ReportId(2)
    );
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(
        matches!(harness.state_mut().manual_mut().take_command(), Some(ManualCommand::Edit { edits, .. }) if matches!(edits.as_slice(), [tallyline_reporting::api::ManualEdit::Price(_, Some(value))] if value.to_string() == "123.45"))
    );
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_q_manual_r_ui_16__older_draft_result_cannot_clear_a_newer_attachment_request() {
    let (mut state, mut first, preview) = selected().unwrap();
    let second = Report::manual(
        ReportId(2),
        first.key(),
        Timestamp::from_seconds(100),
        Some("4.10.2"),
    )
    .unwrap();
    state.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![first.clone(), second.clone()],
        selected: Some(first.id()),
    });
    state.receive(BackgroundEvent::AttachmentPreview {
        id: first.id(),
        expected: first.version(),
        preview,
    });
    references(&mut state).unwrap();
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1400.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                tallyline_ui::api::show_manual_editor(ui, state, &catalogue);
            },
            state,
        );
    harness.run();
    harness
        .get_by_label("Contains no balance or personal data")
        .click();
    harness.run();
    assert!(matches!(
        harness.state_mut().manual_mut().take_command(),
        Some(ManualCommand::Attachment(_))
    ));
    harness.get_by_label("Resume Draft 2 · 11 · Live").click();
    harness.run();
    harness.get_by_label("Check attachment readiness").click();
    harness.run();
    assert!(matches!(
        harness.state_mut().manual_mut().take_command(),
        Some(ManualCommand::Attachment(_))
    ));
    first
        .edit_attachment(AttachmentEdit::ConfirmTime(Timestamp::from_seconds(90)))
        .unwrap();
    harness.state_mut().receive(BackgroundEvent::ReportsLoaded {
        reports: vec![first, second.clone()],
        selected: None,
    });
    harness.run();
    assert!(
        harness
            .get_by_label("Check attachment readiness")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::AttachmentStatus {
            id: second.id(),
            expected: second.version(),
            account: AccountState::Unconfigured,
            policy: Ok(ScreenshotPolicy::Unknown),
            readiness: Ok(()),
        });
    harness.run();
    assert!(
        !harness
            .get_by_label("Check attachment readiness")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_q_manual__recovered_same_version_preview_retires_only_its_own_request() {
    let (mut state, mut report, preview) = selected().unwrap();
    report
        .edit_attachment(AttachmentEdit::ConfirmPreview(preview.binding()))
        .unwrap();
    report
        .edit_attachment(AttachmentEdit::BindWorkingCopy(
            preview.binding(),
            ImageHash::new([8; 32]),
        ))
        .unwrap();
    report
        .edit_attachment(AttachmentEdit::ConfirmTime(Timestamp::from_seconds(90)))
        .unwrap();
    state.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1200.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                show_manual_attachment_editor(ui, state, &catalogue, &AccountState::Unconfigured);
            },
            state,
        );
    harness.run();
    harness.get_by_label("View saved private region").click();
    harness.run();
    assert!(
        matches!(harness.state_mut().manual_mut().take_command(), Some(ManualCommand::Attachment(command)) if matches!(command.action, ManualAttachmentAction::View))
    );
    assert!(
        harness
            .get_by_label("Check attachment readiness")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::AttachmentPreview {
            id: report.id(),
            expected: report.version(),
            preview,
        });
    harness.run();
    assert!(
        !harness
            .get_by_label("Check attachment readiness")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_7_q_manual__manual_view_uses_only_the_current_accepted_account_policy_offline() {
    let report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(100),
        Some("4.10.2"),
    )
    .unwrap();
    let first = AccountState::Validated {
        username: "dummy-a".to_owned(),
        validated_at: Timestamp::from_seconds(100),
        persistence: Persistence::Session,
    };
    let second = AccountState::Validated {
        username: "dummy-b".to_owned(),
        validated_at: Timestamp::from_seconds(110),
        persistence: Persistence::Session,
    };
    let mut state = ShellState::default();
    state.receive(BackgroundEvent::ReportsLoaded {
        reports: vec![report.clone()],
        selected: Some(report.id()),
    });
    state.receive(BackgroundEvent::CredentialsUpdated {
        account: first.clone(),
        failure: None,
        available_token: true,
        fallback_available: false,
    });
    state.receive(BackgroundEvent::AttachmentStatus {
        id: report.id(),
        expected: report.version(),
        account: first.clone(),
        policy: Ok(ScreenshotPolicy::Required {
            observed_at: Timestamp::from_seconds(0),
        }),
        readiness: Err(ManualAttachmentBlock::ImageRequired),
    });
    let catalogue = Catalogue::english().unwrap();
    let mut fonts = Some(font_files().unwrap());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1200.0))
        .build_ui_state(
            move |ui, state| {
                if let Some(fonts) = fonts.take() {
                    tallyline_ui::api::configure_fonts(ui.ctx(), fonts);
                }
                tallyline_ui::api::show_manual_editor(ui, state, &catalogue);
            },
            state,
        );
    harness.run();
    assert!(
        harness
            .query_by_label("Screenshot required since 1970-01-01 00:00:00 UTC")
            .is_some()
    );
    assert!(
        harness
            .query_by_label(
                "This account requires a screenshot. Select and confirm a private region."
            )
            .is_some()
    );
    harness
        .state_mut()
        .receive(BackgroundEvent::CredentialsUpdated {
            account: second,
            failure: None,
            available_token: true,
            fallback_available: false,
        });
    harness.run();
    assert!(
        harness
            .query_by_label("Screenshot required since 1970-01-01 00:00:00 UTC")
            .is_none()
    );
    assert!(
        matches!(harness.state_mut().manual_mut().take_command(), Some(ManualCommand::Attachment(command)) if matches!(&command.action, ManualAttachmentAction::RefreshPolicy(AccountState::Validated { username, .. }) if username == "dummy-b"))
    );
}
