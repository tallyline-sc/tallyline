// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! SQLite persistence, consistent backups, forward migrations and reference repositories.

#![forbid(unsafe_code)]

pub mod api;
mod backup;
mod migration;
mod parameter_document;
mod prior_document;
mod reference_cache;
mod reports;
mod status_document;

mod screenshot_policy;

mod report_fixable_reason;
mod report_lifecycle;

mod error_code_document;
mod legacy_row_failure;
mod queue_client_version;
mod queue_document;
mod queue_outcome_document;
mod submission_service_fences;

mod queue;

mod accepted_priors;

mod receipt_enrichment;

mod manual_dispatch;

mod event_outbox;

mod reference_consistency;

mod queue_account_evidence;

mod resolution_action_document;

mod cooldown_split;
mod resolution_draft;

mod report_history;

mod token_quota;

mod authenticated_quota;

mod lifecycle_document;
mod submission_lifecycle;

mod lifecycle_record;
mod lifecycle_source;

mod lifecycle_binding;
mod submission_request_document;
