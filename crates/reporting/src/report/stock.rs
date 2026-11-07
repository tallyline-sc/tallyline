// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stock approvals on immutable manual Drafts.

use super::{Deviation, InvalidReport, Report, ReportRow, RowId, UexEnvironment};
use crate::api::model::StockConfirmation;
impl ReportRow {
    /// Returns the explicit approval bound to this exact stock.
    #[must_use]
    pub const fn stock_confirmation(&self) -> Option<StockConfirmation> {
        self.stock_decision.confirmation
    }
}
impl Report {
    pub(crate) fn approve_stock(
        &self,
        row: RowId,
        deviation: Deviation,
        environment: UexEnvironment,
    ) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        let field = next.row_mut(row)?;
        field.stock_decision.confirmation = Some(StockConfirmation::approve(
            field.stock.ok_or(InvalidReport)?,
            deviation,
            environment,
        ));
        field.stock_decision.deviation = Some(deviation);
        field.stock_decision.unreviewed = false;
        field.stock_decision.revoked = false;
        next.version += 1;
        Ok(next)
    }
}

impl ReportRow {
    /// Returns the last assessed unlowered stock deviation.
    #[must_use]
    pub const fn stock_deviation(&self) -> Option<Deviation> {
        self.stock_decision.deviation
    }
    /// Reports an unresolved stock decision that has not received keyboard focus.
    #[must_use]
    pub const fn stock_unreviewed(&self) -> bool {
        self.stock_decision.unreviewed
    }
    /// Reports a stock decision still requiring explicit approval.
    #[must_use]
    pub fn stock_needs_confirmation(&self) -> bool {
        (self.stock_decision.deviation == Some(Deviation::Major) || self.stock_decision.revoked)
            && self.stock_decision.confirmation.is_none()
    }
    /// Reports that reassessment revoked a stock approval.
    #[must_use]
    pub const fn stock_confirmation_revoked(&self) -> bool {
        self.stock_decision.revoked
    }
}
impl Report {
    pub(crate) fn reassess_stocks(
        &self,
        assessments: &[crate::api::model::StockAssessment],
        environment: UexEnvironment,
    ) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        for assessment in assessments {
            let field = next.row_mut(assessment.row())?;
            let deviation = assessment.deviation().ok_or(InvalidReport)?;
            let decision = &mut field.stock_decision;
            let worsened = decision.confirmation.is_some_and(|approval| {
                environment != approval.reference_environment()
                    || deviation.is_worse_than(approval.deviation())
            });
            if worsened {
                decision.confirmation = None;
                decision.revoked = true;
            }
            if worsened
                || (deviation == Deviation::Major && decision.deviation != Some(Deviation::Major))
            {
                decision.unreviewed = true;
            }
            decision.deviation = Some(deviation);
            if !field.stock_needs_confirmation() {
                field.stock_decision.unreviewed = false;
            }
        }
        if next != *self {
            next.version += 1;
        }
        Ok(next)
    }
    pub(crate) fn focus_stock(&self, row: RowId) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        let field = next.row_mut(row)?;
        field.stock.ok_or(InvalidReport)?;
        if field.stock_decision.unreviewed {
            field.stock_decision.unreviewed = false;
            next.version += 1;
        }
        Ok(next)
    }
}

impl super::StoredRow {
    pub(super) fn valid_stock_decision(&self) -> bool {
        let flags_empty = !self.stock_decision.unreviewed && !self.stock_decision.revoked;
        match self.stock {
            None => {
                self.stock_decision.confirmation.is_none()
                    && self.stock_decision.deviation.is_none()
                    && flags_empty
            }
            Some(value) => match self.stock_decision.confirmation {
                Some((approved, deviation, _)) => {
                    approved == value
                        && flags_empty
                        && self
                            .stock_decision
                            .deviation
                            .is_some_and(|current| !current.is_worse_than(deviation))
                }
                None => match self.stock_decision.deviation {
                    None => flags_empty,
                    Some(deviation) => {
                        !self.stock_decision.unreviewed
                            || deviation == Deviation::Major
                            || self.stock_decision.revoked
                    }
                },
            },
        }
    }
}
