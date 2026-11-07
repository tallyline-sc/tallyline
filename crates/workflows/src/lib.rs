// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-context application settings and lifecycle workflows.

#![forbid(unsafe_code)]

pub mod api;

mod attachments;
mod diagnostics;
mod submission_priors;

mod submission_request;

mod manual_release;

mod submission_delivery;

mod submission_queue;

mod submission_history;

mod receipt_enrichment;

mod manual_dispatch;

mod event_outbox;

mod cooldown_split;
mod resolution_draft;

mod submission_lifecycle;
