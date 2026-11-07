// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Idempotent authenticated-read quota evidence independent of submission history.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{
    SubmissionRepositoryError, TokenQuotaObservation, TokenQuotaReset, TokenScope,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationDocument {
    scope: Option<[u8; 32]>,
    reset: Option<u64>,
    observed_at: Option<u64>,
}
fn unavailable(_: rusqlite::Error) -> SubmissionRepositoryError {
    SubmissionRepositoryError::Unavailable
}
fn document(observation: TokenQuotaObservation) -> ObservationDocument {
    ObservationDocument {
        scope: observation.scope().map(|scope| *scope.bytes()),
        reset: observation.reset().map(|reset| reset.reset().seconds()),
        observed_at: observation
            .reset()
            .and_then(TokenQuotaReset::observed_at)
            .map(|at| at.seconds()),
    }
}
fn decode(payload: &str) -> Result<TokenQuotaObservation, SubmissionRepositoryError> {
    let value: ObservationDocument =
        serde_json::from_str(payload).map_err(|_| SubmissionRepositoryError::Invalid)?;
    let scope = value
        .scope
        .map(|scope| TokenScope::new(scope).ok_or(SubmissionRepositoryError::Invalid))
        .transpose()?;
    if value.reset == Some(0) || value.reset.is_none() && value.observed_at.is_some() {
        return Err(SubmissionRepositoryError::Invalid);
    }
    let reset = value.reset.map(|reset| {
        TokenQuotaReset::new(
            Timestamp::from_seconds(reset),
            value.observed_at.map(Timestamp::from_seconds),
        )
    });
    Ok(TokenQuotaObservation::new(scope, reset))
}
pub(crate) fn record(
    connection: &Connection,
    observation: TokenQuotaObservation,
) -> Result<(), SubmissionRepositoryError> {
    let payload = serde_json::to_string(&document(observation))
        .map_err(|_| SubmissionRepositoryError::Invalid)?;
    decode(&payload)?;
    connection.execute("INSERT INTO authenticated_token_quotas(payload) VALUES(?1) ON CONFLICT(payload) DO NOTHING",[payload]).map_err(unavailable)?;
    Ok(())
}
pub(crate) fn load(
    connection: &Connection,
) -> Result<Vec<TokenQuotaObservation>, SubmissionRepositoryError> {
    let mut statement = connection
        .prepare(
            "SELECT payload FROM authenticated_token_quotas WHERE retired_at IS NULL ORDER BY id",
        )
        .map_err(unavailable)?;
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(unavailable)?
        .map(|payload| decode(&payload.map_err(unavailable)?))
        .collect()
}
pub(crate) fn allows(
    connection: &Connection,
    proof: &tallyline_submission::api::model::TokenAdmission,
) -> Result<bool, SubmissionRepositoryError> {
    Ok(!crate::submission_service_fences::load(connection)?
        .iter()
        .any(|fence| fence.blocks_admission(proof))
        && !load(connection)?
            .iter()
            .any(|observation| observation.blocks_admission(proof)))
}
pub(crate) fn expire(
    connection: &Connection,
    now: Timestamp,
) -> Result<usize, SubmissionRepositoryError> {
    let mut changed = 0;
    for observation in load(connection)? {
        if observation
            .reset()
            .and_then(TokenQuotaReset::usable_reset)
            .is_some_and(|reset| reset <= now)
        {
            let payload = serde_json::to_string(&document(observation))
                .map_err(|_| SubmissionRepositoryError::Invalid)?;
            changed+=connection.execute("UPDATE authenticated_token_quotas SET retired_at=?1 WHERE payload=?2 AND retired_at IS NULL",(now.millis().to_string(),payload)).map_err(unavailable)?;
        }
    }
    Ok(changed)
}
