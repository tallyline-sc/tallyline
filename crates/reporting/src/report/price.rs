// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Price approvals on immutable manual Drafts.

use super::{Deviation, InvalidReport, Report, ReportRow, RowId, UexEnvironment};
use crate::api::model::PriceConfirmation;
impl ReportRow {
    /// Returns the explicit approval bound to this exact price.
    #[must_use]
    pub const fn price_confirmation(&self) -> Option<PriceConfirmation> {
        self.price_decision.confirmation
    }
}
impl Report {
    pub(crate) fn approve_price(
        &self,
        row: RowId,
        deviation: Deviation,
        environment: UexEnvironment,
    ) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        let field = next.row_mut(row)?;
        field.price_decision.confirmation = Some(PriceConfirmation::approve(
            field.price.ok_or(InvalidReport)?,
            deviation,
            environment,
        ));
        field.price_decision.deviation = Some(deviation);
        field.price_decision.unreviewed = false;
        field.price_decision.revoked = false;
        next.version += 1;
        Ok(next)
    }
}

impl ReportRow {
    /// Returns the last assessed unlowered price deviation.
    #[must_use]
    pub const fn price_deviation(&self) -> Option<Deviation> {
        self.price_decision.deviation
    }
    /// Reports an unresolved price decision that has not received keyboard focus.
    #[must_use]
    pub const fn price_unreviewed(&self) -> bool {
        self.price_decision.unreviewed
    }
    /// Reports a price decision still requiring explicit approval.
    #[must_use]
    pub fn price_needs_confirmation(&self) -> bool {
        (self.price_decision.deviation == Some(Deviation::Major) || self.price_decision.revoked)
            && self.price_decision.confirmation.is_none()
    }
    /// Reports that reassessment revoked a price approval.
    #[must_use]
    pub const fn price_confirmation_revoked(&self) -> bool {
        self.price_decision.revoked
    }
}
impl Report {
    pub(crate) fn reassess_prices(
        &self,
        assessments: &[crate::api::model::PriceAssessment],
        environment: UexEnvironment,
    ) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        for assessment in assessments {
            let field = next.row_mut(assessment.row())?;
            let deviation = assessment.deviation().ok_or(InvalidReport)?;
            let decision = &mut field.price_decision;
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
            if !field.price_needs_confirmation() {
                field.price_decision.unreviewed = false;
            }
        }
        if next != *self {
            next.version += 1;
        }
        Ok(next)
    }
    pub(crate) fn focus_price(&self, row: RowId) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        let field = next.row_mut(row)?;
        field.price.ok_or(InvalidReport)?;
        if field.price_decision.unreviewed {
            field.price_decision.unreviewed = false;
            next.version += 1;
        }
        Ok(next)
    }
}

impl super::StoredRow {
    pub(super) fn valid_price_decision(&self) -> bool {
        let flags_empty = !self.price_decision.unreviewed && !self.price_decision.revoked;
        match self.price {
            None => {
                self.price_decision.confirmation.is_none()
                    && self.price_decision.deviation.is_none()
                    && flags_empty
            }
            Some(value) => match self.price_decision.confirmation {
                Some((approved, deviation, _)) => {
                    approved == value
                        && flags_empty
                        && self
                            .price_decision
                            .deviation
                            .is_some_and(|current| !current.is_worse_than(deviation))
                }
                None => match self.price_decision.deviation {
                    None => flags_empty,
                    Some(deviation) => {
                        !self.price_decision.unreviewed
                            || deviation == Deviation::Major
                            || self.price_decision.revoked
                    }
                },
            },
        }
    }
}
