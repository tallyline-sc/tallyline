// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Versioned lifecycle persistence bound to immutable inputs and retained positive proofs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tallyline_submission::api::model::{
    RowEditability, StoredLifecycleRowAdmission, StoredSubmissionLifecycle, SubmissionLifecycle,
    SubmissionRepositoryError, UexLifecycleObservation, UexReportStatus, VerifiedUexReport,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleDocument {
    format: u8,
    job_revision: u64,
    source_hash: [u8; 32],
    receipts: Vec<ReceiptIdentityDocument>,
    admitted_at: Option<String>,
    manual_admitted_at: Option<String>,
    observations: Vec<ObservationDocument>,
    row_admissions: Vec<RowAdmissionDocument>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptIdentityDocument {
    id: u64,
    row: u64,
    date_added: String,
}
impl From<&VerifiedUexReport> for ReceiptIdentityDocument {
    fn from(receipt: &VerifiedUexReport) -> Self {
        Self {
            id: receipt.id(),
            row: receipt.row().0,
            date_added: receipt.date_added().millis().to_string(),
        }
    }
}
impl ReceiptIdentityDocument {
    fn restore<'source>(
        &self,
        source: &'source SubmissionLifecycle,
    ) -> Result<&'source VerifiedUexReport, SubmissionRepositoryError> {
        let date_added = crate::reference_cache::timestamp(&self.date_added).map_err(invalid)?;
        source
            .receipts()
            .iter()
            .find(|receipt| {
                receipt.id() == self.id
                    && receipt.row().0 == self.row
                    && receipt.date_added() == date_added
            })
            .ok_or(SubmissionRepositoryError::Invalid)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationDocument {
    receipt: ReceiptIdentityDocument,
    status: String,
    editable: u8,
    checked_at: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowAdmissionDocument {
    row: u64,
    admitted_at: String,
}
fn invalid<T>(_: T) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Invalid
}
fn status_name(status: UexReportStatus) -> &'static str {
    match status {
        UexReportStatus::Pending => "pending",
        UexReportStatus::UnderReview => "under_review",
        UexReportStatus::Queued => "queued",
        UexReportStatus::Approved => "approved",
        UexReportStatus::Consolidated => "consolidated",
        UexReportStatus::Declined => "declined",
        UexReportStatus::Expired => "expired",
    }
}
fn status(value: &str) -> Result<UexReportStatus, SubmissionRepositoryError> {
    match value {
        "pending" => Ok(UexReportStatus::Pending),
        "under_review" => Ok(UexReportStatus::UnderReview),
        "queued" => Ok(UexReportStatus::Queued),
        "approved" => Ok(UexReportStatus::Approved),
        "consolidated" => Ok(UexReportStatus::Consolidated),
        "declined" => Ok(UexReportStatus::Declined),
        "expired" => Ok(UexReportStatus::Expired),
        _ => Err(SubmissionRepositoryError::Invalid),
    }
}
pub(crate) fn encode(lifecycle: &SubmissionLifecycle) -> Result<String, SubmissionRepositoryError> {
    let observations = lifecycle
        .observations()
        .iter()
        .map(|observation| ObservationDocument {
            receipt: observation.receipt().into(),
            status: status_name(observation.status()).to_owned(),
            editable: match observation.editability() {
                RowEditability::Editable => 1,
                RowEditability::NotEditable => 0,
            },
            checked_at: observation.checked_at().millis().to_string(),
        })
        .collect();
    serde_json::to_string(&LifecycleDocument {
        format: 1,
        job_revision: lifecycle.source().revision(),
        source_hash: crate::lifecycle_binding::hash(lifecycle.source())?,
        receipts: lifecycle.receipts().iter().map(Into::into).collect(),
        admitted_at: lifecycle.admitted_at().map(|at| at.millis().to_string()),
        manual_admitted_at: lifecycle
            .manual_admitted_at()
            .map(|at| at.millis().to_string()),
        row_admissions: lifecycle
            .row_admissions()
            .map(|admission| RowAdmissionDocument {
                row: admission.row.0,
                admitted_at: admission.admitted_at.millis().to_string(),
            })
            .collect(),
        observations,
    })
    .map_err(invalid)
}
pub(crate) fn decode(
    source: &SubmissionLifecycle,
    revision: u64,
    payload: &str,
) -> Result<SubmissionLifecycle, SubmissionRepositoryError> {
    let job = source.source();
    let document: LifecycleDocument = serde_json::from_str(payload).map_err(invalid)?;
    if document.format != 1
        || document.job_revision == 0
        || document.job_revision > job.revision()
        || document.source_hash != crate::lifecycle_binding::hash(job)?
        || document.receipts.is_empty()
        || document.receipts.len() > source.receipts().len()
    {
        return Err(SubmissionRepositoryError::Invalid);
    }
    let mut known_rows = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for identity in &document.receipts {
        let receipt = identity.restore(source)?;
        if !known_rows.insert(receipt.row().0)
            || !identities.insert((receipt.id(), receipt.date_added()))
        {
            return Err(SubmissionRepositoryError::Invalid);
        }
    }
    let mut observations = Vec::with_capacity(document.observations.len());
    for observation in document.observations {
        let receipt = observation.receipt.restore(source)?;
        if !known_rows.contains(&receipt.row().0) {
            return Err(SubmissionRepositoryError::Invalid);
        }
        let editability = match observation.editable {
            0 => RowEditability::NotEditable,
            1 => RowEditability::Editable,
            _ => return Err(SubmissionRepositoryError::Invalid),
        };
        observations.push(UexLifecycleObservation::new(
            receipt.clone(),
            status(&observation.status)?,
            editability,
            crate::reference_cache::timestamp(&observation.checked_at).map_err(invalid)?,
        ));
    }
    let admitted_at = document
        .admitted_at
        .as_deref()
        .map(crate::reference_cache::timestamp)
        .transpose()
        .map_err(invalid)?;
    let manual_admitted_at = document
        .manual_admitted_at
        .as_deref()
        .map(crate::reference_cache::timestamp)
        .transpose()
        .map_err(invalid)?;
    let row_admissions = document
        .row_admissions
        .into_iter()
        .map(|admission| {
            if !known_rows.contains(&admission.row) {
                return Err(SubmissionRepositoryError::Invalid);
            }
            Ok(StoredLifecycleRowAdmission {
                row: tallyline_shared_kernel::api::model::RowId(admission.row),
                admitted_at: crate::reference_cache::timestamp(&admission.admitted_at)
                    .map_err(invalid)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    SubmissionLifecycle::reconstitute(
        source,
        StoredSubmissionLifecycle {
            revision,
            admitted_at,
            manual_admitted_at,
            observations,
            row_admissions,
        },
    )
    .map_err(invalid)
}
