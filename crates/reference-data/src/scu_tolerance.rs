// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit SCU bands without a shipping baseline.

use tallyline_shared_kernel::api::model::{Delta, Percent, PrecisionUnavailable, ScuQuantity};

/// An explicitly supplied SCU percentage and absolute band, without calibration provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScuTolerance {
    percent: Percent,
    minimum: ScuQuantity,
    override_ignored: bool,
}
impl ScuTolerance {
    /// Creates explicit bands without selecting or implying a calibrated default.
    #[must_use]
    pub const fn new(percent: Percent, minimum: ScuQuantity) -> Self {
        Self {
            percent,
            minimum,
            override_ignored: false,
        }
    }
    /// Returns the whole-percentage band.
    #[must_use]
    pub const fn percent(self) -> Percent {
        self.percent
    }
    /// Returns the absolute band in SCU.
    #[must_use]
    pub const fn minimum(self) -> ScuQuantity {
        self.minimum
    }
    /// Tests whether both exact bands are strictly exceeded.
    ///
    /// # Errors
    /// Returns precision unavailable rather than rounding an unrepresentable comparison.
    pub fn exceeded_by(self, difference: Delta) -> Result<bool, PrecisionUnavailable> {
        Ok(difference.exceeds_percentage(self.percent)?
            && difference.exceeds_quantity(self.minimum)?)
    }
}

impl ScuTolerance {
    /// Applies each override only where it tightens the currently supplied bands.
    #[must_use]
    pub fn tighten(self, percent: Percent, minimum: ScuQuantity) -> Self {
        Self {
            percent: Percent::whole(self.percent.get().min(percent.get())),
            minimum: self.minimum.min(minimum),
            override_ignored: percent.get() > self.percent.get() || minimum > self.minimum,
        }
    }
    /// Reports whether either attempted override was too loose for its band.
    #[must_use]
    pub const fn override_ignored(self) -> bool {
        self.override_ignored
    }
}

/// Provenance of the provisional corpus-informed shipping bands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScuToleranceSource {
    /// The documented public quantity sensitivity and observed trading movement.
    ProvisionalPublicCorpus,
}
impl ScuTolerance {
    /// Returns the provisional 20 percent and 16 SCU shipping baseline.
    #[must_use]
    pub const fn baseline() -> Self {
        Self::new(Percent::whole(20), ScuQuantity::whole(16))
    }
    /// Returns the documented shipping baseline provenance.
    #[must_use]
    pub const fn baseline_source() -> ScuToleranceSource {
        ScuToleranceSource::ProvisionalPublicCorpus
    }
}
