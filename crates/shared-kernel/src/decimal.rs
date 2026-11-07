// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact decimal storage hidden behind domain value types.

use rust_decimal::Decimal;
use std::fmt;
use std::str::FromStr;

/// An unsupported decimal spelling, sign, range or precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDecimal;

impl fmt::Display for InvalidDecimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("value is not a supported non-negative exact decimal")
    }
}
impl std::error::Error for InvalidDecimal {}

/// A non-negative exact price in aUEC per SCU, independent of third-party types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PricePerScu(Decimal);

impl FromStr for PricePerScu {
    type Err = InvalidDecimal;
    fn from_str(text: &str) -> Result<Self, InvalidDecimal> {
        exact(text).map(Self)
    }
}
impl fmt::Display for PricePerScu {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}
impl PricePerScu {
    /// Reports zero without converting to binary floating point.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }
}

/// A non-negative exact SCU quantity, distinct from a unit price.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScuQuantity(Decimal);

impl FromStr for ScuQuantity {
    type Err = InvalidDecimal;
    fn from_str(text: &str) -> Result<Self, InvalidDecimal> {
        exact(text).map(Self)
    }
}
impl fmt::Display for ScuQuantity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}
impl ScuQuantity {
    /// Returns an exact whole quantity when it fits the supported unsigned wire range.
    #[must_use]
    pub fn whole_units(self) -> Option<u64> {
        if self.0.scale() == 0 {
            u64::try_from(self.0.mantissa()).ok()
        } else {
            None
        }
    }
    /// Constructs an exact non-negative whole quantity.
    #[must_use]
    pub const fn whole(value: u32) -> Self {
        Self(Decimal::from_parts(value, 0, 0, false, 0))
    }
    /// Reports zero without confusing it with a missing observation.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }
}

fn exact(text: &str) -> Result<Decimal, InvalidDecimal> {
    if text
        .bytes()
        .any(|byte| !byte.is_ascii_digit() && !matches!(byte, b'.' | b'+' | b'-' | b'e' | b'E'))
    {
        return Err(InvalidDecimal);
    }
    let value = if let Some((base, exponent)) = text.split_once(['e', 'E']) {
        let base = Decimal::from_str_exact(base)
            .map_err(|_| InvalidDecimal)?
            .normalize();
        Decimal::from_scientific(&format!("{base}e{exponent}"))
    } else {
        Decimal::from_str_exact(text)
    }
    .map_err(|_| InvalidDecimal)?;
    if value < Decimal::ZERO {
        Err(InvalidDecimal)
    } else {
        Ok(value.normalize())
    }
}

impl PricePerScu {
    /// Calculates an exact signed difference while preserving both accepted input values.
    ///
    /// # Errors
    /// Returns precision unavailable when the coefficients cannot be aligned exactly.
    pub fn difference(
        self,
        reference: Self,
    ) -> Result<crate::api::model::Delta, crate::api::model::PrecisionUnavailable> {
        crate::api::model::Delta::calculate(self.0, reference.0)
    }
}

impl ScuQuantity {
    /// Calculates an exact signed SCU difference without narrowing either accepted input.
    ///
    /// # Errors
    /// Returns precision unavailable when the coefficients cannot be aligned exactly.
    pub fn difference(
        self,
        reference: Self,
    ) -> Result<crate::api::model::Delta, crate::api::model::PrecisionUnavailable> {
        crate::api::model::Delta::calculate(self.0, reference.0)
    }
    pub(crate) const fn decimal(self) -> Decimal {
        self.0
    }
}
