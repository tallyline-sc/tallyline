// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact LIVE priors and their source provenance per terminal.

use std::num::NonZeroU8;
use tallyline_shared_kernel::api::model::{
    CommodityId, ContainerSize, PricePerScu, ScuQuantity, TerminalId, Timestamp, TradeSide,
};

/// An invalid prior identity, traded price or provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidPrior;
impl std::fmt::Display for InvalidPrior {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("terminal prior identity, price or provenance is invalid")
    }
}
impl std::error::Error for InvalidPrior {}

/// Exact observed values on the terminal's single traded side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorValues {
    /// Positive price in aUEC per SCU.
    pub price: PricePerScu,
    /// Reported inventory in SCU, preserving a known zero.
    pub stock: ScuQuantity,
    /// Optional status code, with zero represented as unknown.
    pub status: Option<NonZeroU8>,
    /// Known container sizes for this terminal and commodity.
    pub container_sizes: Vec<ContainerSize>,
}
/// The server version and modification time of this price row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorProvenance {
    /// Unmodified UEX version string without semantic-version assumptions.
    pub game_version: String,
    /// Server row modification time, distinct from the client fetch time.
    pub modified_at: Timestamp,
}
/// One validated single-side commodity prior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PricePrior {
    commodity: CommodityId,
    side: TradeSide,
    values: PriorValues,
    provenance: PriorProvenance,
}
impl PricePrior {
    /// Creates a prior with a positive identity, price and nonempty source version.
    ///
    /// # Errors
    /// Returns an invalid identity, zero traded price or empty version.
    pub fn new(
        commodity: CommodityId,
        side: TradeSide,
        values: PriorValues,
        provenance: PriorProvenance,
    ) -> Result<Self, InvalidPrior> {
        if commodity.0 == 0 || values.price.is_zero() || provenance.game_version.trim().is_empty() {
            return Err(InvalidPrior);
        }
        Ok(Self {
            commodity,
            side,
            values,
            provenance,
        })
    }
    /// Returns the commodity identity.
    #[must_use]
    pub const fn commodity(&self) -> CommodityId {
        self.commodity
    }
    /// Returns the single traded side.
    #[must_use]
    pub const fn side(&self) -> TradeSide {
        self.side
    }
    /// Returns exact row values without exposing mutable snapshot state.
    #[must_use]
    pub const fn values(&self) -> &PriorValues {
        &self.values
    }
    /// Returns the row's source version and modification time.
    #[must_use]
    pub const fn provenance(&self) -> &PriorProvenance {
        &self.provenance
    }
}
/// A successfully fetched assortment for one LIVE terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalPriors {
    terminal: TerminalId,
    rows: Vec<PricePrior>,
}
impl TerminalPriors {
    /// Creates one terminal assortment with distinct commodity identities.
    ///
    /// # Errors
    /// Returns a zero terminal identity or repeated commodity.
    pub fn new(terminal: TerminalId, rows: Vec<PricePrior>) -> Result<Self, InvalidPrior> {
        let mut identities = std::collections::BTreeSet::new();
        if terminal.0 == 0 || !rows.iter().all(|row| identities.insert(row.commodity())) {
            return Err(InvalidPrior);
        }
        Ok(Self { terminal, rows })
    }
    /// Returns the terminal identity.
    #[must_use]
    pub const fn terminal(&self) -> TerminalId {
        self.terminal
    }
    /// Returns this terminal's immutable price rows.
    #[must_use]
    pub fn rows(&self) -> &[PricePrior] {
        &self.rows
    }
}
