// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Queue activation after coordinated storage preparation, on the existing session CPU owner.

use std::io;
use std::sync::mpsc::{Receiver, SyncSender};
use tallyline_app::api::{
    CredentialQueue, DeferredCredentialQueue, HttpClient, SubmissionQueueBridge,
    SubmissionQueueController, SubmissionQueuePreferences,
};
use tallyline_ui::api::model::{BackgroundEvent, SubmissionControl};

pub(crate) fn activate(
    layout: tallyline_adapter_files::api::StorageLayout,
    client: HttpClient,
    channels: (
        Receiver<SubmissionControl>,
        SyncSender<BackgroundEvent>,
        SyncSender<tallyline_ui::api::model::ReferenceCommand>,
    ),
    activation: &SyncSender<Box<dyn CredentialQueue>>,
    context: egui::Context,
    refresh: tallyline_app::api::ReleaseReferenceClient,
) -> io::Result<()> {
    let queue = DeferredCredentialQueue::new(move || {
        let database = tallyline_adapter_storage::api::SqliteReferenceCache::open(
            layout.data().join("tallyline.db"),
        )
        .map_err(|_| io::Error::other("submission storage unavailable"))?;
        let copies = tallyline_adapter_files::api::ManualWorkingCopies::new(
            layout.data().join("manual-attachments"),
        )
        .map_err(|_| io::Error::other("submission evidence storage unavailable"))?;
        let settings =
            tallyline_adapter_files::api::ConfigFile::new(layout.config().join("settings.json"));
        let information = tallyline_app::api::compiled_build_information()
            .map_err(|_| io::Error::other("compiled build information is invalid"))?;
        let controller = SubmissionQueueController::new(
            database,
            client,
            move || {
                let loaded = settings.load()?;
                let game = settings.load_game_preferences()?;
                if !loaded.errors.is_empty() || !game.errors.is_empty() {
                    return Err(io::Error::other("submission preferences need review"));
                }
                let fallback = if information.kind()
                    == tallyline_app::api::model::BuildKind::Release
                    && loaded.settings.onboarding_complete()
                {
                    tallyline_shared_kernel::api::model::SubmissionMode::Production
                } else {
                    tallyline_shared_kernel::api::model::SubmissionMode::DryRun
                };
                Ok(SubmissionQueuePreferences {
                    reviewer: tallyline_reporting::api::ManualReviewer::new(
                        game.settings.environment_mapping,
                        loaded.settings.stock_settings(),
                        None,
                    ),
                    age: loaded.settings.observation_age(),
                    mode: loaded.settings.submission_mode(fallback),
                })
            },
            Some(copies),
            crate::update_wiring::SystemClock,
        )
        .map_err(|_| io::Error::other("submission recovery unavailable"))?;
        let publish = channels.2;
        Ok(Box::new(
            SubmissionQueueBridge::new(controller, channels.0, channels.1, move || {
                tallyline_ui::api::wake_submission(&context)
            })
            .with_release_refresh(refresh)
            .with_reference_publication(move || {
                publish
                    .try_send(tallyline_ui::api::model::ReferenceCommand::PublishCached)
                    .is_ok()
            }),
        ) as Box<dyn CredentialQueue>)
    });
    activation
        .send(Box::new(queue))
        .map_err(|_| io::Error::other("submission owner unavailable"))
}
