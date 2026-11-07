// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public app-owned CPU worker start and lifetime contracts.

use tallyline_adapter_platform::api::worker_priority;
use tallyline_app::api::CpuWorker;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_nf_3__cpu_task_starts_below_normal_and_returns_through_its_owner() {
    let before = worker_priority().unwrap();
    let worker = CpuWorker::start("tallyline-cpu-contract", || {
        assert!(tokio::runtime::Handle::try_current().is_err());
        assert_eq!(
            std::thread::current().name(),
            Some("tallyline-cpu-contract")
        );
        assert!(worker_priority().unwrap().is_below_normal());
        42
    })
    .unwrap();
    assert_eq!(worker.finish().unwrap(), 42);
    assert_eq!(worker_priority().unwrap(), before);
}
