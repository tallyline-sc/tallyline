// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared finite container-size boundary.

use tallyline_shared_kernel::api::model::ContainerSize;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_5__container_sizes_accept_only_the_documented_scu_values() {
    for (scu, size) in [
        (1, ContainerSize::Scu1),
        (2, ContainerSize::Scu2),
        (4, ContainerSize::Scu4),
        (8, ContainerSize::Scu8),
        (16, ContainerSize::Scu16),
        (24, ContainerSize::Scu24),
        (32, ContainerSize::Scu32),
    ] {
        assert_eq!(ContainerSize::from_scu(scu), Some(size));
        assert_eq!(size.scu(), scu);
    }
    for invalid in [0, 3, 6, 15, 33, u32::MAX] {
        assert_eq!(ContainerSize::from_scu(invalid), None);
    }
}
