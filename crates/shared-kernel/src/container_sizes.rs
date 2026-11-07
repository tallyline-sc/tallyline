// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finite Star Citizen cargo-container sizes shared across contexts.

/// A container size of the documented set {1, 2, 4, 8, 16, 24, 32} SCU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContainerSize {
    /// 1 SCU.
    Scu1,
    /// 2 SCU.
    Scu2,
    /// 4 SCU.
    Scu4,
    /// 8 SCU.
    Scu8,
    /// 16 SCU.
    Scu16,
    /// 24 SCU.
    Scu24,
    /// 32 SCU.
    Scu32,
}

impl ContainerSize {
    /// Returns the size for `scu`, `None` outside the documented set.
    #[must_use]
    pub fn from_scu(scu: u32) -> Option<Self> {
        match scu {
            1 => Some(Self::Scu1),
            2 => Some(Self::Scu2),
            4 => Some(Self::Scu4),
            8 => Some(Self::Scu8),
            16 => Some(Self::Scu16),
            24 => Some(Self::Scu24),
            32 => Some(Self::Scu32),
            _ => None,
        }
    }

    /// Returns the size in SCU.
    #[must_use]
    pub fn scu(self) -> u32 {
        match self {
            Self::Scu1 => 1,
            Self::Scu2 => 2,
            Self::Scu4 => 4,
            Self::Scu8 => 8,
            Self::Scu16 => 16,
            Self::Scu24 => 24,
            Self::Scu32 => 32,
        }
    }
}
