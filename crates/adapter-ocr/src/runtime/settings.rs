// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit per-session CPU and idle resource bounds.

use std::time::Duration;

/// Validated session limits; sequential execution and disabled spinning are fixed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OcrRuntimeSettings {
    intra_op_threads: usize,
    idle_close_after: Duration,
}

/// A zero idle duration or thread count outside the native positive signed-int range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidOcrRuntimeSettings;

impl OcrRuntimeSettings {
    /// Validates positive native thread and idle bounds.
    ///
    /// # Errors
    /// Refuses zero values and thread counts greater than `i32::MAX`.
    pub fn new(
        intra_op_threads: usize,
        idle_close_after: Duration,
    ) -> Result<Self, InvalidOcrRuntimeSettings> {
        if intra_op_threads == 0
            || i32::try_from(intra_op_threads).is_err()
            || idle_close_after.is_zero()
        {
            return Err(InvalidOcrRuntimeSettings);
        }
        Ok(Self {
            intra_op_threads,
            idle_close_after,
        })
    }

    /// Returns the explicit per-session native CPU bound.
    #[must_use]
    pub const fn intra_op_threads(self) -> usize {
        self.intra_op_threads
    }

    /// Returns the inactivity interval after which worker sessions are released.
    #[must_use]
    pub const fn idle_close_after(self) -> Duration {
        self.idle_close_after
    }
}

impl Default for OcrRuntimeSettings {
    fn default() -> Self {
        Self {
            intra_op_threads: 1,
            idle_close_after: Duration::from_secs(300),
        }
    }
}
