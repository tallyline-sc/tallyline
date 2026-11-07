// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Independent assessment of explicitly entered inventory status.

use std::num::NonZeroU8;
use tallyline_shared_kernel::api::model::RowId;

/// A field's unlowered difference from its reference value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deviation {
    /// The value equals its reference.
    Equal,
    /// The value differs within the effective tolerance.
    Minor,
    /// The value differs beyond the effective tolerance.
    Major,
    /// No comparable reference exists.
    NoReference,
}

/// Assessment of one explicitly entered inventory status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusAssessment {
    row: RowId,
    reference: Option<NonZeroU8>,
    deviation: Deviation,
    reference_age_millis: Option<u128>,
    reference_outdated: bool,
}
impl StatusAssessment {
    pub(crate) fn assess(
        row: RowId,
        value: NonZeroU8,
        reference: Option<NonZeroU8>,
        reference_age_millis: Option<u128>,
        staleness_days: u32,
    ) -> Self {
        let deviation = match reference.map(|prior| value.get().abs_diff(prior.get())) {
            None => Deviation::NoReference,
            Some(0) => Deviation::Equal,
            Some(1) => Deviation::Minor,
            Some(_) => Deviation::Major,
        };
        Self {
            row,
            reference,
            deviation,
            reference_age_millis,
            reference_outdated: reference_age_millis
                .is_some_and(|age| age > u128::from(staleness_days) * 86_400_000),
        }
    }
    /// Returns the stable assessed row identity.
    #[must_use]
    pub const fn row(self) -> RowId {
        self.row
    }
    /// Returns the comparable terminal status, absent when unknown.
    #[must_use]
    pub const fn reference(self) -> Option<NonZeroU8> {
        self.reference
    }
    /// Returns the unlowered deviation used by the release gate.
    #[must_use]
    pub const fn deviation(self) -> Deviation {
        self.deviation
    }
    /// Returns the prior age in milliseconds, absent when no comparable reference or age exists.
    #[must_use]
    pub const fn reference_age_millis(self) -> Option<u128> {
        self.reference_age_millis
    }
    /// Reports whether the comparable reference exceeds the effective staleness limit.
    #[must_use]
    pub const fn reference_outdated(self) -> bool {
        self.reference_outdated
    }
    /// Returns presentation severity without weakening the unlowered deviation.
    #[must_use]
    pub fn display_deviation(self) -> Deviation {
        if self.reference_outdated && self.deviation == Deviation::Minor {
            Deviation::Equal
        } else {
            self.deviation
        }
    }
}

impl Deviation {
    /// Reports whether this unlowered level invalidates approval at the previous level.
    #[must_use]
    pub const fn is_worse_than(self, previous: Self) -> bool {
        matches!(
            (previous, self),
            (Self::Equal | Self::NoReference, Self::Minor | Self::Major)
                | (Self::Minor, Self::Major)
        )
    }
}
