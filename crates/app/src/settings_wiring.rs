// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The single settings worker and bounded UI channels.

use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};
use tallyline_adapter_files::api::{ConfigFile, StorageLayout};
use tallyline_ui::api::model::{BackgroundEvent, ReferenceCommand, SettingsCommand};
use tallyline_ui::api::wake_up;
use tallyline_workflows::api::SettingsApplication;

type WorkerChannels = (
    SyncSender<SettingsCommand>,
    SyncSender<ReferenceCommand>,
    SyncSender<tallyline_ui::api::model::ManualCommand>,
    SyncSender<tallyline_ui::api::model::CredentialCommand>,
    SyncSender<tallyline_ui::api::model::SubmissionControl>,
    Receiver<BackgroundEvent>,
    std::thread::JoinHandle<()>,
    tallyline_app::api::CredentialWorker,
);

pub(crate) fn start(
    context: egui::Context,
    layout: StorageLayout,
    client: tallyline_app::api::HttpClient,
) -> io::Result<WorkerChannels> {
    let (commands, incoming) = mpsc::sync_channel(1);
    let (reference_commands, reference_incoming) = mpsc::sync_channel(1);
    let (manual_commands, manual_incoming) = mpsc::sync_channel(1);
    let (credential_commands, credential_incoming) = mpsc::sync_channel(1);
    let (outgoing, events) = mpsc::sync_channel(16);
    let (submission_commands, submission_incoming) = mpsc::sync_channel(1);
    let (activate, activation) = mpsc::sync_channel(1);
    let credential_worker = crate::credential_wiring::start(
        layout.data().join("credentials"),
        credential_incoming,
        outgoing.clone(),
        context.clone(),
        client.clone(),
        activation,
    )?;
    let reference_publication = reference_commands.clone();
    let worker = crate::executor::spawn("tallyline-settings", move || {
        let result = run(
            &incoming,
            &outgoing,
            &context,
            layout,
            (
                reference_incoming,
                manual_incoming,
                submission_incoming,
                activate,
                reference_publication,
            ),
            client,
        );
        if let Err(error) = result {
            let _sent = outgoing.try_send(BackgroundEvent::Failure(error.to_string()));
            wake_up(&context);
        }
    })?;
    Ok((
        commands,
        reference_commands,
        manual_commands,
        credential_commands,
        submission_commands,
        events,
        worker,
        credential_worker,
    ))
}

fn deliver(
    sender: &SyncSender<BackgroundEvent>,
    context: &egui::Context,
    event: BackgroundEvent,
) -> io::Result<()> {
    sender.send(event).map_err(io::Error::other)?;
    wake_up(context);
    Ok(())
}

type WorkerInputs = (
    Receiver<ReferenceCommand>,
    Receiver<tallyline_ui::api::model::ManualCommand>,
    Receiver<tallyline_ui::api::model::SubmissionControl>,
    SyncSender<Box<dyn tallyline_app::api::CredentialQueue>>,
    SyncSender<ReferenceCommand>,
);

fn run(
    incoming: &Receiver<SettingsCommand>,
    outgoing: &SyncSender<BackgroundEvent>,
    context: &egui::Context,
    layout: StorageLayout,
    workers: WorkerInputs,
    client: tallyline_app::api::HttpClient,
) -> io::Result<()> {
    let (reference_incoming, manual_incoming, submission_incoming, activate, reference_publication) =
        workers;
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("executable directory is missing"))?;
    let legal_event = match std::fs::read_to_string(directory.join("legal/catalogue.json")) {
        Ok(text) => match tallyline_ui::api::parse_licence_catalogue(&text) {
            Ok(catalogue) => BackgroundEvent::LicencesLoaded(catalogue),
            Err(_) => BackgroundEvent::LicencesUnavailable,
        },
        Err(_) => BackgroundEvent::LicencesUnavailable,
    };
    deliver(outgoing, context, legal_event)?;
    let (warnings, issues): (Vec<_>, Vec<_>) = layout.self_test().into_iter().partition(|issue| {
        issue.kind == tallyline_adapter_files::api::model::DirectoryIssueKind::PermissionsWarning
    });
    if !warnings.is_empty() {
        deliver(
            outgoing,
            context,
            BackgroundEvent::StorageWarning(
                warnings
                    .iter()
                    .map(|issue| format!("{}: {}", issue.path.display(), issue.error))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        )?;
    }
    if !issues.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            issues
                .iter()
                .map(|issue| format!("{}: {}", issue.path.display(), issue.error))
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }
    let settings_path = layout.config().join("settings.json");
    let tallyline_app::api::model::PreparedStorage {
        settings: store,
        cache,
    } = tallyline_app::api::initialise_storage(&layout, &crate::update_wiring::SystemClock)?;
    let loaded = store.load()?;
    deliver(
        outgoing,
        context,
        BackgroundEvent::SettingsLoaded(loaded.settings.clone()),
    )?;
    if !loaded.errors.is_empty() {
        deliver(
            outgoing,
            context,
            BackgroundEvent::Failure(
                loaded
                    .errors
                    .iter()
                    .map(|issue| format!("{}: {} → {}", issue.key, issue.rejected, issue.default))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        )?;
    }
    let (release_refresh, release_owner) = tallyline_app::api::ReleaseReferenceClient::channel();
    crate::submission_wiring::activate(
        layout.clone(),
        client.clone(),
        (submission_incoming, outgoing.clone(), reference_publication),
        &activate,
        context.clone(),
        release_refresh,
    )?;
    deliver_game(outgoing, context, &settings_path)?;
    let reference_worker = crate::reference_wiring::start(
        cache,
        reference_incoming,
        outgoing.clone(),
        context.clone(),
        client,
        layout.data().join("tallyline.db"),
        release_owner,
    )?;
    let manual_store = tallyline_adapter_storage::api::SqliteReferenceCache::open(
        layout.data().join("tallyline.db"),
    )
    .map_err(io::Error::other)?;
    let manual_settings = ConfigFile::new(settings_path.clone());
    let manual_context = context.clone();
    let manual_worker = tallyline_app::api::ManualWorker::start_with_attachments(
        manual_store,
        crate::update_wiring::SystemClock,
        manual_incoming,
        outgoing.clone(),
        tallyline_app::api::ManualAttachmentStorage::new(layout.data().to_owned()),
        move || {
            manual_settings
                .load_game_preferences()
                .map(|loaded| loaded.settings.environment_mapping)
        },
        move || wake_up(&manual_context),
    )?;
    let mut settings = SettingsApplication::new(loaded.settings, store);
    let installed = tallyline_app::api::compiled_build_information()
        .map_err(|_| io::Error::other("compiled build information is invalid"))?
        .update_baseline();
    deliver(
        outgoing,
        context,
        BackgroundEvent::UpdateChecked(settings.settings().cached_update(installed)),
    )?;
    let (update_requests, update_results, update_worker) = crate::update_wiring::start(installed)?;
    let mut checking = false;
    let result = (|| {
        loop {
            match update_results.try_recv() {
                Ok(status) => {
                    checking = false;
                    let status = match settings.record_update(status, installed) {
                        Ok(status) => status,
                        Err(_) => settings.settings().resolve_update(
                            installed,
                            tallyline_workflows::api::model::UpdateStatus::Unavailable,
                        ),
                    };
                    deliver(outgoing, context, BackgroundEvent::UpdateChecked(status))?;
                }
                Err(mpsc::TryRecvError::Empty) => (),
                Err(mpsc::TryRecvError::Disconnected) => {
                    if checking {
                        checking = false;
                        deliver(
                            outgoing,
                            context,
                            BackgroundEvent::UpdateChecked(settings.settings().resolve_update(
                                installed,
                                tallyline_workflows::api::model::UpdateStatus::Unavailable,
                            )),
                        )?;
                    }
                }
            }
            let update_unavailable =
                match settings.begin_update_check(&crate::update_wiring::SystemClock) {
                    Ok(true) => {
                        checking = true;
                        update_requests.try_send(()).is_err()
                    }
                    Ok(false) => false,
                    Err(_) => true,
                };
            if update_unavailable {
                deliver(
                    outgoing,
                    context,
                    BackgroundEvent::UpdateChecked(settings.settings().resolve_update(
                        installed,
                        tallyline_workflows::api::model::UpdateStatus::Unavailable,
                    )),
                )?;
            }
            let wait = if checking {
                std::time::Duration::from_millis(500)
            } else {
                std::time::Duration::from_secs(60)
            };
            let command = match incoming.recv_timeout(wait) {
                Ok(command) => command,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let app_command = match &command {
                SettingsCommand::ChangeSubmissionMode(_)
                | SettingsCommand::ChangeObservationAge(_)
                | SettingsCommand::ChangeStockSettings(_)
                | SettingsCommand::ChangeTheme(_)
                | SettingsCommand::CompleteOnboarding
                | SettingsCommand::ChangeUpdateChecks(_)
                | SettingsCommand::DismissUpdate => true,
                SettingsCommand::ChangeEnvironmentMapping(_, _)
                | SettingsCommand::ChangeWitnessMode(_)
                | SettingsCommand::ChangeGameChannel(_)
                | SettingsCommand::RefreshWitness => false,
            };
            let update_command = matches!(
                command,
                SettingsCommand::DismissUpdate | SettingsCommand::ChangeUpdateChecks(_)
            );
            let result = match command {
                SettingsCommand::ChangeSubmissionMode(mode) => {
                    settings.change_submission_mode(mode)
                }
                SettingsCommand::ChangeObservationAge(age) => settings.change_observation_age(age),
                SettingsCommand::ChangeEnvironmentMapping(channel, environment) => {
                    crate::game_wiring::change_mapping(settings_path.clone(), channel, environment)
                }
                SettingsCommand::DismissUpdate => settings.dismiss_update(),
                SettingsCommand::ChangeUpdateChecks(checks) => {
                    settings.change_update_checks(checks)
                }
                SettingsCommand::ChangeStockSettings(value) => {
                    settings.change_stock_settings(value)
                }
                SettingsCommand::ChangeTheme(theme) => settings.change_theme(theme),
                SettingsCommand::CompleteOnboarding => settings.complete_onboarding(),
                SettingsCommand::ChangeWitnessMode(mode) => {
                    crate::game_wiring::change_mode(settings_path.clone(), mode)
                }
                SettingsCommand::ChangeGameChannel(channel) => {
                    ConfigFile::new(settings_path.clone()).change_game_channel(&channel)
                }
                SettingsCommand::RefreshWitness => Ok(()),
            };
            match result {
                Ok(()) => {
                    if app_command {
                        deliver(
                            outgoing,
                            context,
                            BackgroundEvent::SettingsLoaded(settings.settings().clone()),
                        )?;
                        if update_command {
                            deliver(
                                outgoing,
                                context,
                                BackgroundEvent::UpdateChecked(
                                    settings.settings().cached_update(installed),
                                ),
                            )?;
                        }
                    } else {
                        deliver_game(outgoing, context, &settings_path)?;
                    }
                }
                Err(error) => deliver(
                    outgoing,
                    context,
                    BackgroundEvent::Failure(error.to_string()),
                )?,
            }
        }
        Ok(())
    })();
    let manual_result = manual_worker.finish();
    let reference_result = reference_worker.finish();
    drop(update_requests);
    update_worker
        .join()
        .map_err(|_| io::Error::other("update worker terminated unexpectedly"))?;
    manual_result?;
    reference_result?;
    result
}

pub(crate) fn profile_path(key: &str) -> io::Result<PathBuf> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing profile directory: {key}"),
            )
        })
}

pub(crate) fn storage_layout() -> io::Result<StorageLayout> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "executable directory missing"))?;
    if directory.join("portable.marker").is_file() {
        return Ok(StorageLayout::portable(directory));
    }
    #[cfg(windows)]
    return Ok(StorageLayout::installed_windows(
        &profile_path("APPDATA")?,
        &profile_path("LOCALAPPDATA")?,
    ));
    #[cfg(unix)]
    {
        let home = profile_path("HOME")?;
        let xdg = |key: &str, default: &str| {
            std::env::var_os(key)
                .map(PathBuf::from)
                .filter(|value| value.is_absolute())
                .unwrap_or_else(|| home.join(default))
        };
        Ok(StorageLayout::installed_linux(
            &xdg("XDG_CONFIG_HOME", ".config"),
            &xdg("XDG_DATA_HOME", ".local/share"),
            &xdg("XDG_STATE_HOME", ".local/state"),
            &xdg("XDG_CACHE_HOME", ".cache"),
        ))
    }
}

fn deliver_game(
    sender: &SyncSender<BackgroundEvent>,
    context: &egui::Context,
    path: &std::path::Path,
) -> io::Result<()> {
    let (view, issues) = crate::game_wiring::view(&ConfigFile::new(path.to_owned()))?;
    deliver(sender, context, BackgroundEvent::GameWitnessLoaded(view))?;
    if !issues.is_empty() {
        deliver(sender, context, BackgroundEvent::Failure(issues.join("\n")))?;
    }
    Ok(())
}
