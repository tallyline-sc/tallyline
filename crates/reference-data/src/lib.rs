// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cached reference vocabulary, endpoint freshness and refresh application services.

#![forbid(unsafe_code)]

pub mod api;
mod averages;
mod local_priors;
mod parameters;
mod prior_history;
mod priors;
mod scu_tolerance;
mod snapshot;
mod tolerance;
