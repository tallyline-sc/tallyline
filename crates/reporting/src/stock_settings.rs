// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated tighten-only SCU settings.

use tallyline_reference_data::api::model::ScuTolerance;
use tallyline_shared_kernel::api::model::{Percent, ScuQuantity};

/// A rejected attempt to loosen either shipping SCU band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidStockSettings;

/// Independently tighten-only percentage and absolute SCU bands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockSettings(ScuTolerance);
impl Default for StockSettings {
    fn default() -> Self {
        Self::baseline()
    }
}
impl StockSettings {
    /// Returns the provisional shipping baseline without an override.
    #[must_use]
    pub const fn baseline() -> Self {
        Self(ScuTolerance::baseline())
    }
    /// Validates both bands against the provisional corpus-informed baseline.
    ///
    /// # Errors
    /// Refuses a percentage above 20 or an absolute minimum above 16 SCU.
    pub fn new(percent: Percent, minimum: ScuQuantity) -> Result<Self, InvalidStockSettings> {
        let tolerance = ScuTolerance::baseline().tighten(percent, minimum);
        if tolerance.override_ignored() {
            return Err(InvalidStockSettings);
        }
        Ok(Self(tolerance))
    }
    /// Returns the validated effective tolerance.
    #[must_use]
    pub const fn tolerance(self) -> ScuTolerance {
        self.0
    }
}
