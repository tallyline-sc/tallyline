// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Activation reaches the native host's hidden-window callback without a paint pass.

use std::sync::mpsc::{SyncSender, sync_channel};
use tallyline_ui::api::model::{BackgroundEvent, WindowCommand};
use tallyline_ui::api::{Catalogue, DesktopShell};

struct Fixture {
    context: egui::Context,
    shell: DesktopShell,
    activation: SyncSender<WindowCommand>,
    background: SyncSender<BackgroundEvent>,
}

fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let context = egui::Context::default();
    let (commands, _commands) = sync_channel(1);
    let (background, events) = sync_channel(1);
    let (activation, activation_events) = sync_channel(1);
    let image = egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 255]);
    let shell = DesktopShell::new(
        &context,
        Catalogue::english()?,
        image.clone(),
        image,
        commands,
        events,
    )
    .with_window_commands(activation_events);
    Ok(Fixture {
        context,
        shell,
        activation,
        background,
    })
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__explicit_activation_restores_during_the_hidden_window_logic_callback()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = fixture()?;
    fixture.activation.send(WindowCommand::Activate)?;
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .minimized = Some(true);
    let mut frame = eframe::Frame::_new_kittest();
    let output = fixture.context.run_logic(&input, |context| {
        eframe::App::logic(&mut fixture.shell, context, &mut frame);
    });
    let commands = output
        .viewport_commands
        .get(&egui::ViewportId::ROOT)
        .ok_or("activation did not reach the hidden window")?;
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Minimized(false)))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Focus))
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational)
    )));
    let idle = fixture.context.run_logic(&input, |context| {
        eframe::App::logic(&mut fixture.shell, context, &mut frame);
    });
    assert!(
        idle.viewport_commands.values().all(Vec::is_empty),
        "activation must be consumed once"
    );
    Ok(())
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_16__a_background_failure_never_restores_the_hidden_window()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = fixture()?;
    fixture.background.send(BackgroundEvent::Failure(
        "background fixture failure".to_owned(),
    ))?;
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .minimized = Some(true);
    let mut frame = eframe::Frame::_new_kittest();
    let output = fixture.context.run_logic(&input, |context| {
        eframe::App::logic(&mut fixture.shell, context, &mut frame);
    });
    assert!(output.viewport_commands.values().all(Vec::is_empty));
    Ok(())
}
