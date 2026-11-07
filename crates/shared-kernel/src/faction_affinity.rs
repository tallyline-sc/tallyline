// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicitly entered faction affinity within the UEX report range.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// A player's explicitly entered affinity with the terminal's faction, from -100 to 100.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FactionAffinity(i8);

/// An affinity that is not a whole number within -100 to 100.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidFactionAffinity;

impl Display for InvalidFactionAffinity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("faction affinity must be an integer from -100 to 100")
    }
}
impl Error for InvalidFactionAffinity {}

impl FactionAffinity {
    /// Validates an explicitly supplied integer within the inclusive UEX range.
    ///
    /// # Errors
    /// Refuses values below -100 or above 100.
    pub const fn new(value: i8) -> Result<Self, InvalidFactionAffinity> {
        if value < -100 || value > 100 {
            return Err(InvalidFactionAffinity);
        }
        Ok(Self(value))
    }
    /// Returns the entered integer, preserving an explicit zero.
    #[must_use]
    pub const fn value(self) -> i8 {
        self.0
    }
}
impl FromStr for FactionAffinity {
    type Err = InvalidFactionAffinity;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input.parse().map_err(|_| InvalidFactionAffinity)?)
    }
}
impl Display for FactionAffinity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
