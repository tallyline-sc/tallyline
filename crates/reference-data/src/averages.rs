// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact commodity-wide LIVE averages distinguished by terminal side.

use tallyline_shared_kernel::api::model::{PricePerScu, TradeSide};

/// Optional current LIVE averages with zero represented as missing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommodityAverages {
    buy: Option<PricePerScu>,
    sell: Option<PricePerScu>,
}
impl CommodityAverages {
    /// Creates side-specific averages without treating zero as a usable substitute.
    #[must_use]
    pub fn new(buy: Option<PricePerScu>, sell: Option<PricePerScu>) -> Self {
        Self {
            buy: buy.filter(|price| !price.is_zero()),
            sell: sell.filter(|price| !price.is_zero()),
        }
    }
    pub(crate) const fn price(self, side: TradeSide) -> Option<PricePerScu> {
        match side {
            TradeSide::Buy => self.buy,
            TradeSide::Sell => self.sell,
        }
    }
}
