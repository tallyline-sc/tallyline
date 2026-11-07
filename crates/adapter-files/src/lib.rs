// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Versioned settings, bounded image codecs, owner-only directories and private exports.

#![forbid(unsafe_code)]

pub mod api;
mod config;
mod game_config;
mod game_log;
mod game_log_mapping;
mod game_log_parser;
mod layout;
mod log_time;
mod logo;
mod witness_output;

mod update_config;

mod backup;
mod image_intake;
mod manual_attachment;

mod private_artifact;

mod diagnostic_logs;
mod diagnostics;
mod working_copies;

mod stock_config;

mod png_output;
mod submission_config;
