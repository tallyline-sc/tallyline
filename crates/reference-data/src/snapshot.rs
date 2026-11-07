// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned environment-specific read state independent of cache workers.

use crate::api::model::{
    CachedReference, Commodity, CommodityAverages, Endpoint, EndpointData, GameVersions,
    ReferenceState, StatusLevels, Terminal, UexEnvironment,
};
use tallyline_shared_kernel::api::model::{Timestamp, TradeSide};

/// Immutable reference data for one mapped UEX environment and one decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceSnapshot {
    environment: UexEnvironment,
    state: ReferenceState,
    entries: Vec<CachedReference>,
    local_priors: Vec<crate::api::model::AcceptedLocalPrior>,
}

impl ReferenceSnapshot {
    pub(crate) fn new(
        environment: UexEnvironment,
        state: ReferenceState,
        entries: &[CachedReference],
    ) -> Self {
        let entries = entries
            .iter()
            .filter(|entry| {
                environment != UexEnvironment::Ptu
                    || !matches!(entry.data(), EndpointData::TerminalPriors(_))
            })
            .map(|entry| match entry.data() {
                EndpointData::Commodities(items) => CachedReference::new(
                    entry.fetched_at(),
                    EndpointData::Commodities(
                        items
                            .iter()
                            .filter(|item| item.available(environment))
                            .map(|item| {
                                Commodity::new(
                                    item.id(),
                                    item.name(),
                                    item.code(),
                                    item.available(UexEnvironment::Live),
                                    CommodityAverages::new(
                                        item.average_price(environment, TradeSide::Buy),
                                        item.average_price(environment, TradeSide::Sell),
                                    ),
                                )
                            })
                            .collect(),
                    ),
                ),
                EndpointData::Terminals(_)
                | EndpointData::GameVersions(_)
                | EndpointData::StatusLevels(_)
                | EndpointData::Parameters(_)
                | EndpointData::TerminalPriors(_) => entry.clone(),
            })
            .collect();
        Self {
            environment,
            state,
            entries,
            local_priors: Vec::new(),
        }
    }

    /// Compares current persisted UEX references with this snapshot's exact projected data.
    ///
    /// Successful refreshes change the comparison even when their values are equal.
    /// Local accepted-row projections remain a separate history/version comparison.
    #[must_use]
    pub fn matches_cached_references(&self, entries: &[CachedReference]) -> bool {
        let current = Self::new(self.environment, self.state, entries);
        self.entries.len() == current.entries.len()
            && self
                .entries
                .iter()
                .all(|entry| current.entries.contains(entry))
    }

    /// Returns the mapped UEX environment, independent of the launcher's channel name.
    #[must_use]
    pub const fn environment(&self) -> UexEnvironment {
        self.environment
    }

    /// Returns matching commodities with no LIVE average retained in a PTU snapshot.
    #[must_use]
    pub fn commodities(&self, query: &str) -> Vec<&Commodity> {
        let query = query.to_lowercase();
        self.entries
            .iter()
            .flat_map(|entry| match entry.data() {
                EndpointData::Commodities(items) => items.as_slice(),
                EndpointData::Terminals(_)
                | EndpointData::GameVersions(_)
                | EndpointData::StatusLevels(_)
                | EndpointData::Parameters(_)
                | EndpointData::TerminalPriors(_) => &[],
            })
            .filter(|item| {
                item.name().to_lowercase().contains(&query)
                    || item.code().to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Searches selectable terminal labels and location context for this environment.
    #[must_use]
    pub fn terminals(&self, query: &str) -> Vec<&Terminal> {
        let query = query.to_lowercase();
        self.entries
            .iter()
            .flat_map(|entry| match entry.data() {
                EndpointData::Terminals(items) => items.as_slice(),
                EndpointData::Commodities(_)
                | EndpointData::GameVersions(_)
                | EndpointData::StatusLevels(_)
                | EndpointData::Parameters(_)
                | EndpointData::TerminalPriors(_) => &[],
            })
            .filter(|terminal| {
                let text = terminal.text();
                terminal.available(self.environment)
                    && [
                        &text.name,
                        &text.nickname,
                        &text.display_name,
                        &text.location,
                    ]
                    .iter()
                    .any(|field| fuzzy_match(field, &query))
            })
            .collect()
    }

    /// Returns cached version read data, never a fresh network observation.
    #[must_use]
    pub fn game_versions(&self) -> Option<&GameVersions> {
        self.entries.iter().find_map(|entry| match entry.data() {
            EndpointData::GameVersions(versions) => Some(versions),
            EndpointData::Commodities(_)
            | EndpointData::Terminals(_)
            | EndpointData::StatusLevels(_)
            | EndpointData::Parameters(_)
            | EndpointData::TerminalPriors(_) => None,
        })
    }

    /// Returns the cached per-side status vocabulary.
    #[must_use]
    pub fn status_levels(&self) -> Option<&StatusLevels> {
        self.entries.iter().find_map(|entry| match entry.data() {
            EndpointData::StatusLevels(levels) => Some(levels),
            EndpointData::Commodities(_)
            | EndpointData::Terminals(_)
            | EndpointData::GameVersions(_)
            | EndpointData::Parameters(_)
            | EndpointData::TerminalPriors(_) => None,
        })
    }
    /// Returns cached submission policy, tolerances and capped literal staff text.
    #[must_use]
    pub fn parameters(&self) -> Option<&crate::api::model::ReferenceParameters> {
        self.entries.iter().find_map(|entry| match entry.data() {
            EndpointData::Parameters(parameters) => Some(parameters),
            EndpointData::Commodities(_)
            | EndpointData::Terminals(_)
            | EndpointData::GameVersions(_)
            | EndpointData::StatusLevels(_)
            | EndpointData::TerminalPriors(_) => None,
        })
    }

    /// Returns the LIVE assortment for one terminal, with no LIVE prior retained in PTU.
    #[must_use]
    pub fn terminal_priors(
        &self,
        terminal: tallyline_shared_kernel::api::model::TerminalId,
    ) -> Option<&crate::api::model::TerminalPriors> {
        self.entries.iter().find_map(|entry| match entry.data() {
            EndpointData::TerminalPriors(priors) if priors.terminal() == terminal => Some(priors),
            EndpointData::Commodities(_)
            | EndpointData::Terminals(_)
            | EndpointData::GameVersions(_)
            | EndpointData::StatusLevels(_)
            | EndpointData::Parameters(_)
            | EndpointData::TerminalPriors(_) => None,
        })
    }

    /// Returns one endpoint's successful fetch time without creating an observation.
    #[must_use]
    pub fn fetched_at(&self, endpoint: Endpoint) -> Option<Timestamp> {
        self.entries
            .iter()
            .find(|entry| entry.endpoint() == endpoint)
            .map(CachedReference::fetched_at)
    }

    /// Returns the connection state captured with these records.
    #[must_use]
    pub const fn state(&self) -> ReferenceState {
        self.state
    }
}

fn fuzzy_match(field: &str, query: &str) -> bool {
    let lowered = field.to_lowercase();
    let mut characters = lowered.chars();
    query
        .chars()
        .filter(|character| !character.is_whitespace())
        .all(|expected| characters.any(|actual| actual == expected))
}

impl ReferenceSnapshot {
    /// Computes the shared prior staleness limit with a raise-only optional override.
    #[must_use]
    pub fn effective_staleness(
        &self,
        override_days: Option<std::num::NonZeroU32>,
    ) -> crate::api::model::EffectiveStaleness {
        crate::api::model::EffectiveStaleness::from_constraints(
            self.parameters()
                .map(crate::api::model::ReferenceParameters::constraints),
            override_days,
        )
    }
}

impl ReferenceSnapshot {
    /// Returns the shared source-bound price policy for this immutable snapshot.
    #[must_use]
    pub fn effective_price_tolerance(
        &self,
        override_percent: Option<tallyline_shared_kernel::api::model::Percent>,
    ) -> crate::api::model::PriceTolerance {
        let reference = self.entries.iter().find_map(|entry| match entry.data() {
            EndpointData::Parameters(parameters) => {
                Some((parameters.constraints(), entry.fetched_at()))
            }
            EndpointData::Commodities(_)
            | EndpointData::Terminals(_)
            | EndpointData::GameVersions(_)
            | EndpointData::StatusLevels(_)
            | EndpointData::TerminalPriors(_) => None,
        });
        crate::api::model::PriceTolerance::from_reference(reference, override_percent)
    }
}

impl ReferenceSnapshot {
    /// Attaches the current active local projection, never promoting any PTU record to a price reference.
    ///
    /// Persistence must discard an environment's active projection on its next successful reference
    /// snapshot refresh. Historical accepted records remain separate from this active projection.
    ///
    /// # Errors
    /// Refuses duplicate active LIVE row keys rather than choosing an arbitrary accepted record.
    pub fn with_local_priors(
        mut self,
        records: &[crate::api::model::AcceptedLocalPrior],
    ) -> Result<Self, crate::api::model::InvalidPrior> {
        self.local_priors.clear();
        if self.environment == UexEnvironment::Live {
            let mut keys = std::collections::BTreeSet::new();
            for record in records
                .iter()
                .filter(|record| record.context().environment == UexEnvironment::Live)
            {
                let context = record.context();
                let side = match context.side {
                    TradeSide::Buy => 0,
                    TradeSide::Sell => 1,
                };
                if !keys.insert((context.terminal, record.commodity(), side)) {
                    return Err(crate::api::model::InvalidPrior);
                }
                self.local_priors.push(record.clone());
            }
        }
        Ok(self)
    }
    /// Selects an active LIVE local acceptance or the exact matching UEX row, preserving source provenance.
    #[must_use]
    pub fn prior_reference(
        &self,
        terminal: tallyline_shared_kernel::api::model::TerminalId,
        commodity: tallyline_shared_kernel::api::model::CommodityId,
        side: TradeSide,
    ) -> Option<crate::api::model::PriorReference<'_>> {
        use crate::api::model::PriorReference;
        if self.environment == UexEnvironment::Ptu {
            return None;
        }
        self.local_priors
            .iter()
            .find(|record| {
                record.context().terminal == terminal
                    && record.context().side == side
                    && record.commodity() == commodity
            })
            .map(PriorReference::AcceptedLocal)
            .or_else(|| {
                self.terminal_priors(terminal)?
                    .rows()
                    .iter()
                    .find(|row| row.commodity() == commodity && row.side() == side)
                    .map(PriorReference::Uex)
            })
    }
}
