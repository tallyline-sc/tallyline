// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded technical identifiers classified once at the adapter boundary.

/// A documented conflict between buy-side and sell-side row fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowFieldConflict {
    /// Both buy and sell prices were supplied.
    Prices,
    /// Both buy and sell quantities were supplied.
    Quantities,
    /// Both buy and sell statuses were supplied.
    Statuses,
    /// Buy price and sell status were supplied.
    BuyPriceSellStatus,
    /// Buy price and sell quantity were supplied.
    BuyPriceSellQuantity,
    /// Sell price and buy status were supplied.
    SellPriceBuyStatus,
    /// Sell price and buy quantity were supplied.
    SellPriceBuyQuantity,
}
/// The closed semantic cause retained beside the original safe technical identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowFailureKind {
    /// The failing row could not be stored.
    Database,
    /// An existing same-account commodity report caused a cooldown refusal.
    Duplicate,
    /// The supplied row array or its format was invalid.
    InvalidPrices,
    /// A required reference key was absent.
    MissingReference,
    /// No commodity rows were found.
    NoCommodities,
    /// The row's commodity identity was absent.
    MissingCommodity,
    /// The row's commodity identity was invalid.
    InvalidCommodity,
    /// Neither a price nor an explicit missing mark was supplied.
    MissingPrice,
    /// Row fields combined conflicting buy-side and sell-side values.
    ConflictingSides(RowFieldConflict),
    /// The buy-side status was invalid.
    InvalidBuyStatus,
    /// The sell-side status was invalid.
    InvalidSellStatus,
    /// The quality value was invalid.
    InvalidQuality,
    /// The bounded identifier has no documented row-specific semantic cause.
    Unrecognised,
}
/// A bounded ASCII status identifier safe for passive queue presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UexErrorCode {
    value: Box<str>,
    row_failure: RowFailureKind,
}
impl UexErrorCode {
    /// Accepts a bounded nonempty ASCII identifier without assigning a row cause.
    #[must_use]
    pub fn new(value: String) -> Option<Self> {
        Self::classified(value, RowFailureKind::Unrecognised)
    }
    /// Retains an adapter-supplied cause beside a bounded nonempty ASCII identifier.
    #[must_use]
    pub fn classified(value: String, row_failure: RowFailureKind) -> Option<Self> {
        (!value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
        .then(|| Self {
            row_failure,
            value: value.into_boxed_str(),
        })
    }
    /// Borrows the exact bounded technical identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
    /// Returns the closed cause without downstream technical-code parsing.
    #[must_use]
    pub const fn row_failure(&self) -> RowFailureKind {
        self.row_failure
    }
}
