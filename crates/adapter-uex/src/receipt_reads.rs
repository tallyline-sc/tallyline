// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Account-scoped bounded receipt reads with shared exact-row proof parsing.

use crate::api::UexReader;
use crate::receipt_window::ReceiptReadFailure;
use serde_json::Value;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{CredentialValues, SubmissionRequest, VerifiedUexReport};

pub(crate) struct ReceiptWindow {
    pub(crate) receipts: Vec<VerifiedUexReport>,
    pub(crate) complete: bool,
}
impl UexReader {
    pub(crate) async fn read_receipt_window(
        &self,
        request: &SubmissionRequest,
        date_added: Timestamp,
        credentials: &CredentialValues,
        token: &str,
        id: Option<u64>,
    ) -> Result<ReceiptWindow, ReceiptReadFailure> {
        if !self.token_request_allowed(credentials) {
            return Err(ReceiptReadFailure::Unavailable);
        }
        let terminal = request.header().terminal.0.to_string();
        let identity = id.map(|value| value.to_string());
        let mut filters = vec![
            ("type", "commodity"),
            ("id_terminal", terminal.as_str()),
            ("username", request.header().account.as_str()),
            ("limit", "100"),
        ];
        if let Some(identity) = identity.as_deref() {
            filters.push(("id", identity));
        }
        let bearer = format!("Bearer {token}");
        let read = async {
            let result = self
                .transport
                .get(
                    "data_info/",
                    &filters,
                    &[
                        ("Authorization", bearer.as_str()),
                        ("secret-key", credentials.secret_key()),
                    ],
                )
                .await;
            self.observe_read_quota(&result, credentials.token_scope())
                .map_err(|_| ReceiptReadFailure::Unavailable)?;
            let Ok(Value::Array(records)) = result else {
                return Err(ReceiptReadFailure::Unavailable);
            };
            let snapshot = request.clone();
            self.transport
                .process(move || {
                    Ok(
                        crate::receipt_window::proofs(&records, &snapshot, date_added).map(
                            |mut receipts| {
                                if let Some(id) = id {
                                    receipts.retain(|proof| proof.id() == id);
                                }
                                ReceiptWindow {
                                    receipts,
                                    complete: records.len() < crate::receipt_window::LIMIT,
                                }
                            },
                        ),
                    )
                })
                .await
                .map_err(|_| ReceiptReadFailure::Unavailable)?
        };
        tokio::time::timeout(crate::receipt_window::TIMEOUT, read)
            .await
            .map_err(|_| ReceiptReadFailure::Unavailable)?
    }
}
