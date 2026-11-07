// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared bounded data-info window validation.

use serde_json::Value;
use std::collections::BTreeSet;
use std::time::Duration;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{SubmissionRequest, VerifiedUexReport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReceiptReadFailure {
    Unavailable,
    InvalidWindow,
    Ambiguous,
}
pub(crate) const LIMIT: usize = 100;
pub(crate) const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn valid_record(record: &Value) -> bool {
    record.get("type").and_then(Value::as_str) == Some("commodity")
        && record
            .get("id")
            .and_then(crate::submission::number)
            .is_some_and(|id| id > 0)
        && record
            .get("id_terminal")
            .and_then(crate::submission::number)
            .is_some_and(|id| id > 0 && u32::try_from(id).is_ok())
        && record
            .get("id_commodity")
            .and_then(crate::submission::number)
            .is_some_and(|id| id > 0 && u32::try_from(id).is_ok())
        && record
            .get("date_added")
            .and_then(crate::submission::number)
            .is_some()
}

pub(crate) fn valid(records: &[Value], request: &SubmissionRequest) -> bool {
    records.len() <= LIMIT
        && records.iter().all(|record| {
            valid_record(record)
                && record
                    .get("id_terminal")
                    .and_then(crate::submission::number)
                    == Some(u64::from(request.header().terminal.0))
        })
}
pub(crate) fn proofs(
    records: &[Value],
    request: &SubmissionRequest,
    date_added: Timestamp,
) -> Result<Vec<VerifiedUexReport>, ReceiptReadFailure> {
    if !valid(records, request) {
        return Err(ReceiptReadFailure::InvalidWindow);
    }
    let mut rows = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut proofs = Vec::new();
    for record in records {
        if let Some(proof) = record
            .get("id")
            .and_then(crate::submission::number)
            .and_then(|id| crate::submission::lookup(record, id, request, date_added))
        {
            if !rows.insert(proof.row().0) || !identities.insert(proof.id()) {
                return Err(ReceiptReadFailure::Ambiguous);
            }
            proofs.push(proof);
        }
    }
    Ok(proofs)
}
