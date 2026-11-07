// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed semantic tone names for message surfaces.

/// A message tone backed by a complete theme token family.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SemanticTone {
    /// Ordinary informational state.
    #[default]
    Neutral,
    /// A relevant informational notice.
    Info,
    /// A successful operation.
    Success,
    /// A caution requiring attention.
    Caution,
    /// A critical failure.
    Critical,
}

impl SemanticTone {
    pub(crate) const fn tokens(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Neutral => ("neutral-fg", "neutral-bg", "neutral-border"),
            Self::Info => ("info-fg", "info-bg", "info-border"),
            Self::Success => ("success-fg", "success-bg", "success-border"),
            Self::Caution => ("caution-fg", "caution-bg", "caution-border"),
            Self::Critical => ("critical-fg", "critical-bg", "critical-border"),
        }
    }
}
