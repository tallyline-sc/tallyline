// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable values committed before a submission attempt.

use tallyline_shared_kernel::api::model::{CommodityId, PricePerScu, RowId, ScuQuantity};

pub(crate) use tallyline_shared_kernel::api::model::MAX_SUBMISSION_ROWS as MAX_REPORT_ROWS;

/// A missing identity or invalid submission value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSubmission;

/// A complete observed row, retaining exact values until wire translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionRow {
    id: RowId,
    commodity: CommodityId,
    price: PricePerScu,
    stock: ScuQuantity,
    status: std::num::NonZeroU8,
}

impl SubmissionRow {
    /// Validates resolved identities, a positive price and an explicit status.
    ///
    /// # Errors
    /// Refuses missing identities, a zero price, non-integer or unsupported stock and an absent status.
    pub fn new(
        id: RowId,
        commodity: CommodityId,
        price: PricePerScu,
        stock: ScuQuantity,
        status: u8,
    ) -> Result<Self, InvalidSubmission> {
        if id.0 == 0 || commodity.0 == 0 || price.is_zero() || stock.whole_units().is_none() {
            return Err(InvalidSubmission);
        }
        Ok(Self {
            id,
            commodity,
            price,
            stock,
            status: std::num::NonZeroU8::new(status).ok_or(InvalidSubmission)?,
        })
    }
    /// Returns the stable Report row identity.
    #[must_use]
    pub const fn id(&self) -> RowId {
        self.id
    }
    /// Returns the resolved commodity identity.
    #[must_use]
    pub const fn commodity(&self) -> CommodityId {
        self.commodity
    }
    /// Returns the exact observed unit price.
    #[must_use]
    pub const fn price(&self) -> PricePerScu {
        self.price
    }
    /// Returns the observed inventory, preserving a known zero.
    #[must_use]
    pub const fn stock(&self) -> ScuQuantity {
        self.stock
    }
    /// Returns the explicitly selected vocabulary status.
    #[must_use]
    pub const fn status(&self) -> u8 {
        self.status.get()
    }
}

use tallyline_shared_kernel::api::model::{
    ReportId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};

/// The selected account, target and immutable observation identity at release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionHeader {
    /// Local aggregate identity.
    pub report: ReportId,
    /// The committed Report version captured at release.
    pub report_version: u64,
    /// Validated account identity, never a secret key.
    pub account: String,
    /// Test or production mode fixed for this request.
    pub mode: SubmissionMode,
    /// Resolved terminal identity.
    pub terminal: TerminalId,
    /// The only traded side represented by every row.
    pub side: TradeSide,
    /// The explicitly mapped UEX environment.
    pub environment: UexEnvironment,
    /// The version observed for this observation and environment.
    pub game_version: String,
    /// Explicit user-entered affinity with this commodity terminal's faction.
    pub faction_affinity: Option<tallyline_shared_kernel::api::model::FactionAffinity>,
    /// The original observation time, never moved later at dispatch.
    pub observed_at: Timestamp,
}

/// A bounded immutable released Report snapshot without credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionRequest {
    header: std::sync::Arc<SubmissionHeader>,
    rows: Vec<SubmissionRow>,
    newer_observation_confirmations: Vec<NewerObservationConfirmation>,
}
impl SubmissionRequest {
    /// Validates one to five hundred distinct resolved commodity rows.
    ///
    /// # Errors
    /// Refuses missing identities, account or version and duplicate or excessive rows.
    pub fn new(
        header: SubmissionHeader,
        rows: Vec<SubmissionRow>,
    ) -> Result<Self, InvalidSubmission> {
        if header.report.0 == 0
            || header.report_version == 0
            || header.terminal.0 == 0
            || header.account.trim().is_empty()
            || header.game_version.trim().is_empty()
            || rows.is_empty()
            || rows.len() > MAX_REPORT_ROWS
        {
            return Err(InvalidSubmission);
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut commodities = std::collections::BTreeSet::new();
        if rows
            .iter()
            .any(|row| !ids.insert(row.id().0) || !commodities.insert(row.commodity().0))
        {
            return Err(InvalidSubmission);
        }
        Ok(Self {
            header: std::sync::Arc::new(header),
            rows,
            newer_observation_confirmations: Vec::new(),
        })
    }
    /// Borrows the account and observation snapshot fixed at release.
    #[must_use]
    pub fn header(&self) -> &SubmissionHeader {
        &self.header
    }
    pub(crate) fn shared_header(&self) -> std::sync::Arc<SubmissionHeader> {
        std::sync::Arc::clone(&self.header)
    }
    /// Borrows the complete untrimmed row set.
    #[must_use]
    pub fn rows(&self) -> &[SubmissionRow] {
        &self.rows
    }
}

/// A user's exact-row approval against a newer accepted observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewerObservationConfirmation {
    row: RowId,
    observed_at: Timestamp,
}
impl NewerObservationConfirmation {
    /// Returns the stable row in this immutable request snapshot.
    #[must_use]
    pub const fn row(self) -> RowId {
        self.row
    }
    /// Returns the newest accepted observation covered by this decision.
    #[must_use]
    pub const fn observed_at(self) -> Timestamp {
        self.observed_at
    }
}
impl SubmissionRequest {
    /// Maps an explicit Reporting-owned user decision onto this immutable release snapshot.
    ///
    /// A newly built request starts without decisions; changed values or context cannot inherit
    /// this request's private decision list. Workflows must map only freshly checked decisions.
    ///
    /// # Errors
    /// Refuses an absent row or a timestamp that is not newer than the released observation.
    pub fn with_confirmed_newer_observation(
        &self,
        row: RowId,
        observed_at: Timestamp,
    ) -> Result<Self, InvalidSubmission> {
        if observed_at <= self.header.observed_at
            || !self.rows.iter().any(|value| value.id() == row)
        {
            return Err(InvalidSubmission);
        }
        let mut next = self.clone();
        if let Some(old) = next
            .newer_observation_confirmations
            .iter_mut()
            .find(|old| old.row == row)
        {
            old.observed_at = old.observed_at.max(observed_at);
        } else {
            next.newer_observation_confirmations
                .push(NewerObservationConfirmation { row, observed_at });
        }
        Ok(next)
    }
    /// Borrows decisions bound to this request's exact values and context.
    #[must_use]
    pub fn newer_observation_confirmations(&self) -> &[NewerObservationConfirmation] {
        &self.newer_observation_confirmations
    }
    pub(crate) fn newer_observation_confirmed(&self, row: RowId, observed_at: Timestamp) -> bool {
        self.newer_observation_confirmations
            .iter()
            .any(|approval| approval.row == row && approval.observed_at >= observed_at)
    }
}
