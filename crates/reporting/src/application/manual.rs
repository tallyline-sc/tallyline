// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic manual Report edits against one observed version and reference context.

use super::{ManualEdit, ReportingApplication};
use crate::api::model::{Report, ReportId};
use crate::api::{ReportRepository, RepositoryError};
use tallyline_reference_data::api::model::ReferenceSnapshot;
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::TradeSide;

impl<R: ReportRepository, C: Clock> ReportingApplication<R, C> {
    /// Validates and persists an explicit user edit against its expected version.
    ///
    /// # Errors
    /// Returns stale edits, invalid vocabulary/reference choices or unavailable storage.
    pub fn edit(
        &mut self,
        id: ReportId,
        expected: u64,
        edit: ManualEdit,
        snapshot: &ReferenceSnapshot,
    ) -> Result<Report, RepositoryError> {
        self.edit_batch(id, expected, std::slice::from_ref(&edit), snapshot)
    }
    /// Validates and atomically persists ordered user edits against one observed version.
    ///
    /// # Errors
    /// Refuses empty input, stale versions, invalid edits or unavailable storage.
    pub fn edit_batch(
        &mut self,
        id: ReportId,
        expected: u64,
        edits: &[ManualEdit],
        snapshot: &ReferenceSnapshot,
    ) -> Result<Report, RepositoryError> {
        if edits.is_empty() {
            return Err(RepositoryError::Invalid);
        }
        let mut report = self
            .repository
            .load_draft(id)?
            .ok_or(RepositoryError::Invalid)?;
        if report.version() != expected {
            return Err(RepositoryError::Conflict);
        }
        for edit in edits {
            self.apply_edit(&mut report, edit, snapshot)?;
        }
        if report.version() > expected {
            self.repository.save_draft(&report, expected)?;
        }
        Ok(report)
    }
    fn apply_edit(
        &self,
        report: &mut Report,
        edit: &ManualEdit,
        snapshot: &ReferenceSnapshot,
    ) -> Result<(), RepositoryError> {
        let key = match *edit {
            ManualEdit::ChangeKey(key) => key,
            ManualEdit::AddCommodity(_)
            | ManualEdit::FactionAffinity(_)
            | ManualEdit::Price(..)
            | ManualEdit::Stock(..)
            | ManualEdit::Status(..)
            | ManualEdit::TakeOver(_) => report.key(),
        };
        if self.mapping.environment(key.environment()) != snapshot.environment() {
            return Err(RepositoryError::Invalid);
        }
        let result = match *edit {
            ManualEdit::AddCommodity(commodity) => {
                let known = snapshot
                    .commodities("")
                    .iter()
                    .any(|item| item.id() == commodity)
                    || snapshot
                        .terminal_priors(key.terminal())
                        .is_some_and(|priors| {
                            priors
                                .rows()
                                .iter()
                                .any(|row| row.commodity() == commodity && row.side() == key.side())
                        });
                if !known {
                    return Err(RepositoryError::Invalid);
                }
                report.add_commodity(commodity).map(|_| ())
            }
            ManualEdit::Price(row, value) => report.set_price(row, value),
            ManualEdit::FactionAffinity(value) => report.set_faction_affinity(value).map(|state| {
                *report = state;
            }),
            ManualEdit::Stock(row, value) => report.set_stock(row, value),
            ManualEdit::Status(row, value) => {
                if let Some(value) = value {
                    let levels = snapshot.status_levels().ok_or(RepositoryError::Invalid)?;
                    let statuses = match key.side() {
                        TradeSide::Buy => &levels.buy,
                        TradeSide::Sell => &levels.sell,
                    };
                    if !statuses.iter().any(|level| level.code == value) {
                        return Err(RepositoryError::Invalid);
                    }
                }
                report.set_status(row, value)
            }
            ManualEdit::TakeOver(row) => {
                let commodity = report
                    .rows()
                    .iter()
                    .find(|item| item.id() == row)
                    .ok_or(RepositoryError::Invalid)?
                    .commodity();
                let prior = snapshot
                    .terminal_priors(key.terminal())
                    .and_then(|priors| {
                        priors.rows().iter().find(|prior| {
                            prior.commodity() == commodity && prior.side() == key.side()
                        })
                    })
                    .ok_or(RepositoryError::Invalid)?;
                let values = prior.values();
                report
                    .set_price(row, Some(values.price))
                    .and_then(|()| report.set_stock(row, Some(values.stock)))
                    .and_then(|()| report.set_status(row, values.status))
            }
            ManualEdit::ChangeKey(key) => {
                if !snapshot
                    .terminals("")
                    .iter()
                    .any(|terminal| terminal.id() == key.terminal())
                {
                    return Err(RepositoryError::Invalid);
                }
                report.change_key(key)
            }
        };
        result.map_err(|_| RepositoryError::Invalid)
    }
}
