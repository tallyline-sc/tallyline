// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offline corpus inspection, annotation proposals and retained-reader measurement.

#![forbid(unsafe_code)]

pub mod corpus;

pub mod panel_dump;
pub mod reader_replay;
#[cfg(test)]
mod scratch;
pub mod scu_policy;
