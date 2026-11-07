// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Source-bound price tolerance without a fabricated SCU calibration.

use crate::api::model::CommodityConstraints;
use tallyline_shared_kernel::api::model::{Percent, Timestamp};

const DOCUMENTED_PRICE_FALLBACK: SourcedPriceFallback = SourcedPriceFallback {
    percent: Percent::whole(25),
    verified_on: "2026-11-02",
    source_url: "https://api.uexcorp.uk/2.0/data_parameters",
};

/// The documented price starting fallback and its independent verification provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcedPriceFallback {
    percent: Percent,
    verified_on: &'static str,
    source_url: &'static str,
}
impl SourcedPriceFallback {
    /// Returns the documented whole-percent baseline.
    #[must_use]
    pub const fn percent(self) -> Percent {
        self.percent
    }
    /// Returns the verification date without implying a fresh cached response.
    #[must_use]
    pub const fn verified_on(self) -> &'static str {
        self.verified_on
    }
    /// Returns the public source of the verified baseline.
    #[must_use]
    pub const fn source_url(self) -> &'static str {
        self.source_url
    }
}

/// Origin of the baseline price tolerance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceToleranceSource {
    /// Validated cached UEX parameters with their actual successful fetch time.
    Uex(Timestamp),
    /// The documented starting fallback, never represented as a cached fetch.
    SettingsFallback(SourcedPriceFallback),
}

/// A source-bound whole-percentage price tolerance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriceTolerance {
    percent: Percent,
    source: PriceToleranceSource,
    override_ignored: bool,
}
impl PriceTolerance {
    pub(crate) fn from_reference(
        reference: Option<(CommodityConstraints, Timestamp)>,
        override_percent: Option<Percent>,
    ) -> Self {
        let (percent, source) = reference.map_or(
            (
                DOCUMENTED_PRICE_FALLBACK.percent,
                PriceToleranceSource::SettingsFallback(DOCUMENTED_PRICE_FALLBACK),
            ),
            |(constraints, fetched_at)| {
                (
                    Percent::whole(constraints.price_variation_percent),
                    PriceToleranceSource::Uex(fetched_at),
                )
            },
        );
        Self {
            percent: override_percent.map_or(percent, |value| {
                Percent::whole(value.get().min(percent.get()))
            }),
            source,
            override_ignored: override_percent.is_some_and(|value| value.get() > percent.get()),
        }
    }
    /// Returns the whole-percentage tolerance used for exact decisions.
    #[must_use]
    pub const fn percent(self) -> Percent {
        self.percent
    }
    /// Returns the baseline source and its distinct verification or fetch provenance.
    #[must_use]
    pub const fn source(self) -> PriceToleranceSource {
        self.source
    }
}

impl PriceTolerance {
    /// Reports an override above the baseline that could not relax the policy.
    #[must_use]
    pub const fn override_ignored(self) -> bool {
        self.override_ignored
    }
}

/// The independent origin of a displayed price reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceReferenceSource {
    /// The latest value of the matching terminal, commodity and side.
    Terminal,
    /// The matching environment's side-specific commodity-wide average.
    CommodityAverage,
}
impl PriceTolerance {
    /// Tests the exact terminal band or the doubled commodity-average band.
    ///
    /// # Errors
    /// Returns precision unavailable rather than rounding or wrapping a comparison.
    pub fn exceeded_by(
        self,
        difference: tallyline_shared_kernel::api::model::Delta,
        source: PriceReferenceSource,
    ) -> Result<bool, tallyline_shared_kernel::api::model::PrecisionUnavailable> {
        let percent = match source {
            PriceReferenceSource::Terminal => self.percent,
            PriceReferenceSource::CommodityAverage => self.percent.doubled()?,
        };
        difference.exceeds_percentage(percent)
    }
}
