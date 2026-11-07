// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Identity-bound UEX receipts, independent of response order.

use crate::api::model::{InvalidSubmission, SubmissionHeader, SubmissionRequest, SubmissionRow};
use tallyline_shared_kernel::api::model::{CommodityId, RowId, TerminalId, Timestamp};

/// A positive UEX identity verified against one released row and exact attempted wire time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedUexReport {
    id: u64,
    row: SubmissionRow,
    date_added: Timestamp,
    header: std::sync::Arc<SubmissionHeader>,
}
impl VerifiedUexReport {
    /// Matches a lookup record to an unadjusted released observation.
    ///
    /// # Errors
    /// Refuses zero identities or metadata outside the request.
    pub fn from_lookup(
        id: u64,
        terminal: TerminalId,
        commodity: CommodityId,
        date_added: Timestamp,
        request: &SubmissionRequest,
    ) -> Result<Self, InvalidSubmission> {
        Self::from_lookup_at(
            id,
            terminal,
            commodity,
            date_added,
            request,
            Timestamp::from_seconds(request.header().observed_at.seconds()),
        )
    }
    /// Matches a lookup record to the actual earlier-only timestamp sent for this snapshot.
    ///
    /// # Errors
    /// Refuses missing identities, mismatched metadata, fractional seconds or a later attempted time.
    pub fn from_lookup_at(
        id: u64,
        terminal: TerminalId,
        commodity: CommodityId,
        date_added: Timestamp,
        request: &SubmissionRequest,
        sent_at: Timestamp,
    ) -> Result<Self, InvalidSubmission> {
        if request.header().mode != tallyline_shared_kernel::api::model::SubmissionMode::Production
            || id == 0
            || terminal != request.header().terminal
            || date_added != sent_at
            || !sent_at.millis().is_multiple_of(1000)
            || sent_at > request.header().observed_at
        {
            return Err(InvalidSubmission);
        }
        let row = request
            .rows()
            .iter()
            .find(|row| row.commodity() == commodity)
            .ok_or(InvalidSubmission)?;
        Ok(Self {
            id,
            row: row.clone(),
            date_added,
            header: request.shared_header(),
        })
    }
    pub(crate) fn matches_at(&self, request: &SubmissionRequest, sent_at: Timestamp) -> bool {
        *self.header == *request.header()
            && self.date_added == sent_at
            && request.rows().iter().any(|row| row == &self.row)
    }
    /// Returns the positive remote identity, meaningful only with its date.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }
    /// Returns the verified local row identity.
    #[must_use]
    pub const fn row(&self) -> RowId {
        self.row.id()
    }
    /// Returns the UEX partition discriminator retained with the identity.
    #[must_use]
    pub const fn date_added(&self) -> Timestamp {
        self.date_added
    }
}
