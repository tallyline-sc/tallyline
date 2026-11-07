// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The UEX API adapter: a read client and `data_submit` dry-run client for UEX API 2.0.

#![forbid(unsafe_code)]

pub mod api;
#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "tagged test names use a double underscore (11 B4)"
)]
mod contract_tests;
#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod credential_contract_tests;
mod credentials;
mod envelope;
mod parameter_wire;
mod prior_wire;
mod public_contract;
mod reference_fetcher;
mod reference_wire;
mod transport;
mod wire;

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod terminal_contract_tests;

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod proxy_contract_tests;
#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod status_contract_tests;

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod parameter_contract_tests;

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod prior_contract_tests;

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod public_contract_tests;

#[cfg(test)]
mod queue_contract_tests;
mod submission;
mod submission_account_recheck;
mod submission_code;
mod submission_refusal;

mod http_date;
mod server_time;

mod response_processing;

#[cfg(test)]
mod processing_contract_tests;

mod verified_row_receipts;

mod receipt_window;

mod receipt_reads;

mod quota_observations;

mod authenticated_read_gate;

#[cfg(test)]
mod quota_post_contract_tests;

mod lifecycle_reads;
mod lifecycle_wire;
