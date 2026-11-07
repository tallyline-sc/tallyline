// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only account recheck after a submission key refusal.

use crate::api::{ReadError, SecretKey, UexReader};
use tallyline_submission::api::model::{CredentialValues, SubmissionPauseReason};

impl UexReader {
    pub(crate) async fn recheck_submission_account(
        &self,
        credentials: &CredentialValues,
        expected_account: &str,
    ) -> SubmissionPauseReason {
        match self
            .account(&SecretKey::new(credentials.secret_key()))
            .await
        {
            Ok(account)
                if account.username == expected_account
                    && account.is_datarunner
                    && !account.is_datarunner_banned =>
            {
                SubmissionPauseReason::KeyChecked
            }
            Ok(_) => SubmissionPauseReason::Reauthenticate,
            Err(ReadError::Rejected {
                http_status: 401 | 403,
                ..
            }) => SubmissionPauseReason::Reauthenticate,
            Err(_) => SubmissionPauseReason::KeyCheckUnavailable,
        }
    }
}
