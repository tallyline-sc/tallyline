// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Positive lifecycle evidence for one exactly verified remote row.

use crate::api::model::VerifiedUexReport;
use tallyline_shared_kernel::api::model::Timestamp;

/// The seven documented lifecycle states of a UEX report row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UexReportStatus {
    /// Waiting for automatic approval.
    Pending,
    /// Awaiting a moderator's decision.
    UnderReview,
    /// Parked until the reported PTU version goes live.
    Queued,
    /// Cleared and awaiting consolidation.
    Approved,
    /// Incorporated into the live prices.
    Consolidated,
    /// Rejected as inconsistent or invalid.
    Declined,
    /// Expired before approval.
    Expired,
}

impl UexReportStatus {
    /// Reports whether polling stops for this positively observed row.
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(self, Self::Consolidated | Self::Declined | Self::Expired)
    }
}

/// Whether the exact observed row is currently editable at UEX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowEditability {
    /// UEX positively reported its editable flag as one.
    Editable,
    /// UEX positively reported its editable flag as zero.
    NotEditable,
}

/// The requested source of a bounded lifecycle refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleRefresh {
    /// The background lifecycle cadence became eligible.
    Automatic,
    /// The user explicitly requested fresh lifecycle evidence.
    Manual,
}

/// Documented lifecycle values positively bound to an exact verified remote row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UexLifecycleRow {
    receipt: VerifiedUexReport,
    status: UexReportStatus,
    editability: RowEditability,
}
impl UexLifecycleRow {
    /// Binds documented status and editability to an exact verified row.
    #[must_use]
    pub const fn new(
        receipt: VerifiedUexReport,
        status: UexReportStatus,
        editability: RowEditability,
    ) -> Self {
        Self {
            receipt,
            status,
            editability,
        }
    }
    /// Borrows the exact remote identity and released row proof.
    #[must_use]
    pub const fn receipt(&self) -> &VerifiedUexReport {
        &self.receipt
    }
    /// Returns the positively observed remote status.
    #[must_use]
    pub const fn status(&self) -> UexReportStatus {
        self.status
    }
    /// Returns the positively observed editability flag.
    #[must_use]
    pub const fn editability(&self) -> RowEditability {
        self.editability
    }
    pub(crate) const fn checked(self, checked_at: Timestamp) -> UexLifecycleObservation {
        UexLifecycleObservation {
            row: self,
            checked_at,
        }
    }
}

/// A positive row observation with the controller-observed completion time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UexLifecycleObservation {
    row: UexLifecycleRow,
    checked_at: Timestamp,
}
impl UexLifecycleObservation {
    /// Binds documented lifecycle values to a verified row and the local check time.
    #[must_use]
    pub const fn new(
        receipt: VerifiedUexReport,
        status: UexReportStatus,
        editability: RowEditability,
        checked_at: Timestamp,
    ) -> Self {
        UexLifecycleRow::new(receipt, status, editability).checked(checked_at)
    }
    /// Borrows the exact remote identity and released row proof.
    #[must_use]
    pub const fn receipt(&self) -> &VerifiedUexReport {
        self.row.receipt()
    }
    /// Returns the positively observed remote status.
    #[must_use]
    pub const fn status(&self) -> UexReportStatus {
        self.row.status()
    }
    /// Returns the positively observed editability flag.
    #[must_use]
    pub const fn editability(&self) -> RowEditability {
        self.row.editability()
    }
    /// Returns when the controller consumed and checked the completed read.
    #[must_use]
    pub const fn checked_at(&self) -> Timestamp {
        self.checked_at
    }
}
