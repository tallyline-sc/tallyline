// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real OS worker-priority contracts.

use tallyline_adapter_platform::api::{lower_worker_priority, worker_priority};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_nf_3__lowering_one_worker_preserves_the_callers_priority() {
    let caller = worker_priority().unwrap();
    let worker = std::thread::spawn(|| {
        let lowered = lower_worker_priority().unwrap();
        assert!(lowered.is_below_normal());
        assert_eq!(worker_priority().unwrap(), lowered);
        assert_eq!(lower_worker_priority().unwrap(), lowered);
    });
    worker.join().unwrap();
    assert_eq!(worker_priority().unwrap(), caller);
}
