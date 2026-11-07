// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only SCU assessment with explicitly supplied tolerance bands.

use crate::api::model::{AssessmentUnavailable, Deviation};
use tallyline_reference_data::api::model::{EffectiveStaleness, PriorReference, ScuTolerance};
use tallyline_shared_kernel::api::model::{
    Delta, PrecisionUnavailable, RowId, ScuQuantity, Timestamp,
};

/// A quantity comparison that never implies calibration or Report release eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockAssessment {
    row: RowId,
    reference: Option<ScuQuantity>,
    difference: Option<Delta>,
    tolerance: ScuTolerance,
    reference_age_millis: Option<u128>,
    reference_outdated: bool,
    result: Result<Deviation, AssessmentUnavailable>,
}
impl StockAssessment {
    pub(crate) fn assess(
        row: RowId,
        value: ScuQuantity,
        prior: Option<PriorReference<'_>>,
        tolerance: ScuTolerance,
        now: Timestamp,
        staleness: EffectiveStaleness,
    ) -> Self {
        let reference = prior.map(|prior| prior.values().stock);
        let (difference, result) = match reference {
            None => (None, Ok(Deviation::NoReference)),
            Some(reference) => match value.difference(reference) {
                Err(PrecisionUnavailable) => {
                    (None, Err(AssessmentUnavailable::PrecisionUnavailable))
                }
                Ok(difference) => (
                    Some(difference),
                    tolerance
                        .exceeded_by(difference)
                        .map(|outside| {
                            if outside {
                                Deviation::Major
                            } else if difference.is_zero() {
                                Deviation::Equal
                            } else {
                                Deviation::Minor
                            }
                        })
                        .map_err(|PrecisionUnavailable| {
                            AssessmentUnavailable::PrecisionUnavailable
                        }),
                ),
            },
        };
        let reference_age_millis =
            prior.and_then(|prior| now.millis().checked_sub(prior.modified_at().millis()));
        Self {
            row,
            reference,
            difference,
            tolerance,
            reference_age_millis,
            reference_outdated: reference_age_millis
                .is_some_and(|age| age > u128::from(staleness.days()) * 86_400_000),
            result,
        }
    }
    /// Returns the stable Report row identity.
    #[must_use]
    pub const fn row(self) -> RowId {
        self.row
    }
    /// Returns the matching terminal inventory, preserving a known zero.
    #[must_use]
    pub const fn reference(self) -> Option<ScuQuantity> {
        self.reference
    }
    /// Returns the exact signed SCU difference when representable.
    #[must_use]
    pub const fn difference(self) -> Option<Delta> {
        self.difference
    }
    /// Returns the explicit bands used for this comparison.
    #[must_use]
    pub const fn tolerance(self) -> ScuTolerance {
        self.tolerance
    }
    /// Returns the unlowered deviation when an exact comparison is available.
    #[must_use]
    pub fn deviation(self) -> Option<Deviation> {
        self.result.ok()
    }
    /// Returns the reason an exact comparison is unavailable.
    #[must_use]
    pub fn unavailable(self) -> Option<AssessmentUnavailable> {
        self.result.err()
    }
    /// Returns the terminal age, absent for a missing prior or future timestamp.
    #[must_use]
    pub const fn reference_age_millis(self) -> Option<u128> {
        self.reference_age_millis
    }
    /// Reports whether the terminal age exceeds the effective staleness limit.
    #[must_use]
    pub const fn reference_outdated(self) -> bool {
        self.reference_outdated
    }
    /// Returns presentation severity without lowering the stored comparison level.
    #[must_use]
    pub fn display_deviation(self) -> Option<Deviation> {
        self.deviation().map(|deviation| {
            if self.reference_outdated && deviation == Deviation::Minor {
                Deviation::Equal
            } else {
                deviation
            }
        })
    }
    /// Returns a rounded display percentage, absent for a missing or zero reference.
    ///
    /// # Errors
    /// Returns precision unavailable if checked display arithmetic cannot preserve its intermediates.
    pub fn percentage_difference(self) -> Result<Option<String>, PrecisionUnavailable> {
        self.difference.map_or(Ok(None), Delta::percentage_text)
    }
}
