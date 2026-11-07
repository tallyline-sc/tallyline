// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only credential validation with closed, redacted failures.

use crate::api::{AppToken, ReadError, SecretKey, UexReader};
use tallyline_submission::api::CredentialValidator;
use tallyline_submission::api::model::{CredentialFailure, CredentialValues, ValidatedAccount};

impl CredentialValidator for UexReader {
    async fn validate(
        &self,
        values: &CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        let key = SecretKey::new(values.secret_key());
        let account = self.eligible_account(values).await?;
        let token = values.own_token().ok_or(CredentialFailure::MissingToken)?;
        if !self.token_request_allowed(values) {
            return Err(CredentialFailure::Unavailable);
        }
        let access = self
            .report_access(&AppToken::new(token), &key, &account.username)
            .await;
        self.observe_read_quota(&access, values.token_scope())
            .map_err(|_| CredentialFailure::Unavailable)?;
        access.map_err(|error| failure(error, Check::Token))?;
        Ok(ValidatedAccount::new(account.username))
    }
}

impl UexReader {
    async fn eligible_account(
        &self,
        values: &CredentialValues,
    ) -> Result<crate::api::model::UexAccount, CredentialFailure> {
        let account = self
            .account(&SecretKey::new(values.secret_key()))
            .await
            .map_err(|error| failure(error, Check::Account))?;
        if !account.is_datarunner || account.is_datarunner_banned {
            return Err(CredentialFailure::NotAllowed);
        }
        Ok(account)
    }
    /// Performs the tokenless account check without issuing a held token-validation request.
    pub async fn check_account_while_token_held(
        &self,
        values: &CredentialValues,
    ) -> CredentialFailure {
        match self.eligible_account(values).await {
            Ok(_) => CredentialFailure::Unavailable,
            Err(failure) => failure,
        }
    }
}

enum Check {
    Account,
    Token,
}

fn failure(error: ReadError, check: Check) -> CredentialFailure {
    match error {
        ReadError::Rejected {
            http_status,
            code,
            message: _,
            data: _,
        } => match code.as_str() {
            "missing_secret_key" | "invalid_secret_key" | "user_not_found" => {
                CredentialFailure::InvalidKey
            }
            "user_not_allowed" | "user_disabled" | "user_not_verified" => {
                CredentialFailure::NotAllowed
            }
            "upgrade_required" => CredentialFailure::UpdateRequired,
            "missing_client_version" => CredentialFailure::ClientVersionMissing,
            "no_api_found" | "not_allowed" | "access_denied" => match check {
                Check::Token => CredentialFailure::InvalidToken,
                Check::Account => CredentialFailure::InvalidKey,
            },
            _ if http_status == 401 || http_status == 403 => match check {
                Check::Account => CredentialFailure::InvalidKey,
                Check::Token => CredentialFailure::InvalidToken,
            },
            _ => CredentialFailure::Unavailable,
        },
        ReadError::NotJson { .. }
        | ReadError::Redirect { .. }
        | ReadError::Malformed { .. }
        | ReadError::BodyTooLarge { .. }
        | ReadError::Transport(_) => CredentialFailure::Unavailable,
    }
}
