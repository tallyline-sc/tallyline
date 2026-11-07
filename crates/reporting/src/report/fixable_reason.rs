// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Report-owned correction actions without remote status identifiers.

/// A concrete reason to correct a returned Draft before renewed release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixableReason {
    /// The account requires confirmed private screenshot evidence.
    ScreenshotRequired,
    /// The observation time must be reviewed with the system clock.
    InvalidObservationTime,
    /// The Report exceeds the accepted row limit.
    TooManyRows,
    /// The terminal must be selected from current reference data.
    TerminalNotFound,
    /// The screenshot encoding must be corrected.
    InvalidScreenshot,
    /// The encoded screenshot exceeds the accepted size.
    ScreenshotTooLarge,
}
