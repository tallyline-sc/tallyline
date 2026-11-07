// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed persistence of explicit user statements about unknown delivery.

use serde::{Deserialize, Serialize};
use tallyline_shared_kernel::api::model::{ReportId, RowId, Timestamp};
use tallyline_submission::api::model::{
    ReceiptDecision, SubmissionRepositoryError, SubmissionRequest, SubmissionResolutionAction,
    VerifiedUexReport,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ResolutionActionDocument {
    Split {
        report: u64,
        at: String,
    },
    RowsDuplicated {
        report: u64,
        rows: Vec<u64>,
        at: String,
    },
    RowDecision {
        row: u64,
        decision: DecisionDocument,
        at: String,
    },
    Received {
        at: String,
    },
    NotReceived {
        at: String,
    },
    Duplicated {
        report: u64,
        at: String,
    },
    Verified {
        receipts: Vec<(u64, u64, String)>,
        at: String,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionDocument {
    Received,
    NotReceived,
}
impl From<&SubmissionResolutionAction> for ResolutionActionDocument {
    fn from(value: &SubmissionResolutionAction) -> Self {
        match value {
            SubmissionResolutionAction::Split { report, at } => Self::Split {
                report: report.0,
                at: at.millis().to_string(),
            },
            SubmissionResolutionAction::RowsDuplicated { report, rows, at } => {
                Self::RowsDuplicated {
                    report: report.0,
                    rows: rows.iter().map(|row| row.0).collect(),
                    at: at.millis().to_string(),
                }
            }
            SubmissionResolutionAction::RowDecision { row, decision, at } => Self::RowDecision {
                row: row.0,
                decision: match decision {
                    ReceiptDecision::Received => DecisionDocument::Received,
                    ReceiptDecision::NotReceived => DecisionDocument::NotReceived,
                },
                at: at.millis().to_string(),
            },
            SubmissionResolutionAction::Verified { receipts, at } => Self::Verified {
                receipts: receipts
                    .iter()
                    .map(|receipt| {
                        (
                            receipt.row().0,
                            receipt.id(),
                            receipt.date_added().millis().to_string(),
                        )
                    })
                    .collect(),
                at: at.millis().to_string(),
            },
            SubmissionResolutionAction::Duplicated { report, at } => Self::Duplicated {
                report: report.0,
                at: at.millis().to_string(),
            },
            SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::Received,
                at,
            } => Self::Received {
                at: at.millis().to_string(),
            },
            SubmissionResolutionAction::Decision {
                decision: ReceiptDecision::NotReceived,
                at,
            } => Self::NotReceived {
                at: at.millis().to_string(),
            },
        }
    }
}
impl ResolutionActionDocument {
    pub(crate) fn restore(
        self,
        request: &SubmissionRequest,
        sent_at: Option<Timestamp>,
    ) -> Result<SubmissionResolutionAction, SubmissionRepositoryError> {
        let (decision, at) = match self {
            Self::Split { report, at } => {
                return Ok(SubmissionResolutionAction::Split {
                    report: ReportId(report),
                    at: crate::reference_cache::timestamp(&at)
                        .map_err(|_| SubmissionRepositoryError::Invalid)?,
                });
            }
            Self::RowsDuplicated { report, rows, at } => {
                return Ok(SubmissionResolutionAction::RowsDuplicated {
                    report: ReportId(report),
                    rows: rows.into_iter().map(RowId).collect(),
                    at: crate::reference_cache::timestamp(&at)
                        .map_err(|_| SubmissionRepositoryError::Invalid)?,
                });
            }
            Self::RowDecision { row, decision, at } => {
                return Ok(SubmissionResolutionAction::RowDecision {
                    row: RowId(row),
                    decision: match decision {
                        DecisionDocument::Received => ReceiptDecision::Received,
                        DecisionDocument::NotReceived => ReceiptDecision::NotReceived,
                    },
                    at: crate::reference_cache::timestamp(&at)
                        .map_err(|_| SubmissionRepositoryError::Invalid)?,
                });
            }
            Self::Verified { receipts, at } => {
                let sent_at = sent_at.ok_or(SubmissionRepositoryError::Invalid)?;
                let receipts = receipts
                    .into_iter()
                    .map(|(row, id, date)| {
                        let row = request
                            .rows()
                            .iter()
                            .find(|candidate| candidate.id().0 == row)
                            .ok_or(SubmissionRepositoryError::Invalid)?;
                        VerifiedUexReport::from_lookup_at(
                            id,
                            request.header().terminal,
                            row.commodity(),
                            crate::reference_cache::timestamp(&date)
                                .map_err(|_| SubmissionRepositoryError::Invalid)?,
                            request,
                            sent_at,
                        )
                        .map_err(|_| SubmissionRepositoryError::Invalid)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(SubmissionResolutionAction::Verified {
                    receipts,
                    at: crate::reference_cache::timestamp(&at)
                        .map_err(|_| SubmissionRepositoryError::Invalid)?,
                });
            }
            Self::Duplicated { report, at } => {
                return Ok(SubmissionResolutionAction::Duplicated {
                    report: ReportId(report),
                    at: crate::reference_cache::timestamp(&at)
                        .map_err(|_| SubmissionRepositoryError::Invalid)?,
                });
            }
            Self::Received { at } => (ReceiptDecision::Received, at),
            Self::NotReceived { at } => (ReceiptDecision::NotReceived, at),
        };
        let at = crate::reference_cache::timestamp(&at)
            .map_err(|_| SubmissionRepositoryError::Invalid)?;
        Ok(SubmissionResolutionAction::Decision { decision, at })
    }
}
