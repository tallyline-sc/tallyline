// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application composition and coordinated local-storage bootstrap.

#![forbid(unsafe_code)]

pub mod api;
mod attachment_worker;
mod cpu_worker;
mod executor;
mod manual_worker;
mod reference_worker;
mod storage_bootstrap;

mod manual_attachments;

mod credential_worker;

mod credential_logs;
mod manual_reviews;

mod build_info;

mod build_entry;

mod dependency_diagnostics;
mod http_client;
mod http_driver;
mod http_protocol;

mod receipt_lookup;

mod response_worker;

mod submission_slot;

mod credential_queue;

mod submission_controller;

mod submission_bridge;

mod release_reference;

mod native_icon;

mod http_quota;
