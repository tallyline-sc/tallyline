// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public queue facade with private leaf implementations.

mod decision_control;
mod dispatch_preparation;
mod dispatch_state;
mod failure_diagnostics;
mod models;
mod queue_state;
mod receipt_enrichment;
mod refusal_recovery;
mod release_control;
mod resources;
mod submission_queue_controller;

pub use models::{SubmissionControlError, SubmissionQueuePreferences};
pub use submission_queue_controller::SubmissionQueueController;

mod receipt_resolution;

mod lifecycle_reads;
