// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference cache loading, searching and refresh commands.

pub mod model;

use model::{
    CachedReference, Commodity, Endpoint, EndpointData, GameVersions, ReferenceDataRefreshed,
    ReferenceSnapshot, ReferenceState, StatusLevels, Terminal, UexEnvironment,
};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;

/// Fetches one typed public response without owning application workers or clocks.
pub trait ReferenceDataFetcher {
    /// The transport or boundary-mapping failure.
    type Error;

    /// Requests one endpoint without exposing its wire representation.
    fn fetch(
        &self,
        endpoint: Endpoint,
    ) -> impl std::future::Future<Output = Result<EndpointData, Self::Error>>;
    /// Fetches one to ten distinct positive terminal identities as LIVE assortments.
    fn fetch_terminal_priors(
        &self,
        terminals: &[tallyline_shared_kernel::api::model::TerminalId],
    ) -> impl std::future::Future<Output = Result<Vec<model::TerminalPriors>, Self::Error>>;
}
/// Loads and atomically replaces successful reference responses.
pub trait ReferenceDataCache {
    /// The persistence failure type.
    type Error;

    /// Loads persisted responses without making a new network observation.
    ///
    /// # Errors
    /// Returns the storage or migration failure.
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error>;

    /// Replaces one response and its successful fetch time atomically.
    ///
    /// # Errors
    /// Returns a persistence error while preserving the previous response.
    fn replace(&mut self, entry: &CachedReference) -> Result<(), Self::Error>;
}

/// Serves cached reference data independently of background network work.
pub struct ReferenceApplication<S, C> {
    store: S,
    clock: C,
    entries: Vec<CachedReference>,
    state: ReferenceState,
    failed: Vec<Endpoint>,
}

impl<S: ReferenceDataCache, C: Clock> ReferenceApplication<S, C> {
    /// Loads cached data before any network work begins.
    ///
    /// # Errors
    /// Returns the cache's read or migration failure.
    pub fn load(mut store: S, clock: C) -> Result<Self, S::Error> {
        let entries = store.load()?;
        let state = if entries.is_empty() {
            ReferenceState::NoReferenceData
        } else {
            ReferenceState::Cached
        };
        Ok(Self {
            store,
            clock,
            entries,
            state,
            failed: Vec::new(),
        })
    }

    /// Searches names and non-unique codes without collapsing identities.
    #[must_use]
    pub fn commodities(&self, environment: UexEnvironment, query: &str) -> Vec<&Commodity> {
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
                item.available(environment)
                    && (item.name().to_lowercase().contains(&query)
                        || item.code().to_lowercase().contains(&query))
            })
            .collect()
    }

    /// Searches terminal labels and location context without merging identities.
    #[must_use]
    pub fn terminals(&self, environment: UexEnvironment, query: &str) -> Vec<&Terminal> {
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
                terminal.available(environment)
                    && [
                        &text.name,
                        &text.nickname,
                        &text.display_name,
                        &text.location,
                    ]
                    .iter()
                    .any(|field| field.to_lowercase().contains(&query))
            })
            .collect()
    }
    /// Returns cached versions without creating a version-history observation.
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
    /// Returns the cached status vocabulary with distinct buy and sell labels.
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
    /// Returns endpoints with expired cache data or a failed last refresh.
    #[must_use]
    pub fn due(&self) -> Vec<Endpoint> {
        let mut due: Vec<_> = [
            Endpoint::Commodities,
            Endpoint::Terminals,
            Endpoint::GameVersions,
            Endpoint::StatusLevels,
            Endpoint::Parameters,
        ]
        .into_iter()
        .filter(|endpoint| self.expired(*endpoint, endpoint.ttl_seconds()))
        .collect();
        for endpoint in &self.failed {
            if !due.contains(endpoint) {
                due.push(*endpoint);
            }
        }
        due
    }

    /// Reports whether a requested terminal needs its own fresh LIVE assortment.
    #[must_use]
    pub fn terminal_priors_due(
        &self,
        terminal: tallyline_shared_kernel::api::model::TerminalId,
    ) -> bool {
        let endpoint = Endpoint::TerminalPriors(terminal);
        self.failed.contains(&endpoint) || self.expired(endpoint, endpoint.ttl_seconds())
    }

    /// Returns send-gate endpoints that need a new successful observation.
    #[must_use]
    pub fn due_before_send(&self) -> Vec<Endpoint> {
        [Endpoint::GameVersions, Endpoint::Parameters]
            .into_iter()
            .filter(|endpoint| self.failed.contains(endpoint) || self.expired(*endpoint, 900))
            .collect()
    }

    fn expired(&self, endpoint: Endpoint, ttl_seconds: u64) -> bool {
        self.fetched_at(endpoint).is_none_or(|at| {
            self.clock
                .now()
                .millis()
                .checked_sub(at.millis())
                .is_none_or(|age| age >= u128::from(ttl_seconds) * 1_000)
        })
    }
    /// Publishes a successful network response only after persistence succeeds.
    ///
    /// # Errors
    /// Returns the storage failure without replacing active data or fetch time.
    pub fn refreshed(&mut self, data: EndpointData) -> Result<ReferenceDataRefreshed, S::Error> {
        let entry = CachedReference::new(self.clock.now(), data);
        self.store.replace(&entry)?;
        let event = ReferenceDataRefreshed {
            endpoint: entry.endpoint(),
            fetched_at: entry.fetched_at(),
            game_versions: match entry.data() {
                EndpointData::GameVersions(versions) => Some(versions.clone()),
                EndpointData::Commodities(_)
                | EndpointData::Terminals(_)
                | EndpointData::StatusLevels(_)
                | EndpointData::Parameters(_)
                | EndpointData::TerminalPriors(_) => None,
            },
        };
        self.entries
            .retain(|old| old.endpoint() != entry.endpoint());
        self.failed.retain(|endpoint| *endpoint != entry.endpoint());
        self.entries.push(entry);
        self.state = if self.failed.is_empty() {
            ReferenceState::Cached
        } else {
            ReferenceState::Offline
        };
        Ok(event)
    }
    /// Retains previous data and fetch times after a failed request.
    pub fn refresh_failed(&mut self, endpoint: Endpoint) {
        if !self.failed.contains(&endpoint) {
            self.failed.push(endpoint);
        }
        if !self.entries.is_empty() {
            self.state = ReferenceState::Offline;
        }
    }

    /// Captures immutable environment-specific read data for a consumer.
    #[must_use]
    pub fn snapshot(&self, environment: UexEnvironment) -> ReferenceSnapshot {
        ReferenceSnapshot::new(environment, self.state, &self.entries)
    }
    /// Returns the visible connection state.
    #[must_use]
    pub const fn state(&self) -> ReferenceState {
        self.state
    }

    /// Returns an endpoint's last successful fetch time.
    #[must_use]
    pub fn fetched_at(&self, endpoint: Endpoint) -> Option<Timestamp> {
        self.entries
            .iter()
            .find(|entry| entry.endpoint() == endpoint)
            .map(CachedReference::fetched_at)
    }

    /// Returns the owned cache adapter.
    #[must_use]
    pub fn into_store(self) -> S {
        self.store
    }
}

/// Reads retained local acceptance independently of usable UEX reference snapshots.
pub trait AcceptedPriorHistoryStore {
    /// A closed persistence failure, never an empty successful history on failure.
    type Error;
    /// Loads ordered evidence and exact active, refreshed or revoked projections.
    ///
    /// # Errors
    /// Refuses unavailable or inconsistent stored evidence.
    fn accepted_history(&mut self) -> Result<model::AcceptedPriorHistory, Self::Error>;
}
