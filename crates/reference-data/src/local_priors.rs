// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Provenance-preserving accepted local rows and borrowed reference selection.

use crate::api::model::{InvalidPrior, PricePrior, PriorValues};
use std::sync::Arc;
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, SubmissionJobId, TerminalId, Timestamp, TradeSide, UexEnvironment,
};

/// Immutable history provenance shared by the accepted rows of one production submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedPriorContext {
    /// The accepted local Report identity.
    pub report: ReportId,
    /// The exact Report version submitted.
    pub report_version: u64,
    /// The durable production submission identity.
    pub submission: SubmissionJobId,
    /// The environment in which the rows were accepted.
    pub environment: UexEnvironment,
    /// The submitted terminal.
    pub terminal: TerminalId,
    /// The only submitted trade side.
    pub side: TradeSide,
    /// The submitted version, never silently replaced with the current version.
    pub game_version: String,
    /// Original observation time of the exact accepted values.
    pub observed_at: Timestamp,
    /// Time at which the successful production answer was received.
    pub accepted_at: Timestamp,
}

/// An accepted production value, distinct from a UEX-fetched prior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedLocalPrior {
    context: Arc<AcceptedPriorContext>,
    commodity: CommodityId,
    values: PriorValues,
}
impl AcceptedLocalPrior {
    /// Maps a workflow-verified accepted production row without changing its provenance.
    ///
    /// # Errors
    /// Refuses absent identities, missing source version/status, zero price or unsupported stock.
    pub fn new(
        context: Arc<AcceptedPriorContext>,
        commodity: CommodityId,
        values: PriorValues,
    ) -> Result<Self, InvalidPrior> {
        if context.report.0 == 0
            || context.report_version == 0
            || context.submission.0 == 0
            || context.terminal.0 == 0
            || context.game_version.trim().is_empty()
            || commodity.0 == 0
            || values.price.is_zero()
            || values.status.is_none()
            || values.stock.whole_units().is_none()
        {
            return Err(InvalidPrior);
        }
        Ok(Self {
            context,
            commodity,
            values,
        })
    }
    /// Borrows the immutable submission provenance, including its environment.
    #[must_use]
    pub fn context(&self) -> &AcceptedPriorContext {
        &self.context
    }
    /// Returns the accepted commodity identity.
    #[must_use]
    pub const fn commodity(&self) -> CommodityId {
        self.commodity
    }
    /// Borrows the exact values accepted in production.
    #[must_use]
    pub const fn values(&self) -> &PriorValues {
        &self.values
    }
}

/// The provenance of a usable selected reference, independent of its tolerance band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorReferenceSource {
    /// A successful UEX reference response.
    Uex,
    /// The local user's successful production submission.
    AcceptedLocal,
}

/// A borrowed row without converting local acceptance into UEX provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorReference<'a> {
    /// The UEX-fetched terminal prior.
    Uex(&'a PricePrior),
    /// An active accepted local LIVE value.
    AcceptedLocal(&'a AcceptedLocalPrior),
}
impl<'a> PriorReference<'a> {
    /// Borrows the selected row's exact values.
    #[must_use]
    pub const fn values(self) -> &'a PriorValues {
        match self {
            Self::Uex(prior) => prior.values(),
            Self::AcceptedLocal(prior) => prior.values(),
        }
    }
    /// Returns the original observation time used for reference ageing.
    #[must_use]
    pub fn modified_at(self) -> Timestamp {
        match self {
            Self::Uex(prior) => prior.provenance().modified_at,
            Self::AcceptedLocal(prior) => prior.context().observed_at,
        }
    }
    /// Borrows the source version without cloning metadata per row.
    #[must_use]
    pub fn game_version(self) -> &'a str {
        match self {
            Self::Uex(prior) => &prior.provenance().game_version,
            Self::AcceptedLocal(prior) => &prior.context().game_version,
        }
    }
    /// Identifies the source without confusing local acceptance with a public UEX reference.
    #[must_use]
    pub const fn source(self) -> PriorReferenceSource {
        match self {
            Self::Uex(_) => PriorReferenceSource::Uex,
            Self::AcceptedLocal(_) => PriorReferenceSource::AcceptedLocal,
        }
    }
    /// Borrows local submission provenance only when that was the selected source.
    #[must_use]
    pub fn local_source(self) -> Option<&'a AcceptedPriorContext> {
        match self {
            Self::Uex(_) => None,
            Self::AcceptedLocal(prior) => Some(prior.context()),
        }
    }
}
