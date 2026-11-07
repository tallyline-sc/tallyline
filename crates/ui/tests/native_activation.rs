// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opt-in Windows host verification of R-NF-10 without desktop input injection.

#![forbid(unsafe_code)]

#[path = "resources/common.rs"]
mod resources;

use std::cell::Cell;
use std::io::{self, Write};
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::{Duration, Instant};
use tallyline_ui::api::model::{BackgroundEvent, WindowCommand};
use tallyline_ui::api::{Catalogue, DesktopShell, configure_fonts};
use tallyline_workflows::api::model::{AppSettings, Theme};

const DEADLINE: Duration = Duration::from_secs(5);
const TICK: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Restored,
    NoNativeWindow,
    NoInitialPaint,
    NotMinimised,
    NotRestored,
    ActivationUnavailable,
}

#[derive(Clone, Copy)]
enum Stage {
    AwaitPaint(Instant),
    AwaitMinimise(Instant),
    AwaitRestore(Instant),
    Finished,
}

struct NativeProbe {
    shell: DesktopShell,
    activation: SyncSender<WindowCommand>,
    outcome: Rc<Cell<Option<Outcome>>>,
    stage: Stage,
    painted: bool,
}

impl NativeProbe {
    fn finish(&mut self, context: &egui::Context, outcome: Outcome) {
        self.outcome.set(Some(outcome));
        self.stage = Stage::Finished;
        context.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn tick(&mut self, context: &egui::Context, frame: &eframe::Frame) {
        let Some(window) = frame.winit_window() else {
            self.finish(context, Outcome::NoNativeWindow);
            return;
        };
        let now = Instant::now();
        match self.stage {
            Stage::AwaitPaint(deadline) => {
                if self.painted {
                    window.set_minimized(true);
                    self.stage = Stage::AwaitMinimise(now + DEADLINE);
                } else if now >= deadline {
                    self.finish(context, Outcome::NoInitialPaint);
                }
            }
            Stage::AwaitMinimise(deadline) => {
                if window.is_minimized() == Some(true) {
                    if self.activation.try_send(WindowCommand::Activate).is_err() {
                        self.finish(context, Outcome::ActivationUnavailable);
                    } else {
                        self.stage = Stage::AwaitRestore(now + DEADLINE);
                    }
                } else if now >= deadline {
                    self.finish(context, Outcome::NotMinimised);
                }
            }
            Stage::AwaitRestore(deadline) => {
                if window.is_minimized() == Some(false) {
                    self.finish(context, Outcome::Restored);
                } else if now >= deadline {
                    self.finish(context, Outcome::NotRestored);
                }
            }
            Stage::Finished => context.send_viewport_cmd(egui::ViewportCommand::Close),
        }
        if !matches!(self.stage, Stage::Finished) {
            context.request_repaint_after(TICK);
        }
    }
}

impl eframe::App for NativeProbe {
    fn logic(&mut self, context: &egui::Context, frame: &mut eframe::Frame) {
        self.shell.logic(context, frame);
        self.tick(context, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.shell.ui(ui, frame);
        self.painted = true;
        ui.ctx().request_repaint_after(TICK);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(target_os = "windows")
        || std::env::var_os("TALLYLINE_NATIVE_HOST_CHECK").as_deref()
            != Some(std::ffi::OsStr::new("1"))
    {
        writeln!(
            io::stdout().lock(),
            "SKIP R-NF-10 native Windows host: set TALLYLINE_NATIVE_HOST_CHECK=1 on an interactive Windows desktop"
        )?;
        return Ok(());
    }
    let outcome = Rc::new(Cell::new(None));
    let observed = Rc::clone(&outcome);
    eframe::run_native(
        "Tallyline activation contract",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 900.0]),
            ..Default::default()
        },
        Box::new(move |creation| {
            configure_fonts(
                &creation.egui_ctx,
                resources::font_files().map_err(io::Error::other)?,
            );
            let light = resources::decode_logo(include_bytes!(
                "../../../docs/images/fankit/made-by-the-community.png"
            ))?;
            let dark = resources::decode_logo(include_bytes!(
                "../../../docs/images/fankit/MadeByTheCommunity_White.png"
            ))?;
            let (commands, _commands) = sync_channel(1);
            let (background, events) = sync_channel(1);
            background.try_send(BackgroundEvent::SettingsLoaded(
                AppSettings::from_persisted(Theme::Dark, true),
            ))?;
            let (activation, activation_events) = sync_channel(1);
            let shell = DesktopShell::new(
                &creation.egui_ctx,
                Catalogue::english()?,
                light,
                dark,
                commands,
                events,
            )
            .with_window_commands(activation_events);
            Ok(Box::new(NativeProbe {
                shell,
                activation,
                outcome,
                stage: Stage::AwaitPaint(Instant::now() + DEADLINE),
                painted: false,
            }))
        }),
    )?;
    match observed.get() {
        Some(Outcome::Restored) => {
            writeln!(
                io::stdout().lock(),
                "PASS R-NF-10 native Windows host: minimised window restored through App::logic"
            )?;
            Ok(())
        }
        outcome => {
            Err(io::Error::other(format!("R-NF-10 native restoration failed: {outcome:?}")).into())
        }
    }
}
