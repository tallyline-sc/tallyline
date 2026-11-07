// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offline measurement of frozen aligned observations against verified corpus fields.

use dataset::Dataset;
use serde_json::Value;
use std::path::Path;

mod dataset;
mod metrics;
mod observation;
mod replay_error;

pub use replay_error::ReplayError;

/// Replays every retained single, pair and triple without recognising or modifying inputs.
///
/// # Errors
/// Returns a typed refusal for unavailable inputs, changed hashes or invalid retained evidence.
pub fn analyse(public: &Path, packet: &Path, expected_hash: &str) -> Result<Value, ReplayError> {
    metrics::report(&Dataset::read(public, packet, expected_hash)?)
}
