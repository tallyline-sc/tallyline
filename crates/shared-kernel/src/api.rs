// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Published shared ports.

pub mod model;

use model::Timestamp;

/// Supplies wall-clock time to application services.
pub trait Clock {
    /// Returns the current time.
    fn now(&self) -> Timestamp;
}
pub use crate::area_sample::sample_area;
pub use crate::projective_map::sample_bilinear;
