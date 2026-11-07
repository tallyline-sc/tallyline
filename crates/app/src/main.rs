// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The native Tallyline composition root.

#![forbid(unsafe_code)]

mod credential_wiring;
mod executor;
mod game_wiring;
mod http_wiring;
mod instance_wiring;
mod reference_wiring;
mod settings_wiring;
mod submission_wiring;
mod update_wiring;

use tallyline_ui::api::{Catalogue, DesktopShell, configure_fonts};

fn main() -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let mut output = std::io::stdout();
    tallyline_app::api::enter_application(
        std::env::args_os().skip(1),
        tallyline_app::api::compiled_build_information(),
        &mut output,
        |information| {
            use tallyline_adapter_platform::api::model::{OcrChildExit, OcrProcessStartup};
            match tallyline_adapter_platform::api::prepare_ocr_process()? {
                OcrProcessStartup::Prepared => {
                    tallyline_app::api::install_credential_log_bridge()?;
                    tracing::subscriber::set_global_default(
                        tallyline_app::api::DependencyDiagnostics::new(
                            tracing::subscriber::NoSubscriber::default(),
                        ),
                    )?;
                    tracing::info!(
                        version = information.version(),
                        build_kind = ?information.kind(),
                        target = information.target().triple(),
                        source_commit = ?information.source_commit(),
                        "application build information"
                    );
                    run(information)?;
                    Ok(std::process::ExitCode::SUCCESS)
                }
                OcrProcessStartup::Relaunched(status) => {
                    Ok(match OcrChildExit::from_status(&status) {
                        OcrChildExit::Code(code) => std::process::ExitCode::from(code),
                        OcrChildExit::Abnormal => std::process::ExitCode::FAILURE,
                    })
                }
            }
        },
    )
}

fn run(
    information: tallyline_app::api::model::BuildInformation,
) -> Result<(), Box<dyn std::error::Error>> {
    let build = match information.kind() {
        tallyline_app::api::model::BuildKind::Development => {
            tallyline_ui::api::model::BuildPresentation::Development {
                version: information.version().to_owned(),
                platform: information.target().platform().to_owned(),
            }
        }
        tallyline_app::api::model::BuildKind::Release => {
            tallyline_ui::api::model::BuildPresentation::Release {
                version: information.version().to_owned(),
                platform: information.target().platform().to_owned(),
                tag: information
                    .release_tag()
                    .ok_or("compiled release tag is missing")?
                    .to_owned(),
                commit: information
                    .source_commit()
                    .ok_or("compiled release commit is missing")?
                    .to_owned(),
            }
        }
    };
    let layout = settings_wiring::storage_layout()?;
    let primary = match tallyline_adapter_platform::api::elect(instance_wiring::paths(&layout)?)? {
        tallyline_adapter_platform::api::Election::Primary(primary) => primary,
        tallyline_adapter_platform::api::Election::Secondary(
            tallyline_adapter_platform::api::ActivationOutcome::Activated,
        ) => return Ok(()),
        tallyline_adapter_platform::api::Election::Secondary(
            tallyline_adapter_platform::api::ActivationOutcome::Unavailable,
        ) => {
            return already_running();
        }
    };
    let (_writer_lease, listener) = primary.into_parts();
    let (files, light_logo, dark_logo, icon) =
        executor::spawn("tallyline-resources", || -> std::io::Result<_> {
            Ok((
                font_files()?,
                logo_pixels(include_bytes!(
                    "../../../docs/images/fankit/made-by-the-community.png"
                ))?,
                logo_pixels(include_bytes!(
                    "../../../docs/images/fankit/MadeByTheCommunity_White.png"
                ))?,
                tallyline_app::api::native_window_icon()?,
            ))
        })?
        .join()
        .map_err(|_| std::io::Error::other("resource worker terminated unexpectedly"))??;
    let (lifecycle_sender, lifecycle_receiver) = std::sync::mpsc::sync_channel(1);
    let (http_driver, http_client) = http_wiring::start(information)?;
    let result = eframe::run_native(
        "Tallyline",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1000.0, 900.0])
                .with_min_inner_size([800.0, 640.0])
                .with_icon(icon),
            ..Default::default()
        },
        Box::new(move |creation| {
            configure_fonts(&creation.egui_ctx, files);
            let catalogue = Catalogue::english()?;

            let (activation_commands, activation_events) = std::sync::mpsc::sync_channel(1);
            let activation_worker =
                instance_wiring::start(listener, activation_commands, creation.egui_ctx.clone())?;
            let (
                commands,
                reference_commands,
                manual_commands,
                credential_commands,
                submission_commands,
                events,
                worker,
                credential_worker,
            ) = settings_wiring::start(creation.egui_ctx.clone(), layout, http_client)?;
            lifecycle_sender.send((worker, activation_worker, credential_worker))?;
            Ok(Box::new(
                DesktopShell::new(
                    &creation.egui_ctx,
                    catalogue,
                    light_logo,
                    dark_logo,
                    commands,
                    events,
                )
                .with_build_presentation(build)
                .with_window_commands(activation_events)
                .with_reference_commands(reference_commands)
                .with_manual_commands(manual_commands)
                .with_credential_commands(credential_commands)
                .with_submission_commands(submission_commands),
            ))
        }),
    );
    for (worker, activation_worker, credential_worker) in lifecycle_receiver.try_iter() {
        worker
            .join()
            .map_err(|_| std::io::Error::other("settings worker terminated unexpectedly"))?;
        credential_worker.finish()?;
        activation_worker.finish()?;
    }
    http_driver.finish()?;
    result?;
    Ok(())
}

fn font_files() -> std::io::Result<tallyline_ui::api::model::FontFiles> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| std::io::Error::other("executable directory is missing"))?;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let root = match arguments.as_slice() {
        [] => directory.join("assets/fonts"),
        [option, path] if option == "--assets" => std::path::PathBuf::from(path).join("fonts"),
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "usage: tallyline [--assets <resource directory>]",
            ));
        }
    };
    let read = |name: &str| std::fs::read(root.join(name));
    Ok(tallyline_ui::api::model::FontFiles {
        material_symbols: read("material-symbols/MaterialSymbolsRounded-Tallyline.ttf")?
            .try_into()
            .map_err(std::io::Error::other)?,
        plex_regular: read("ibm-plex/IBMPlexSans-Regular.ttf")?,
        plex_semibold: read("ibm-plex/IBMPlexSans-SemiBold.ttf")?,
        plex_bold: read("ibm-plex/IBMPlexSans-Bold.ttf")?,
        mono_regular: read("jetbrains-mono/JetBrainsMonoNL-Regular.ttf")?,
        mono_bold: read("jetbrains-mono/JetBrainsMonoNL-Bold.ttf")?,
    })
}

fn logo_pixels(bytes: &[u8]) -> std::io::Result<egui::ColorImage> {
    let raster = tallyline_adapter_files::api::decode_logo(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [raster.width as usize, raster.height as usize],
        &raster.rgba,
    ))
}

fn already_running() -> Result<(), Box<dyn std::error::Error>> {
    let fonts = font_files()?;
    let icon = executor::spawn("tallyline-icon", tallyline_app::api::native_window_icon)?
        .join()
        .map_err(|_| std::io::Error::other("icon worker terminated unexpectedly"))??;
    let catalogue = Catalogue::english()?;
    eframe::run_native(
        "Tallyline",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([420.0, 140.0])
                .with_resizable(false)
                .with_icon(icon),
            ..Default::default()
        },
        Box::new(move |creation| {
            configure_fonts(&creation.egui_ctx, fonts);
            Ok(Box::new(tallyline_ui::api::AlreadyRunningNotice::new(
                catalogue,
            )))
        }),
    )?;
    Ok(())
}
