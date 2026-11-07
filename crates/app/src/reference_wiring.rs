// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Primary-host reference reader composed after coordinated storage bootstrap.

use std::io;
use std::sync::mpsc::{Receiver, SyncSender};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_ui::api::model::{BackgroundEvent, ReferenceCommand};

pub(crate) fn start(
    cache: SqliteReferenceCache,
    incoming: Receiver<ReferenceCommand>,
    outgoing: SyncSender<BackgroundEvent>,
    context: egui::Context,
    client: tallyline_app::api::HttpClient,
    history_path: std::path::PathBuf,
    refresh: tallyline_app::api::ReleaseReferenceOwner,
) -> io::Result<tallyline_app::api::ReferenceWorker> {
    let mut history: Option<SqliteReferenceCache> = None;
    tallyline_app::api::ReferenceWorker::start_with_history_and_refresh(
        cache,
        crate::update_wiring::SystemClock,
        (incoming, outgoing),
        move || Ok::<_, io::Error>(client.clone()),
        move || {
            use tallyline_reference_data::api::AcceptedPriorHistoryStore;
            if history.is_none() {
                history = Some(
                    SqliteReferenceCache::open(&history_path)
                        .map_err(|_| io::Error::other("local reference history unavailable"))?,
                );
            }
            history
                .as_mut()
                .ok_or_else(|| io::Error::other("local reference history unavailable"))?
                .accepted_history()
                .map_err(|_| io::Error::other("local reference history unavailable"))
        },
        refresh,
        move || tallyline_ui::api::wake_up(&context),
    )
}
