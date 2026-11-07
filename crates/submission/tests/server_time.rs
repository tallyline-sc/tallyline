// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fresh monotonic UEX server-time estimates only move observation time earlier.

use std::time::Duration;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::ServerTimeEstimate;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_6__server_time_uses_monotonic_elapsed_time_and_never_moves_observation_later() {
    let estimate = ServerTimeEstimate::new(Timestamp::from_seconds(100), Duration::from_secs(50));
    assert_eq!(
        estimate.at(Duration::from_millis(55250)),
        Some(Timestamp::from_millis(105250))
    );
    assert_eq!(
        estimate.clamp(Timestamp::from_seconds(200), Duration::from_millis(55250)),
        Timestamp::from_millis(105250)
    );
    assert_eq!(
        estimate.clamp(Timestamp::from_seconds(90), Duration::from_secs(55)),
        Timestamp::from_seconds(90)
    );
    assert_eq!(
        estimate.at(Duration::from_secs(950)),
        Some(Timestamp::from_seconds(1000))
    );
    assert_eq!(estimate.at(Duration::from_millis(950001)), None);
    assert_eq!(
        estimate.clamp(Timestamp::from_seconds(5000), Duration::from_millis(950001)),
        Timestamp::from_seconds(5000)
    );
    assert_eq!(estimate.at(Duration::from_secs(49)), None);
    let overflow = ServerTimeEstimate::new(Timestamp::from_seconds(u64::MAX), Duration::ZERO);
    assert_eq!(overflow.at(Duration::from_secs(1)), None);
}
