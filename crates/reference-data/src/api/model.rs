// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable cached reference records and application state.

use tallyline_shared_kernel::api::model::{
    CommodityId, PricePerScu, TerminalId, Timestamp, TradeSide,
};

pub use crate::averages::CommodityAverages;
pub use crate::parameters::{
    AcceptanceSwitch, CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
};
pub use crate::priors::{InvalidPrior, PricePrior, PriorProvenance, PriorValues, TerminalPriors};
pub use crate::snapshot::ReferenceSnapshot;
pub use tallyline_shared_kernel::api::model::UexEnvironment;
/// A reference endpoint with an independent freshness window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endpoint {
    /// Commodity vocabulary and LIVE averages.
    Commodities,
    /// Selectable commodity terminal vocabulary.
    Terminals,
    /// Current LIVE and optional PTU versions.
    GameVersions,
    /// Per-side inventory status labels.
    StatusLevels,
    /// Submission acceptance, tolerances and bounded staff text.
    Parameters,
    /// Per-terminal LIVE price assortment and provenance.
    TerminalPriors(TerminalId),
}

impl Endpoint {
    /// Returns the endpoint's client freshness window in seconds.
    #[must_use]
    pub const fn ttl_seconds(self) -> u64 {
        match self {
            Self::TerminalPriors(_) => 1800,
            Self::Commodities => 3_600,
            Self::Terminals => 43_200,
            Self::GameVersions | Self::StatusLevels | Self::Parameters => 86_400,
        }
    }
}

/// One commodity identified by its UEX ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commodity {
    id: CommodityId,
    name: String,
    code: String,
    available_live: bool,
    averages: CommodityAverages,
}

impl Commodity {
    /// Creates a boundary-mapped commodity.
    #[must_use]
    pub fn new(
        id: CommodityId,
        name: &str,
        code: &str,
        available_live: bool,
        averages: CommodityAverages,
    ) -> Self {
        Self {
            id,
            name: name.to_owned(),
            code: code.to_owned(),
            available_live,
            averages,
        }
    }

    /// Returns the numeric UEX identity.
    #[must_use]
    pub const fn id(&self) -> CommodityId {
        self.id
    }

    /// Returns the display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the non-unique commodity code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Reports availability in the selected environment.
    #[must_use]
    pub const fn available(&self, environment: UexEnvironment) -> bool {
        match environment {
            UexEnvironment::Live => self.available_live,
            UexEnvironment::Ptu => true,
        }
    }

    /// Returns the LIVE average, with no substitute in PTU.
    #[must_use]
    pub const fn average_price(
        &self,
        environment: UexEnvironment,
        side: TradeSide,
    ) -> Option<PricePerScu> {
        match environment {
            UexEnvironment::Live => self.averages.price(side),
            UexEnvironment::Ptu => None,
        }
    }
}

/// A successfully mapped response for one endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndpointData {
    /// Commodity vocabulary.
    Commodities(Vec<Commodity>),
    /// Selectable commodity terminals.
    Terminals(Vec<Terminal>),
    /// Current versions returned by a successful public request.
    GameVersions(GameVersions),
    /// Current per-side inventory status vocabulary.
    StatusLevels(StatusLevels),
    /// Current submission policy and commodity constraints.
    Parameters(ReferenceParameters),
    /// LIVE assortment fetched for one terminal.
    TerminalPriors(TerminalPriors),
}

/// One atomically persisted successful endpoint response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedReference {
    fetched_at: Timestamp,
    data: EndpointData,
}

impl CachedReference {
    /// Creates a successful response with its actual fetch time.
    #[must_use]
    pub const fn new(fetched_at: Timestamp, data: EndpointData) -> Self {
        Self { fetched_at, data }
    }

    /// Returns the response endpoint.
    #[must_use]
    pub const fn endpoint(&self) -> Endpoint {
        match &self.data {
            EndpointData::Commodities(_) => Endpoint::Commodities,
            EndpointData::Terminals(_) => Endpoint::Terminals,
            EndpointData::GameVersions(_) => Endpoint::GameVersions,
            EndpointData::StatusLevels(_) => Endpoint::StatusLevels,
            EndpointData::Parameters(_) => Endpoint::Parameters,
            EndpointData::TerminalPriors(priors) => Endpoint::TerminalPriors(priors.terminal()),
        }
    }

    /// Returns the successful network fetch time.
    #[must_use]
    pub const fn fetched_at(&self) -> Timestamp {
        self.fetched_at
    }

    /// Returns the mapped response.
    #[must_use]
    pub const fn data(&self) -> &EndpointData {
        &self.data
    }
}

/// The visible reference-data connection state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceState {
    /// No successful cached reference response exists.
    NoReferenceData,
    /// Cached data is immediately usable.
    Cached,
    /// A refresh failed while retaining cached data.
    Offline,
}

/// Decoded terminal names and location context, none of which is a unique key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalText {
    /// The full terminal name.
    pub name: String,
    /// The terminal's abbreviated name.
    pub nickname: String,
    /// The API's display label.
    pub display_name: String,
    /// Available location names, retained for disambiguation.
    pub location: String,
}

/// One selectable commodity terminal identified by its UEX ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terminal {
    id: TerminalId,
    text: TerminalText,
    available_live: bool,
    max_container_size: Option<std::num::NonZeroU32>,
}

impl Terminal {
    /// Creates a terminal after the adapter excludes unsupported terminal types.
    #[must_use]
    pub const fn new(id: TerminalId, text: TerminalText, available_live: bool) -> Self {
        Self {
            id,
            text,
            available_live,
            max_container_size: None,
        }
    }

    /// Returns a changed terminal with its known maximum container size.
    #[must_use]
    pub const fn with_max_container_size(mut self, maximum: Option<std::num::NonZeroU32>) -> Self {
        self.max_container_size = maximum;
        self
    }

    /// Returns the maximum container size, absent when the API has no known limit.
    #[must_use]
    pub const fn max_container_size(&self) -> Option<std::num::NonZeroU32> {
        self.max_container_size
    }

    /// Returns the numeric UEX identity.
    #[must_use]
    pub const fn id(&self) -> TerminalId {
        self.id
    }

    /// Returns decoded display and search text.
    #[must_use]
    pub const fn text(&self) -> &TerminalText {
        &self.text
    }

    /// Reports availability without applying LIVE flags to PTU.
    #[must_use]
    pub const fn available(&self, environment: UexEnvironment) -> bool {
        match environment {
            UexEnvironment::Live => self.available_live,
            UexEnvironment::Ptu => true,
        }
    }
}

/// Current UEX version strings, preserved without imposing semantic versioning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameVersions {
    /// The current LIVE version.
    pub live: String,
    /// The current PTU version, absent while none is published.
    pub ptu: Option<String>,
}

/// A persisted successful network response, never produced by cache loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceDataRefreshed {
    /// The endpoint actually fetched.
    pub endpoint: Endpoint,
    /// The successful network observation time.
    pub fetched_at: Timestamp,
    /// A new version-history observation only for a game-versions fetch.
    pub game_versions: Option<GameVersions>,
}

/// One non-zero inventory status code with its side-specific labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLevel {
    /// The server's non-zero code, whose membership is checked against this vocabulary.
    pub code: std::num::NonZeroU8,
    /// The complete display label.
    pub name: String,
    /// The abbreviated display label.
    pub short_name: String,
    /// The server's abbreviation.
    pub abbreviation: String,
}

/// Independent buy and sell inventory-status vocabularies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLevels {
    /// Buy-side labels in server order.
    pub buy: Vec<StatusLevel>,
    /// Sell-side labels in server order.
    pub sell: Vec<StatusLevel>,
}
pub use crate::local_priors::{
    AcceptedLocalPrior, AcceptedPriorContext, PriorReference, PriorReferenceSource,
};
pub use crate::parameters::{EffectiveStaleness, StalenessSource};
pub use crate::scu_tolerance::{ScuTolerance, ScuToleranceSource};
pub use crate::tolerance::PriceReferenceSource;
pub use crate::tolerance::{PriceTolerance, PriceToleranceSource, SourcedPriceFallback};

pub use crate::prior_history::{
    AcceptedPriorHistory, AcceptedPriorRecord, LocalPriorProjection, LocalPriorRevocation,
    StoredAcceptedPriorRecord,
};
