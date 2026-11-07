// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Windows owned-child platform boundary.

mod child;
mod diagnostics;
mod generation;
mod handles;
mod job;
pub(crate) mod model;
mod owned_process;
mod pipes;
mod startup;

pub use owned_process::OwnedProcess;
