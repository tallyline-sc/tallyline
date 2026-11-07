// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Identification provenance for an immutable named failure position.

/// The evidence used to identify the failing released row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowFailureSource {
    /// No failing row could be identified.
    Unmapped,
    /// UEX supplied an in-range zero-based row index.
    Index,
    /// UEX supplied the exact terminal and commodity of this released row.
    Commodity,
    /// A stored row position has no recorded identification source.
    Unspecified,
}
