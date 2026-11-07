// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Status decisions on immutable manual Drafts.

use super::{
    Deviation, InvalidReport, Report, ReportRow, RowId, StatusConfirmation, UexEnvironment,
};
impl ReportRow {
    /// Returns the explicit approval bound to this status value.
    #[must_use]
    pub const fn status_confirmation(&self) -> Option<StatusConfirmation> {
        self.status_confirmation
    }
}
impl Report {
    pub(crate) fn approve_status(
        &self,
        row: RowId,
        deviation: Deviation,
        environment: UexEnvironment,
    ) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        let field = next.row_mut(row)?;
        field.status_confirmation = Some(StatusConfirmation::approve(
            field.status.ok_or(InvalidReport)?,
            deviation,
            environment,
        ));
        field.status_deviation = Some(deviation);
        field.status_unreviewed = false;
        field.status_confirmation_revoked = false;
        next.version += 1;
        Ok(next)
    }
}
impl ReportRow {
    /// Reports whether this unresolved status decision has not received keyboard focus.
    #[must_use]
    pub const fn status_unreviewed(&self) -> bool {
        self.status_unreviewed
    }
}
impl Report {
    pub(crate) fn reassess_status(
        &self,
        assessments: &[crate::api::model::StatusAssessment],
        environment: UexEnvironment,
    ) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        for assessment in assessments {
            let field = next.row_mut(assessment.row())?;
            let deviation = assessment.deviation();
            let worsened = field.status_confirmation.is_some_and(|confirmation| {
                environment != confirmation.reference_environment()
                    || deviation.is_worse_than(confirmation.deviation())
            });
            if worsened {
                field.status_confirmation = None;
                field.status_confirmation_revoked = true;
            }
            if worsened
                || (deviation == Deviation::Major
                    && field.status_deviation != Some(Deviation::Major))
            {
                field.status_unreviewed = true;
            }
            field.status_deviation = Some(deviation);
            if !field.status_needs_confirmation() {
                field.status_unreviewed = false;
            }
        }
        if next != *self {
            next.version += 1;
        }
        Ok(next)
    }
}
impl ReportRow {
    /// Returns the last assessed unlowered status deviation.
    #[must_use]
    pub const fn status_deviation(&self) -> Option<Deviation> {
        self.status_deviation
    }
}
impl ReportRow {
    /// Reports an assessed major status deviation without explicit approval.
    #[must_use]
    pub fn status_needs_confirmation(&self) -> bool {
        (self.status_deviation == Some(Deviation::Major) || self.status_confirmation_revoked)
            && self.status_confirmation.is_none()
    }
}
impl Report {
    pub(crate) fn focus_status(&self, row: RowId) -> Result<Self, InvalidReport> {
        let mut next = self.clone();
        let field = next.row_mut(row)?;
        field.status.ok_or(InvalidReport)?;
        if field.status_unreviewed {
            field.status_unreviewed = false;
            next.version += 1;
        }
        Ok(next)
    }
}

impl ReportRow {
    /// Reports that a context change or worsening deviation revoked approval and still requires a decision.
    #[must_use]
    pub const fn status_confirmation_revoked(&self) -> bool {
        self.status_confirmation_revoked
    }
}

impl super::StoredRow {
    pub(super) fn valid_status_decision(&self) -> bool {
        let flags_empty = !self.status_unreviewed && !self.status_confirmation_revoked;
        match self.status {
            None => {
                self.status_confirmation.is_none() && self.status_deviation.is_none() && flags_empty
            }
            Some(value) => match self.status_confirmation {
                Some((approved, deviation, _)) => {
                    approved == value
                        && flags_empty
                        && self
                            .status_deviation
                            .is_some_and(|current| !current.is_worse_than(deviation))
                }
                None => match self.status_deviation {
                    None => flags_empty,
                    Some(deviation) => {
                        !self.status_unreviewed
                            || deviation == Deviation::Major
                            || self.status_confirmation_revoked
                    }
                },
            },
        }
    }
}
