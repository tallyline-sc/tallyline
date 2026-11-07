// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Game witness preferences and privacy-safe results.

pub use crate::environment_mapping::{EnvironmentMapping, GameEnvironment};

use std::collections::BTreeSet;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp};

/// The user's explicit permission to read game files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WitnessMode {
    /// Do not access the game's log or manifest.
    #[default]
    Off,
    /// Read only version-validated witness facts.
    On,
}

/// Game-channel mapping and local read-only witness preferences.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameSettings {
    /// Explicit opt-in, disabled by default.
    pub witness: WitnessMode,
    /// Independent UEX target for each launcher channel.
    pub environment_mapping: EnvironmentMapping,
}

/// A read-only witness observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WitnessSnapshot {
    /// The user has not enabled access.
    Off,
    /// No validated facts can be supplied.
    Unavailable(WitnessUnavailable),
    /// Version-validated facts that contain no reported values.
    Available(WitnessFacts),
}

/// A privacy-safe reason that the witness cannot supply facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessUnavailable {
    /// The exact build and branch have no validated format.
    UnvalidatedBuild,
    /// A selected source could not be read; no path or OS message is retained.
    ReadFailure,
    /// The observed payload does not match the validated format.
    UnvalidatedFormat,
    /// No channel directory has been selected.
    NotConfigured,
    /// A source exceeds its bounded input budget.
    InputLimit,
}

/// One sanitised buy-side shop-inventory observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopInventoryFact {
    logged_at: Timestamp,
    terminal: Option<TerminalId>,
    commodity: Option<CommodityId>,
    container_sizes: BTreeSet<u8>,
}
impl ShopInventoryFact {
    /// Constructs an adapter-mapped observation without any raw source text.
    #[must_use]
    pub const fn new(
        logged_at: Timestamp,
        terminal: Option<TerminalId>,
        commodity: Option<CommodityId>,
        container_sizes: BTreeSet<u8>,
    ) -> Self {
        Self {
            logged_at,
            terminal,
            commodity,
            container_sizes,
        }
    }
    /// Returns the UTC log time.
    #[must_use]
    pub const fn logged_at(&self) -> Timestamp {
        self.logged_at
    }
    /// Returns the validated terminal pair, absent for an unsupported mapping.
    #[must_use]
    pub const fn terminal(&self) -> Option<TerminalId> {
        self.terminal
    }
    /// Returns the validated commodity alias, absent for an unsupported mapping.
    #[must_use]
    pub const fn commodity(&self) -> Option<CommodityId> {
        self.commodity
    }
    /// Returns witnessed buy-side sizes, never a reported value.
    #[must_use]
    pub const fn container_sizes(&self) -> &BTreeSet<u8> {
        &self.container_sizes
    }
}

/// Sanitised observations from the validated build and branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WitnessFacts {
    rows: Vec<ShopInventoryFact>,
}
impl WitnessFacts {
    /// Collects version-validated adapter observations.
    #[must_use]
    pub const fn new(rows: Vec<ShopInventoryFact>) -> Self {
        Self { rows }
    }
    /// Returns buy-side observations in source order.
    #[must_use]
    pub fn rows(&self) -> &[ShopInventoryFact] {
        &self.rows
    }
    /// Returns the validated major and minor version without claiming a UEX patch.
    #[must_use]
    pub const fn major_minor(&self) -> (u16, u16) {
        (4, 10)
    }
}
