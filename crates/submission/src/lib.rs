// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Account validation and secure credential persistence for submission.
//!
//! Secret-bearing public values cannot be debug-logged (R-NF-4).
//!
//! ```compile_fail,E0277
//! use tallyline_submission::api::model::ReleaseToken;
//! fn log(value: &ReleaseToken) { let _ = format!("{value:?}"); }
//! ```
//!
//! ```compile_fail,E0277
//! use tallyline_submission::api::model::CredentialValues;
//! fn log(value: &CredentialValues) { let _ = format!("{value:?}"); }
//! ```
//!
//! ```compile_fail,E0277
//! use tallyline_submission::api::model::CredentialCandidate;
//! fn log(value: &CredentialCandidate) { let _ = format!("{value:?}"); }
//! ```
//!
//! ```compile_fail,E0277
//! use tallyline_submission::api::model::SavedCredentials;
//! fn log(value: &SavedCredentials) { let _ = format!("{value:?}"); }
//! ```
//!
//! ```compile_fail,E0277
//! use tallyline_submission::api::ValidationRequest;
//! fn log(value: &ValidationRequest) { let _ = format!("{value:?}"); }
//! ```

#![forbid(unsafe_code)]

pub mod api;

mod screenshot_policy;

mod application;
mod budget;
mod events;
mod job;
mod preflight;
mod receipt;
mod request;
mod schedule;
mod server_time;

mod receipt_mapping;

mod observation_order;

mod duplicate_evidence;
mod queue_wait;
mod retry;
mod service_fence;
mod uex_error_code;

mod submission_account_evidence;
mod submission_rejection;

mod submission_resolution_action;

mod submission_row_result;

mod row_failure_source;

mod token_scope;

mod token_quota_reset;

mod token_quota_observation;

mod token_admission;

mod submission_lifecycle;
mod uex_lifecycle_observation;

mod submission_lifecycle_settings;

mod lifecycle_read_batch;

mod lifecycle_read_response;
