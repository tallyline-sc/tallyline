// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact price deviations independent of user-entered values.

use crate::api::model::Deviation;
use tallyline_reference_data::api::model::{
    EffectiveStaleness, PriceReferenceSource, PriceTolerance, PriorReference,
};
use tallyline_shared_kernel::api::model::{
    Delta, PrecisionUnavailable, PricePerScu, RowId, Timestamp,
};

/// A closed reason why a numeric deviation cannot be assessed safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssessmentUnavailable {
    /// Exact coefficient arithmetic cannot represent this comparison.
    PrecisionUnavailable,
}

/// Read-only deviation and exact difference of one entered price.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriceAssessment {
    row: RowId,
    reference: Option<PriceReference>,
    difference: Option<Delta>,
    tolerance: PriceTolerance,
    reference_age_millis: Option<u128>,
    reference_outdated: bool,
    result: Result<Deviation, AssessmentUnavailable>,
}
impl PriceAssessment {
    pub(crate) fn assess(
        row: RowId,
        value: PricePerScu,
        reference: Option<PriceReference>,
        tolerance: PriceTolerance,
        now: Timestamp,
        staleness: EffectiveStaleness,
    ) -> Self {
        let (difference, result) = match reference {
            None => (None, Ok(Deviation::NoReference)),
            Some(reference) => match value.difference(reference.value) {
                Err(PrecisionUnavailable) => {
                    (None, Err(AssessmentUnavailable::PrecisionUnavailable))
                }
                Ok(difference) => (
                    Some(difference),
                    classify(difference, tolerance, reference.source),
                ),
            },
        };
        let reference_age_millis = reference.and_then(|reference| {
            reference
                .modified_at
                .and_then(|at| now.millis().checked_sub(at.millis()))
        });
        Self {
            row,
            reference,
            difference,
            tolerance,
            result,
            reference_age_millis,
            reference_outdated: reference_age_millis
                .is_some_and(|age| age > u128::from(staleness.days()) * 86_400_000),
        }
    }
    /// Returns the stable row identity.
    #[must_use]
    pub const fn row(self) -> RowId {
        self.row
    }
    /// Returns the comparable reference price without changing the entered value.
    #[must_use]
    pub fn reference(self) -> Option<PricePerScu> {
        self.reference.map(|reference| reference.value)
    }
    /// Returns the exact signed unit difference when representable.
    #[must_use]
    pub const fn difference(self) -> Option<Delta> {
        self.difference
    }
    /// Returns the shared source-bound policy used for this assessment.
    #[must_use]
    pub const fn tolerance(self) -> PriceTolerance {
        self.tolerance
    }
    /// Returns the unlowered deviation only after an exact comparison.
    #[must_use]
    pub fn deviation(self) -> Option<Deviation> {
        self.result.ok()
    }
    /// Returns the reason that prevents a safe numeric assessment.
    #[must_use]
    pub fn unavailable(self) -> Option<AssessmentUnavailable> {
        self.result.err()
    }
}
fn classify(
    difference: Delta,
    tolerance: PriceTolerance,
    source: PriceReferenceSource,
) -> Result<Deviation, AssessmentUnavailable> {
    tolerance
        .exceeded_by(difference, source)
        .map(|outside| {
            if outside {
                Deviation::Major
            } else if source == PriceReferenceSource::CommodityAverage {
                Deviation::NoReference
            } else if difference.is_zero() {
                Deviation::Equal
            } else {
                Deviation::Minor
            }
        })
        .map_err(|PrecisionUnavailable| AssessmentUnavailable::PrecisionUnavailable)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PriceReference {
    value: PricePerScu,
    modified_at: Option<Timestamp>,
    source: PriceReferenceSource,
}
impl PriceReference {
    pub(crate) fn terminal(prior: PriorReference<'_>) -> Self {
        Self {
            value: prior.values().price,
            modified_at: Some(prior.modified_at()),
            source: PriceReferenceSource::Terminal,
        }
    }
}
impl PriceAssessment {
    /// Returns terminal prior age, absent for an average, missing prior or future timestamp.
    #[must_use]
    pub const fn reference_age_millis(self) -> Option<u128> {
        self.reference_age_millis
    }
    /// Reports whether the terminal reference exceeds the effective staleness limit.
    #[must_use]
    pub const fn reference_outdated(self) -> bool {
        self.reference_outdated
    }
    /// Returns presentation severity while preserving unlowered gate severity.
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
}

impl PriceReference {
    pub(crate) fn average(value: PricePerScu) -> Self {
        Self {
            value,
            modified_at: None,
            source: PriceReferenceSource::CommodityAverage,
        }
    }
}
impl PriceAssessment {
    /// Returns the reference origin without treating an average as a terminal observation.
    #[must_use]
    pub fn reference_source(self) -> Option<PriceReferenceSource> {
        self.reference.map(|reference| reference.source)
    }
}

impl PriceAssessment {
    /// Returns an approximate percentage for display without using it to decide deviation.
    ///
    /// # Errors
    /// Returns precision unavailable if checked percentage display arithmetic cannot preserve its intermediates.
    pub fn percentage_difference(self) -> Result<Option<String>, PrecisionUnavailable> {
        self.difference.map_or(Ok(None), Delta::percentage_text)
    }
}
