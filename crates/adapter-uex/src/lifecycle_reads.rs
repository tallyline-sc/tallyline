// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! One bounded username read provides positive lifecycle rows across admitted source jobs.

use crate::api::UexReader;
use serde_json::Value;
use tallyline_submission::api::SubmissionLifecycleReader;
use tallyline_submission::api::model::{
    CredentialValues, LifecycleReadBatch, LifecycleReadFailure, LifecycleReadResponse,
};

impl SubmissionLifecycleReader for UexReader {
    async fn read_lifecycle(
        &self,
        batch: LifecycleReadBatch,
        credentials: &CredentialValues,
        release_token: Option<&str>,
    ) -> Result<LifecycleReadResponse, LifecycleReadFailure> {
        if !self.token_request_allowed(credentials) {
            return Err(LifecycleReadFailure::Unavailable);
        }
        let token = credentials
            .own_token()
            .or(release_token)
            .ok_or(LifecycleReadFailure::Unavailable)?;
        let read = async {
            let bearer = format!("Bearer {token}");
            let result = self
                .transport
                .get(
                    "data_info/",
                    &[
                        ("type", "commodity"),
                        ("username", batch.account()),
                        ("limit", "100"),
                    ],
                    &[
                        ("Authorization", bearer.as_str()),
                        ("secret-key", credentials.secret_key()),
                    ],
                )
                .await;
            self.observe_read_quota(&result, credentials.token_scope())
                .map_err(|_| LifecycleReadFailure::Unavailable)?;
            let Value::Array(records) = result.map_err(|_| LifecycleReadFailure::Unavailable)?
            else {
                return Err(LifecycleReadFailure::InvalidResponse);
            };
            self.transport
                .process(move || Ok(crate::lifecycle_wire::parse(batch, &records)))
                .await
                .map_err(|_| LifecycleReadFailure::Unavailable)?
        };
        tokio::time::timeout(crate::receipt_window::TIMEOUT, read)
            .await
            .map_err(|_| LifecycleReadFailure::Unavailable)?
    }
}
