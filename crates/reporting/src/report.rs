// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! State-local manual Report invariants.

use crate::api::model::{Deviation, ManualAttachment, StatusConfirmation, StoredAttachment};
use tallyline_game::api::model::GameEnvironment;
use tallyline_shared_kernel::api::model::{
    CommodityId, FactionAffinity, PricePerScu, ReportId, RowId, ScuQuantity, TerminalId, Timestamp,
    TradeSide, UexEnvironment,
};

/// The source of an explicitly observed field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldOrigin {
    /// A typed value or explicit reference takeover.
    User,
}

/// A refused manual Report command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidReport;
impl std::fmt::Display for InvalidReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("manual Report command is invalid")
    }
}
impl std::error::Error for InvalidReport {}

/// Explicit terminal, side and launcher environment of one Report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportKey {
    terminal: TerminalId,
    side: TradeSide,
    environment: GameEnvironment,
}
impl ReportKey {
    /// Constructs an explicitly selected key.
    ///
    /// # Errors
    /// Refuses an absent terminal identity.
    pub fn new(
        terminal: TerminalId,
        side: TradeSide,
        environment: GameEnvironment,
    ) -> Result<Self, InvalidReport> {
        if terminal.0 == 0 {
            return Err(InvalidReport);
        }
        Ok(Self {
            terminal,
            side,
            environment,
        })
    }
    /// Returns the selected terminal.
    #[must_use]
    pub const fn terminal(self) -> TerminalId {
        self.terminal
    }
    /// Returns the selected side.
    #[must_use]
    pub const fn side(self) -> TradeSide {
        self.side
    }
    /// Returns the explicitly selected game environment.
    #[must_use]
    pub const fn environment(self) -> GameEnvironment {
        self.environment
    }
}

/// A manual commodity row containing only explicitly entered values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportRow {
    id: RowId,
    commodity: CommodityId,
    price: Option<PricePerScu>,
    stock: Option<ScuQuantity>,
    status: Option<std::num::NonZeroU8>,
    price_decision: crate::price_confirmation::PriceDecision,
    stock_decision: crate::stock_confirmation::StockDecision,
    status_confirmation: Option<StatusConfirmation>,
    status_deviation: Option<Deviation>,
    status_unreviewed: bool,
    status_confirmation_revoked: bool,
}
impl ReportRow {
    /// Returns USER only when a field was explicitly entered.
    #[must_use]
    pub const fn origin(&self) -> Option<FieldOrigin> {
        if self.entered() {
            Some(FieldOrigin::User)
        } else {
            None
        }
    }
    /// Returns the stable row identity.
    #[must_use]
    pub const fn id(&self) -> RowId {
        self.id
    }
    /// Returns the explicitly selected commodity.
    #[must_use]
    pub const fn commodity(&self) -> CommodityId {
        self.commodity
    }
    /// Returns the user-entered price.
    #[must_use]
    pub const fn price(&self) -> Option<PricePerScu> {
        self.price
    }
    /// Returns the user-entered inventory.
    #[must_use]
    pub const fn stock(&self) -> Option<ScuQuantity> {
        self.stock
    }
    /// Returns the user-selected status.
    #[must_use]
    pub const fn status(&self) -> Option<std::num::NonZeroU8> {
        self.status
    }
    /// Reports whether at least one field was explicitly entered.
    #[must_use]
    pub const fn entered(&self) -> bool {
        self.price.is_some() || self.stock.is_some() || self.status.is_some()
    }
}

/// A manually created Draft with private, command-controlled state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    id: ReportId,
    key: ReportKey,
    observed_at: Timestamp,
    game_version: Option<String>,
    faction_affinity: Option<FactionAffinity>,
    version: u64,
    rows: Vec<ReportRow>,
    attachment: Option<ManualAttachment>,
    lifecycle: lifecycle::ReportLifecycle,
}
impl Report {
    /// Creates an empty manual Draft at its observation time.
    ///
    /// # Errors
    /// Refuses an absent Report identity or empty supplied game version.
    pub fn manual(
        id: ReportId,
        key: ReportKey,
        observed_at: Timestamp,
        game_version: Option<&str>,
    ) -> Result<Self, InvalidReport> {
        if id.0 == 0 || game_version.is_some_and(|v| v.trim().is_empty()) {
            return Err(InvalidReport);
        }
        Ok(Self {
            id,
            key,
            observed_at,
            game_version: game_version.map(str::to_owned),
            faction_affinity: None,
            version: 0,
            rows: Vec::new(),
            attachment: None,
            lifecycle: lifecycle::ReportLifecycle::default(),
        })
    }
    /// Adds an empty row without copying any reference value.
    ///
    /// # Errors
    /// Refuses a zero or duplicate commodity identity or exhausted row identity.
    pub fn add_commodity(&mut self, commodity: CommodityId) -> Result<RowId, InvalidReport> {
        self.ensure_draft()?;
        if commodity.0 == 0 || self.rows.iter().any(|row| row.commodity == commodity) {
            return Err(InvalidReport);
        }
        let id = RowId(
            self.rows
                .iter()
                .map(|row| row.id.0)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(InvalidReport)?,
        );
        self.rows.push(ReportRow {
            id,
            commodity,
            price: None,
            stock: None,
            status: None,
            price_decision: crate::price_confirmation::PriceDecision::default(),
            stock_decision: crate::stock_confirmation::StockDecision::default(),
            status_confirmation: None,
            status_deviation: None,
            status_unreviewed: false,
            status_confirmation_revoked: false,
        });
        self.version += 1;
        Ok(id)
    }
    /// Changes Draft context, clearing faction affinity only when the terminal changes.
    ///
    /// # Errors
    /// Refuses context changes after release.
    pub fn change_key(&mut self, key: ReportKey) -> Result<(), InvalidReport> {
        self.ensure_draft()?;
        if self.key != key {
            if self.key.terminal != key.terminal {
                self.faction_affinity = None;
            }
            if self.key.environment != key.environment {
                self.game_version = None;
            }
            for row in &mut self.rows {
                row.price_decision = crate::price_confirmation::PriceDecision::default();
                row.stock_decision = crate::stock_confirmation::StockDecision::default();
                row.status_confirmation = None;
                row.status_deviation = None;
                row.status_unreviewed = false;
                row.status_confirmation_revoked = false;
            }
            self.revoke_all_newer_observations();
            self.key = key;
            self.version += 1;
        }
        Ok(())
    }
    /// Sets or clears an explicitly entered price.
    ///
    /// # Errors
    /// Refuses an unknown row or a zero traded price.
    pub fn set_price(
        &mut self,
        row: RowId,
        value: Option<PricePerScu>,
    ) -> Result<(), InvalidReport> {
        self.ensure_draft()?;
        if value.is_some_and(PricePerScu::is_zero) {
            return Err(InvalidReport);
        }
        let field = self.row_mut(row)?;
        let changed = field.price != value;
        if changed {
            field.price_decision = crate::price_confirmation::PriceDecision::default();
        }
        field.price = value;
        if changed {
            self.revoke_newer_observation(row);
        }
        self.version += 1;
        Ok(())
    }
    /// Sets or clears an explicitly entered inventory, including a known zero.
    ///
    /// # Errors
    /// Refuses an unknown row.
    pub fn set_stock(
        &mut self,
        row: RowId,
        value: Option<ScuQuantity>,
    ) -> Result<(), InvalidReport> {
        self.ensure_draft()?;
        let field = self.row_mut(row)?;
        let changed = field.stock != value;
        if changed {
            field.stock_decision = crate::stock_confirmation::StockDecision::default();
        }
        field.stock = value;
        if changed {
            self.revoke_newer_observation(row);
        }
        self.version += 1;
        Ok(())
    }
    /// Sets or clears an explicitly selected status.
    ///
    /// # Errors
    /// Refuses an unknown row; vocabulary membership is assessed against the snapshot.
    pub fn set_status(
        &mut self,
        row: RowId,
        value: Option<std::num::NonZeroU8>,
    ) -> Result<(), InvalidReport> {
        self.ensure_draft()?;
        let field = self.row_mut(row)?;
        let changed = field.status != value;
        if changed {
            field.status_confirmation = None;
            field.status_deviation = None;
            field.status_unreviewed = false;
            field.status_confirmation_revoked = false;
        }
        field.status = value;
        if changed {
            self.revoke_newer_observation(row);
        }
        self.version += 1;
        Ok(())
    }
    fn row_mut(&mut self, id: RowId) -> Result<&mut ReportRow, InvalidReport> {
        self.rows
            .iter_mut()
            .find(|row| row.id == id)
            .ok_or(InvalidReport)
    }
    /// Returns the persistent identity.
    #[must_use]
    pub const fn id(&self) -> ReportId {
        self.id
    }
    /// Returns the explicit grouping key.
    #[must_use]
    pub const fn key(&self) -> ReportKey {
        self.key
    }
    /// Returns the manual observation time.
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
    /// Returns the captured game version without relabelling it on refresh.
    #[must_use]
    pub fn game_version(&self) -> Option<&str> {
        self.game_version.as_deref()
    }
    /// Returns only the player's explicitly entered terminal-faction affinity.
    #[must_use]
    pub const fn faction_affinity(&self) -> Option<FactionAffinity> {
        self.faction_affinity
    }
    /// Returns a Draft with the player's explicitly entered terminal-faction affinity set or cleared.
    ///
    /// # Errors
    /// Refuses changes after release.
    pub fn set_faction_affinity(
        &self,
        value: Option<FactionAffinity>,
    ) -> Result<Self, InvalidReport> {
        self.ensure_draft()?;
        let mut state = self.clone();
        if state.faction_affinity != value {
            state.faction_affinity = value;
            state.version += 1;
        }
        Ok(state)
    }
    /// Returns the optimistic concurrency version.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
    /// Returns immutable commodity rows.
    #[must_use]
    pub fn rows(&self) -> &[ReportRow] {
        &self.rows
    }
}

/// Persistence-only state of one manual Report.
#[derive(Clone, Debug)]
pub struct StoredReport {
    /// Persistent identity.
    pub id: ReportId,
    /// Explicit grouping key.
    pub key: ReportKey,
    /// Original observation time.
    pub observed_at: Timestamp,
    /// Version assigned to the manual observation.
    pub game_version: Option<String>,
    /// Only the explicitly entered terminal-faction affinity.
    pub faction_affinity: Option<FactionAffinity>,
    /// Optimistic concurrency version.
    pub version: u64,
    /// Explicit rows.
    pub rows: Vec<StoredRow>,
    /// Selected attachment metadata without original pixels.
    pub attachment: Option<StoredAttachment>,
    /// Validated lifecycle and fixed release metadata.
    pub lifecycle: StoredReportLifecycle,
}

/// Persistence-only state of a manual row.
#[derive(Clone, Debug)]
pub struct StoredRow {
    /// Stable row identity.
    pub id: RowId,
    /// Resolved commodity identity.
    pub commodity: CommodityId,
    /// Explicit price.
    pub price: Option<PricePerScu>,
    /// Explicit stock.
    pub stock: Option<ScuQuantity>,
    /// Explicit status.
    pub status: Option<std::num::NonZeroU8>,
    /// Persisted price review decision.
    pub price_decision: crate::api::model::StoredPriceDecision,
    /// Persisted stock review decision.
    pub stock_decision: crate::api::model::StoredStockDecision,
    /// Value and unlowered deviation explicitly approved by the user.
    pub status_confirmation: Option<(std::num::NonZeroU8, Deviation, UexEnvironment)>,
    /// Last assessed unlowered status deviation.
    pub status_deviation: Option<Deviation>,
    /// An unresolved status decision that has not received keyboard focus.
    pub status_unreviewed: bool,
    /// A revoked approval that still needs a user decision.
    pub status_confirmation_revoked: bool,
}

impl Report {
    /// Reconstitutes validated state for the persistence adapter.
    ///
    /// # Errors
    /// Refuses invalid identities, duplicate commodities or rows and zero prices.
    pub fn reconstitute(stored: StoredReport) -> Result<Self, InvalidReport> {
        let mut report = Self::manual(
            stored.id,
            stored.key,
            stored.observed_at,
            stored.game_version.as_deref(),
        )?;
        let mut ids = std::collections::BTreeSet::new();
        for row in stored.rows {
            if row.id.0 == 0
                || !ids.insert(row.id.0)
                || row.commodity.0 == 0
                || row.price.is_some_and(PricePerScu::is_zero)
                || !row.valid_price_decision()
                || !row.valid_stock_decision()
                || !row.valid_status_decision()
                || report.rows.iter().any(|old| old.commodity == row.commodity)
            {
                return Err(InvalidReport);
            }
            report.rows.push(ReportRow {
                id: row.id,
                commodity: row.commodity,
                price: row.price,
                stock: row.stock,
                status: row.status,
                price_decision: crate::price_confirmation::PriceDecision {
                    confirmation: row.price_decision.confirmation.map(
                        |(value, deviation, environment)| {
                            crate::api::model::PriceConfirmation::approve(
                                value,
                                deviation,
                                environment,
                            )
                        },
                    ),
                    deviation: row.price_decision.deviation,
                    unreviewed: row.price_decision.unreviewed,
                    revoked: row.price_decision.revoked,
                },
                stock_decision: crate::stock_confirmation::StockDecision {
                    confirmation: row.stock_decision.confirmation.map(
                        |(value, deviation, environment)| {
                            crate::api::model::StockConfirmation::approve(
                                value,
                                deviation,
                                environment,
                            )
                        },
                    ),
                    deviation: row.stock_decision.deviation,
                    unreviewed: row.stock_decision.unreviewed,
                    revoked: row.stock_decision.revoked,
                },
                status_confirmation: row.status_confirmation.map(
                    |(value, deviation, environment)| {
                        StatusConfirmation::approve(value, deviation, environment)
                    },
                ),
                status_deviation: row.status_deviation,
                status_unreviewed: row.status_unreviewed,
                status_confirmation_revoked: row.status_confirmation_revoked,
            });
        }
        if stored.attachment.is_some_and(|attachment| {
            attachment.working_copy_owner.is_some() && attachment.working_copy.is_none()
        }) {
            return Err(InvalidReport);
        }
        report.attachment = stored.attachment.map(|attachment| ManualAttachment {
            binding: attachment.binding,
            time: attachment.time,
            preview_confirmed: attachment.preview_confirmed,
            working_copy: attachment.working_copy,
            working_copy_owner: attachment
                .working_copy
                .map(|_| attachment.working_copy_owner.unwrap_or(stored.id)),
        });
        if report.attachment.is_some_and(|attachment| {
            attachment.time.at() < report.observed_at
                || (attachment.working_copy.is_some() && !attachment.preview_confirmed)
                || attachment
                    .working_copy_owner
                    .is_some_and(|owner| owner.0 == 0)
        }) {
            return Err(InvalidReport);
        }
        report.faction_affinity = stored.faction_affinity;
        report.version = stored.version;
        report.lifecycle = lifecycle::ReportLifecycle::restore(
            stored.lifecycle,
            stored.version,
            stored.observed_at,
            &report,
        )?;
        Ok(report)
    }
}

mod status;

mod attachment;

mod price;

mod stock;

mod fixable_reason;
mod lifecycle;
pub use fixable_reason::FixableReason;
pub use lifecycle::{
    ReportChange, ReportDelivery, ReportEvent, ReportEventKind, ReportFinding,
    ReportReleaseBinding, ReportSplit, ReportState, StoredReportLifecycle,
    StoredReportReleaseBinding,
};

mod newer_observation;
pub use newer_observation::{
    NewerObservationApproval, NewerObservationEvidence, StoredNewerObservationApproval,
};

mod duplicate_as_draft;
