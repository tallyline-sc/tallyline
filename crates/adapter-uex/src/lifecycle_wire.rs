// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conservative documented status parsing preserves exact remote partition identities.

use serde_json::Value;
use std::collections::BTreeSet;
use tallyline_submission::api::model::{
    LifecycleReadBatch, LifecycleReadFailure, LifecycleReadResponse, RowEditability,
    UexLifecycleRow, UexReportStatus,
};

pub(crate) fn parse(
    batch: LifecycleReadBatch,
    records: &[Value],
) -> Result<LifecycleReadResponse, LifecycleReadFailure> {
    if records.len() > crate::receipt_window::LIMIT
        || !records.iter().all(crate::receipt_window::valid_record)
    {
        return Err(LifecycleReadFailure::InvalidResponse);
    }
    let mut identities = BTreeSet::new();
    let mut rows = Vec::new();
    for record in records {
        let id = record
            .get("id")
            .and_then(crate::submission::number)
            .ok_or(LifecycleReadFailure::InvalidResponse)?;
        let date = record
            .get("date_added")
            .and_then(crate::submission::number)
            .ok_or(LifecycleReadFailure::InvalidResponse)?;
        if !identities.insert((id, date)) {
            return Err(LifecycleReadFailure::InvalidResponse);
        }
        for claim in batch.claims() {
            let matching_targets = claim
                .targets()
                .filter(|target| target.id() == id && target.date_added().seconds() == date);
            for target in matching_targets {
                let request = claim.lifecycle().source().request();
                if crate::submission::lookup(record, id, request, target.date_added()).as_ref()
                    != Some(target)
                {
                    continue;
                }
                let status = match record.get("status").and_then(Value::as_str) {
                    Some("pending") => UexReportStatus::Pending,
                    Some("under_review") => UexReportStatus::UnderReview,
                    Some("queued") => UexReportStatus::Queued,
                    Some("approved") => UexReportStatus::Approved,
                    Some("consolidated") => UexReportStatus::Consolidated,
                    Some("declined") => UexReportStatus::Declined,
                    Some("expired") => UexReportStatus::Expired,
                    _ => return Err(LifecycleReadFailure::InvalidResponse),
                };
                let editable = match record
                    .get("is_editable")
                    .and_then(crate::submission::number)
                {
                    Some(0) => RowEditability::NotEditable,
                    Some(1) => RowEditability::Editable,
                    _ => return Err(LifecycleReadFailure::InvalidResponse),
                };
                rows.push(UexLifecycleRow::new(target.clone(), status, editable));
            }
        }
    }
    LifecycleReadResponse::try_new(batch, rows).map_err(|_| LifecycleReadFailure::InvalidResponse)
}
