// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact checked differences without rounding a tolerance decision.

use rust_decimal::Decimal;
use std::fmt;

/// A difference that cannot be assessed without losing exact precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrecisionUnavailable;
impl fmt::Display for PrecisionUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("difference cannot be compared without losing precision")
    }
}
impl std::error::Error for PrecisionUnavailable {}

/// A non-negative tolerance in whole percentage points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Percent(u32);
impl Percent {
    /// Creates a whole percentage without a binary floating-point conversion.
    #[must_use]
    pub const fn whole(value: u32) -> Self {
        Self(value)
    }
    /// Returns whole percentage points.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// An exact signed difference with its original reference coefficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delta {
    coefficient: i128,
    scale: u32,
    reference: u128,
}
impl Delta {
    pub(crate) fn calculate(
        value: Decimal,
        reference: Decimal,
    ) -> Result<Self, PrecisionUnavailable> {
        let scale = value.scale().max(reference.scale());
        let value = align(value, scale)?;
        let reference = align(reference, scale)?;
        Ok(Self {
            coefficient: value.checked_sub(reference).ok_or(PrecisionUnavailable)?,
            scale,
            reference: reference.unsigned_abs(),
        })
    }
    /// Reports equality without rounding either value.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.coefficient == 0
    }
    /// Tests a strict percentage boundary with exact checked cross-products.
    ///
    /// # Errors
    /// Returns precision unavailable when either cross-product exceeds the exact coefficient range.
    pub fn exceeds_percentage(self, tolerance: Percent) -> Result<bool, PrecisionUnavailable> {
        let difference = self
            .coefficient
            .unsigned_abs()
            .checked_mul(100)
            .ok_or(PrecisionUnavailable)?;
        let allowance = self
            .reference
            .checked_mul(u128::from(tolerance.get()))
            .ok_or(PrecisionUnavailable)?;
        Ok(difference > allowance)
    }
}
impl fmt::Display for Delta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut digits = self.coefficient.unsigned_abs().to_string();
        if self.scale != 0 {
            let scale = usize::try_from(self.scale).map_err(|_| fmt::Error)?;
            digits = format!("{digits:0>width$}", width = scale + 1);
            digits.insert(digits.len() - scale, '.');
            digits = digits
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned();
        }
        if self.coefficient < 0 {
            formatter.write_str("-")?;
        }
        formatter.write_str(&digits)
    }
}
fn align(value: Decimal, scale: u32) -> Result<i128, PrecisionUnavailable> {
    value
        .mantissa()
        .checked_mul(
            10_i128
                .checked_pow(scale - value.scale())
                .ok_or(PrecisionUnavailable)?,
        )
        .ok_or(PrecisionUnavailable)
}

impl Percent {
    /// Doubles the effective band without rounding or wrapping its value.
    ///
    /// # Errors
    /// Returns precision unavailable when the doubled whole percentage exceeds its range.
    pub fn doubled(self) -> Result<Self, PrecisionUnavailable> {
        self.0.checked_mul(2).map(Self).ok_or(PrecisionUnavailable)
    }
}

impl Delta {
    /// Returns an approximate signed percentage rounded to two places for display only.
    /// Ties round away from zero; a zero reference has no percentage.
    ///
    /// # Errors
    /// Returns precision unavailable when checked percentage arithmetic cannot preserve its intermediates.
    pub fn percentage_text(self) -> Result<Option<String>, PrecisionUnavailable> {
        if self.reference == 0 {
            return Ok(None);
        }
        let numerator = self
            .coefficient
            .unsigned_abs()
            .checked_mul(100)
            .ok_or(PrecisionUnavailable)?;
        let mut whole = numerator / self.reference;
        let scaled_remainder = (numerator % self.reference)
            .checked_mul(100)
            .ok_or(PrecisionUnavailable)?;
        let mut fraction = scaled_remainder / self.reference;
        if (scaled_remainder % self.reference)
            .checked_mul(2)
            .ok_or(PrecisionUnavailable)?
            >= self.reference
        {
            fraction += 1;
        }
        if fraction == 100 {
            whole = whole.checked_add(1).ok_or(PrecisionUnavailable)?;
            fraction = 0;
        }
        let sign = if self.coefficient < 0 {
            "-"
        } else if self.coefficient > 0 {
            "+"
        } else {
            ""
        };
        Ok(Some(format!("{sign}{whole}.{fraction:02}")))
    }
}

impl Delta {
    /// Tests a strict absolute SCU boundary without rounding decimal scales.
    ///
    /// # Errors
    /// Returns precision unavailable when aligning the exact coefficients exceeds their range.
    pub fn exceeds_quantity(
        self,
        minimum: crate::api::model::ScuQuantity,
    ) -> Result<bool, PrecisionUnavailable> {
        let minimum = minimum.decimal();
        let scale = self.scale.max(minimum.scale());
        let difference = self
            .coefficient
            .unsigned_abs()
            .checked_mul(
                10_u128
                    .checked_pow(scale - self.scale)
                    .ok_or(PrecisionUnavailable)?,
            )
            .ok_or(PrecisionUnavailable)?;
        Ok(difference > align(minimum, scale)?.unsigned_abs())
    }
}
