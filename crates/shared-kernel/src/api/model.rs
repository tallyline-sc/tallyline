// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared identities, exact decimal values and precise Unix timestamps.

pub use crate::container_sizes::ContainerSize;
pub use crate::decimal::{InvalidDecimal, PricePerScu, ScuQuantity};

use std::time::Duration;

/// Maximum entered rows accepted by one immutable UEX submission.
pub const MAX_SUBMISSION_ROWS: usize = 500;

/// Unix time with millisecond precision and the full unsigned-second range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(Duration);

impl Timestamp {
    /// Creates a timestamp from Unix seconds.
    #[must_use]
    pub const fn from_seconds(seconds: u64) -> Self {
        Self(Duration::from_secs(seconds))
    }

    /// Creates a timestamp from Unix milliseconds.
    #[must_use]
    pub const fn from_millis(millis: u64) -> Self {
        Self(Duration::from_millis(millis))
    }

    /// Adds elapsed whole milliseconds without narrowing the supported timestamp range.
    #[must_use]
    pub fn checked_add(self, elapsed: Duration) -> Option<Self> {
        let elapsed = Duration::new(elapsed.as_secs(), elapsed.subsec_millis() * 1_000_000);
        self.0.checked_add(elapsed).map(Self)
    }

    /// Returns whole Unix seconds.
    #[must_use]
    pub const fn seconds(self) -> u64 {
        self.0.as_secs()
    }

    /// Returns exact Unix milliseconds without overflowing the second range.
    #[must_use]
    pub const fn millis(self) -> u128 {
        self.0.as_millis()
    }
}

/// The numeric UEX commodity identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommodityId(pub u32);

/// The numeric UEX terminal identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalId(pub u32);

/// The UEX universe selected by a game-channel mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UexEnvironment {
    /// Production universe.
    Live,
    /// Public test universe.
    Ptu,
}

/// The terminal's buy tab or local market value sell tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeSide {
    /// Buy inventory from the terminal.
    Buy,
    /// Sell inventory to the terminal.
    Sell,
}
pub use crate::image_raster::{ImageRaster, InvalidRaster};

/// A stable local Report identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReportId(pub u64);

/// A stable row identity within a Report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowId(pub u64);
pub use crate::delta::{Delta, Percent, PrecisionUnavailable};

/// The immutable UEX submission mode selected at release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionMode {
    /// Validates without storing UEX reports or changing priors.
    DryRun,
    /// Stores reports in the selected UEX environment.
    Production,
}
pub use crate::observation_age::{
    AgeConfirmation, AgeValidity, InvalidObservationAge, ObservationAgeSettings,
};

/// A positive UEX row identity retained with the partition's returned timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UexReportId {
    id: std::num::NonZeroU64,
    date_added: Timestamp,
}
impl UexReportId {
    /// Refuses dry-run zero identities and fractional partition timestamps.
    #[must_use]
    pub fn new(id: u64, date_added: Timestamp) -> Option<Self> {
        if !date_added.millis().is_multiple_of(1000) {
            return None;
        }
        Some(Self {
            id: std::num::NonZeroU64::new(id)?,
            date_added,
        })
    }
    /// Returns the positive UEX row identity.
    #[must_use]
    pub const fn id(self) -> u64 {
        self.id.get()
    }
    /// Returns the timestamp that distinguishes the UEX partition.
    #[must_use]
    pub const fn date_added(self) -> Timestamp {
        self.date_added
    }
}

/// A stable local submission identity shared across context boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubmissionJobId(pub u64);
/// The context owning a durable aggregate event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventSource {
    /// A Report lifecycle transition.
    Reporting,
    /// A submission outcome or scheduling transition.
    Submission,
    /// Independently versioned read-only evidence within Submission.
    SubmissionLifecycle,
}

/// An outbox identity stable across retries and process restarts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId {
    source: EventSource,
    aggregate_id: u64,
    version: u64,
}
impl EventId {
    /// Creates an identity for one committed aggregate version, including an initial version zero.
    #[must_use]
    pub const fn new(source: EventSource, aggregate_id: u64, version: u64) -> Option<Self> {
        if aggregate_id == 0 {
            return None;
        }
        Some(Self {
            source,
            aggregate_id,
            version,
        })
    }
    /// Returns the context owning the aggregate.
    #[must_use]
    pub const fn source(self) -> EventSource {
        self.source
    }
    /// Returns the stable local aggregate identity.
    #[must_use]
    pub const fn aggregate_id(self) -> u64 {
        self.aggregate_id
    }
    /// Returns the committed aggregate version.
    #[must_use]
    pub const fn version(self) -> u64 {
        self.version
    }
}

pub use crate::area_sample::AreaSampleError;
pub use crate::faction_affinity::{FactionAffinity, InvalidFactionAffinity};
pub use crate::projective_map::{InvalidGeometry, ProjectiveMap};
pub use crate::release_version::ReleaseVersion;
