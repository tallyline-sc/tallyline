// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Value-bound manual stock decisions.

use crate::api::model::Deviation;
use tallyline_shared_kernel::api::model::{ScuQuantity, UexEnvironment};

/// Explicit approval of one exact stock and its unlowered deviation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockConfirmation {
    value: ScuQuantity,
    deviation: Deviation,
    environment: UexEnvironment,
}
impl StockConfirmation {
    pub(crate) const fn approve(
        value: ScuQuantity,
        deviation: Deviation,
        environment: UexEnvironment,
    ) -> Self {
        Self {
            value,
            deviation,
            environment,
        }
    }
    /// Returns the approved stock.
    #[must_use]
    pub const fn value(self) -> ScuQuantity {
        self.value
    }
    /// Returns the unlowered deviation at approval.
    #[must_use]
    pub const fn deviation(self) -> Deviation {
        self.deviation
    }
    /// Returns the mapped reference environment at approval.
    #[must_use]
    pub const fn reference_environment(self) -> UexEnvironment {
        self.environment
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct StockDecision {
    pub(crate) confirmation: Option<StockConfirmation>,
    pub(crate) deviation: Option<Deviation>,
    pub(crate) unreviewed: bool,
    pub(crate) revoked: bool,
}

/// Persistence-only state of a stock review decision.
#[derive(Clone, Copy, Debug, Default)]
pub struct StoredStockDecision {
    /// Exact approved stock, unlowered deviation and mapped environment.
    pub confirmation: Option<(ScuQuantity, Deviation, UexEnvironment)>,
    /// Last assessed unlowered deviation.
    pub deviation: Option<Deviation>,
    /// An unresolved decision that has not received keyboard focus.
    pub unreviewed: bool,
    /// A revoked approval still requiring a decision.
    pub revoked: bool,
}
