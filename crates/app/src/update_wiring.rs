// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded release-request worker and injected system time.

use std::{
    io,
    sync::mpsc::{self, SyncSender},
    thread::JoinHandle,
};
use tallyline_workflows::api::{
    check_update,
    model::{ReleaseVersion, UpdateStatus},
};

pub(crate) struct SystemClock;
impl tallyline_shared_kernel::api::Clock for SystemClock {
    fn now(&self) -> tallyline_shared_kernel::api::model::Timestamp {
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        tallyline_shared_kernel::api::model::Timestamp::from_millis(
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        )
    }
}

pub(crate) fn start(
    installed: ReleaseVersion,
) -> io::Result<(SyncSender<()>, mpsc::Receiver<UpdateStatus>, JoinHandle<()>)> {
    let (requests, incoming) = mpsc::sync_channel(1);
    let (results, completed) = mpsc::sync_channel(1);
    let worker = crate::executor::spawn("tallyline-update", move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let version = installed.to_string();
        let client =
            tallyline_adapter_platform::api::ReleaseClient::new(version.trim_start_matches('v'));
        while incoming.recv().is_ok() {
            let status = match (&runtime, &client) {
                (Ok(runtime), Ok(client)) => runtime.block_on(check_update(client, installed)),
                _ => UpdateStatus::Unavailable,
            };
            if results.send(status).is_err() {
                break;
            }
        }
    })?;
    Ok((requests, completed, worker))
}
