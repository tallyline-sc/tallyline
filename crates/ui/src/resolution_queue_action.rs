// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Domain-derived explicit receipt-resolution intent without rendering or I/O.

use tallyline_shared_kernel::api::model::RowId;
use tallyline_submission::api::model::{ReceiptDecision, SubmissionJob};
/// An explicit user action without permission to replay the original POST.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionQueueAction {
    /// Records a statement independently of positive remote proof.
    Decide(ReceiptDecision),
    /// Records an explicit statement for one unresolved partial row.
    DecideRow {
        /// The exact immutable local row.
        row: RowId,
        /// The user's statement without a remote identity.
        decision: ReceiptDecision,
    },
    /// Reads bounded positive remote evidence for the original attempt.
    CheckReceipt,
    /// Creates a separate unreviewed Draft from eligible immutable submission history.
    DuplicateAsDraft,
}
impl ResolutionQueueAction {
    pub(crate) fn eligible(self, job: &SubmissionJob) -> bool {
        match self {
            Self::Decide(_) => job.can_decide_unknown(),
            Self::DecideRow { row, .. } => job.can_decide_row(row),
            Self::CheckReceipt => job.can_check_receipts(),
            Self::DuplicateAsDraft => {
                job.can_duplicate_unknown()
                    || job.can_duplicate_rejected()
                    || job.partial_copy_rows().is_some()
            }
        }
    }
}
