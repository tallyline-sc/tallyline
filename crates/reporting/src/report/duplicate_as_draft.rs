// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit immutable-origin copying into separate unreviewed Drafts.

use super::{InvalidReport, Report, ReportId, ReportRow};
use crate::api::AttachmentEdit;
use crate::api::model::ReportState;
use std::collections::BTreeSet;
use tallyline_shared_kernel::api::model::RowId;

impl Report {
    /// Copies an eligible immutable origin into a separate unreviewed Draft.
    ///
    /// # Errors
    /// Refuses another source phase or a reused or missing identity.
    pub fn duplicate_as_draft(&self, id: ReportId) -> Result<Self, InvalidReport> {
        if !matches!(
            self.state(),
            ReportState::OutcomeUnknown | ReportState::Rejected
        ) {
            return Err(InvalidReport);
        }
        self.copied_draft(id, self.rows())
    }
    /// Copies the workflow-selected resolved not-accepted subset into a separate unreviewed Draft.
    ///
    /// # Errors
    /// Refuses another source phase, duplicate or foreign rows and a reused or missing identity.
    pub fn duplicate_partial_rows_as_draft(
        &self,
        id: ReportId,
        rows: &[RowId],
    ) -> Result<Self, InvalidReport> {
        let unique: BTreeSet<_> = rows.iter().map(|row| row.0).collect();
        if self.state() != ReportState::PartiallyAccepted
            || rows.is_empty()
            || unique.len() != rows.len()
            || rows
                .iter()
                .any(|row| !self.rows().iter().any(|source| source.id() == *row))
        {
            return Err(InvalidReport);
        }
        self.copied_draft(
            id,
            self.rows()
                .iter()
                .filter(|row| unique.contains(&row.id().0)),
        )
    }
    fn copied_draft<'a>(
        &self,
        id: ReportId,
        rows: impl IntoIterator<Item = &'a ReportRow>,
    ) -> Result<Self, InvalidReport> {
        if id == self.id() {
            return Err(InvalidReport);
        }
        let mut draft = Self::manual(id, self.key(), self.observed_at(), self.game_version())?;
        draft = draft.set_faction_affinity(self.faction_affinity())?;
        for source in rows {
            let row = draft.add_commodity(source.commodity())?;
            draft.set_price(row, source.price())?;
            draft.set_stock(row, source.stock())?;
            draft.set_status(row, source.status())?;
        }
        if let Some(attachment) = self.attachment() {
            draft.edit_attachment(AttachmentEdit::Select(
                attachment.binding(),
                attachment.time(),
            ))?;
        }
        Ok(draft)
    }
}
