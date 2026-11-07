// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Versioned transcription and credential-free local metadata without inference.

pub mod api;
mod local_metadata_client;
mod metadata_connector;
mod parse_answer;
mod request_contract;
