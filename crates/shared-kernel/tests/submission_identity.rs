// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable identities cannot alias contexts or test-only remote IDs.

use tallyline_shared_kernel::api::model::{EventId, EventSource, Timestamp, UexReportId};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__outbox_identity_is_bound_to_context_aggregate_and_committed_version() {
    assert!(EventId::new(EventSource::Submission, 0, 1).is_none());
    assert_eq!(
        EventId::new(EventSource::Reporting, 1, 0)
            .unwrap()
            .version(),
        0
    );
    let event = EventId::new(EventSource::Submission, 1, 2).unwrap();
    assert_eq!(event.source(), EventSource::Submission);
    assert_ne!(Some(event), EventId::new(EventSource::Reporting, 1, 2));
    assert_ne!(Some(event), EventId::new(EventSource::Submission, 2, 2));
    assert_ne!(Some(event), EventId::new(EventSource::Submission, 1, 3));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_8__remote_identity_requires_a_positive_id_and_exact_partition_second() {
    assert!(UexReportId::new(0, Timestamp::from_seconds(100)).is_none());
    assert!(UexReportId::new(42, Timestamp::from_millis(100_001)).is_none());
    let id = UexReportId::new(42, Timestamp::from_seconds(u64::MAX)).unwrap();
    assert_eq!(id.id(), 42);
    assert_eq!(id.date_added(), Timestamp::from_seconds(u64::MAX));
    assert_ne!(Some(id), UexReportId::new(42, Timestamp::from_seconds(100)));
}
