// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value-bound explicit status approvals.

use crate::api::model::Deviation;
use std::num::NonZeroU8;
use tallyline_shared_kernel::api::model::UexEnvironment;

/// An explicit approval of exactly one status at its unlowered deviation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusConfirmation {
    value: NonZeroU8,
    deviation: Deviation,
    environment: UexEnvironment,
}
impl StatusConfirmation {
    pub(crate) const fn approve(
        value: NonZeroU8,
        deviation: Deviation,
        environment: UexEnvironment,
    ) -> Self {
        Self {
            value,
            deviation,
            environment,
        }
    }
    /// Returns the mapped UEX environment against which this value was approved.
    #[must_use]
    pub const fn reference_environment(self) -> UexEnvironment {
        self.environment
    }
    /// Returns the approved value.
    #[must_use]
    pub const fn value(self) -> NonZeroU8 {
        self.value
    }
    /// Returns the unlowered deviation at approval.
    #[must_use]
    pub const fn deviation(self) -> Deviation {
        self.deviation
    }
}
