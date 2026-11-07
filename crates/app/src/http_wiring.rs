// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! One owned UEX transport shared by credential, reference and submission workers.

use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use tallyline_adapter_uex::api::model::ServerTimeTracker;
use tallyline_adapter_uex::api::{BuildError, ClientIdentity, ReaderSettings, UexReader};
use tallyline_app::api::{HttpClient, HttpDriver};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionClock;

struct SystemClock {
    origin: Instant,
}
impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        crate::update_wiring::SystemClock.now()
    }
}
impl SubmissionClock for SystemClock {
    fn monotonic(&self) -> Duration {
        self.origin.elapsed()
    }
}

pub(crate) fn start(
    information: tallyline_app::api::model::BuildInformation,
) -> io::Result<(HttpDriver, HttpClient)> {
    let tracker = ServerTimeTracker::new(Arc::new(SystemClock {
        origin: Instant::now(),
    }));
    let responses = tracker.clone();
    HttpDriver::start_with_processing(
        move |processor| {
            let settings = ReaderSettings::new(
                "https://api.uexcorp.uk/2.0/",
                ClientIdentity::new("tallyline", information.version()),
            )
            .map_err(|_| BuildError::Client("primary UEX URL is invalid".to_owned()))?;
            UexReader::new_with_processor(&settings, processor)
                .map(|reader| reader.with_server_time(responses))
        },
        Some(tracker),
    )
}
