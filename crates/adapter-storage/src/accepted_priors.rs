// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable accepted rows and refresh-bounded local projection in the outcome transaction.

use crate::api::{SqliteReferenceCache, StorageError};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tallyline_reference_data::api::AcceptedPriorHistoryStore;
use tallyline_reference_data::api::model::{
    AcceptedLocalPrior, AcceptedPriorContext, AcceptedPriorHistory, EndpointData,
    LocalPriorProjection, LocalPriorRevocation, PriorValues, StoredAcceptedPriorRecord,
};
use tallyline_shared_kernel::api::model::{
    CommodityId, ContainerSize, ReportId, SubmissionJobId, TerminalId, UexEnvironment,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PriorDocument {
    report: u64,
    report_version: u64,
    submission: u64,
    environment: crate::queue_document::EnvironmentDocument,
    terminal: u32,
    side: crate::queue_document::SideDocument,
    game_version: String,
    observed_at: String,
    accepted_at: String,
    commodity: u32,
    price: String,
    stock: String,
    status: Option<std::num::NonZeroU8>,
    container_sizes: Vec<u32>,
}
fn invalid<T>(_: T) -> StorageError {
    StorageError::InvalidCache
}
fn encode(prior: &AcceptedLocalPrior) -> Result<String, StorageError> {
    let context = prior.context();
    let values = prior.values();
    serde_json::to_string(&PriorDocument {
        report: context.report.0,
        report_version: context.report_version,
        submission: context.submission.0,
        environment: context.environment.into(),
        terminal: context.terminal.0,
        side: context.side.into(),
        game_version: context.game_version.clone(),
        observed_at: context.observed_at.millis().to_string(),
        accepted_at: context.accepted_at.millis().to_string(),
        commodity: prior.commodity().0,
        price: values.price.to_string(),
        stock: values.stock.to_string(),
        status: values.status,
        container_sizes: values
            .container_sizes
            .iter()
            .map(|size| size.scu())
            .collect(),
    })
    .map_err(invalid)
}
fn decode(payload: &str) -> Result<AcceptedLocalPrior, StorageError> {
    let document: PriorDocument = serde_json::from_str(payload).map_err(invalid)?;
    AcceptedLocalPrior::new(
        Arc::new(AcceptedPriorContext {
            report: ReportId(document.report),
            report_version: document.report_version,
            submission: SubmissionJobId(document.submission),
            environment: document.environment.into(),
            terminal: TerminalId(document.terminal),
            side: document.side.into(),
            game_version: document.game_version,
            observed_at: crate::reference_cache::timestamp(&document.observed_at)?,
            accepted_at: crate::reference_cache::timestamp(&document.accepted_at)?,
        }),
        CommodityId(document.commodity),
        PriorValues {
            price: document.price.parse().map_err(invalid)?,
            stock: document.stock.parse().map_err(invalid)?,
            status: document.status,
            container_sizes: document
                .container_sizes
                .into_iter()
                .map(|size| ContainerSize::from_scu(size).ok_or(StorageError::InvalidCache))
                .collect::<Result<_, _>>()?,
        },
    )
    .map_err(invalid)
}
fn environment(environment: UexEnvironment) -> &'static str {
    match environment {
        UexEnvironment::Live => "live",
        UexEnvironment::Ptu => "ptu",
    }
}
fn projection(value: &str) -> Result<LocalPriorProjection, StorageError> {
    Ok(match value {
        "active" => LocalPriorProjection::Active,
        "refreshed" => LocalPriorProjection::Refreshed,
        "withdrawn" => LocalPriorProjection::Revoked(LocalPriorRevocation::Withdrawn),
        "declined" => LocalPriorProjection::Revoked(LocalPriorRevocation::Declined),
        "expired" => LocalPriorProjection::Revoked(LocalPriorRevocation::Expired),
        _ => return Err(StorageError::InvalidCache),
    })
}
pub(crate) fn load(connection: &Connection) -> Result<AcceptedPriorHistory, StorageError> {
    let mut statement = connection.prepare("SELECT submission_id,commodity,environment,projection,payload FROM accepted_priors ORDER BY sequence")?;
    let records = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, u32>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut stored = Vec::new();
    let mut expected =
        std::collections::BTreeMap::<SubmissionJobId, Vec<AcceptedLocalPrior>>::new();
    for record in records {
        let (submission, commodity, target, state, payload) = record?;
        let prior = decode(&payload)?;
        if i64::try_from(prior.context().submission.0).map_err(invalid)? != submission
            || prior.commodity().0 != commodity
            || environment(prior.context().environment) != target
        {
            return Err(StorageError::InvalidCache);
        }
        let submission = prior.context().submission;
        if let std::collections::btree_map::Entry::Vacant(entry) = expected.entry(submission) {
            let job = crate::queue::load_job(connection, submission)
                .map_err(invalid)?
                .ok_or(StorageError::InvalidCache)?;
            let rows =
                tallyline_workflows::api::accepted_submission_priors(&job).map_err(invalid)?;
            entry.insert(rows);
        }
        if !expected
            .get(&submission)
            .is_some_and(|rows| rows.contains(&prior))
        {
            return Err(StorageError::InvalidCache);
        }
        stored.push(StoredAcceptedPriorRecord {
            prior,
            projection: projection(&state)?,
        });
    }
    AcceptedPriorHistory::reconstitute(stored).map_err(invalid)
}
pub(crate) fn accepted(
    connection: &Connection,
    rows: &[AcceptedLocalPrior],
) -> Result<(), StorageError> {
    if rows.is_empty() {
        return Ok(());
    }
    let current = load(connection)?;
    let next = current.accepted(rows).map_err(invalid)?;
    for record in next.records().iter().skip(current.records().len()) {
        let prior = record.prior();
        connection.execute("INSERT INTO accepted_priors(submission_id,commodity,environment,projection,payload) VALUES(?1,?2,?3,'active',?4)", (i64::try_from(prior.context().submission.0).map_err(invalid)?, prior.commodity().0, environment(prior.context().environment), encode(prior)?))?;
    }
    Ok(())
}
pub(crate) fn refreshed(connection: &Connection, data: &EndpointData) -> Result<(), StorageError> {
    if matches!(data, EndpointData::TerminalPriors(_)) {
        connection.execute("UPDATE accepted_priors SET projection='refreshed' WHERE environment='live' AND projection='active'", [])?;
    } else {
        connection.execute(
            "UPDATE accepted_priors SET projection='refreshed' WHERE projection='active'",
            [],
        )?;
    }
    Ok(())
}
impl AcceptedPriorHistoryStore for SqliteReferenceCache {
    type Error = StorageError;
    fn accepted_history(&mut self) -> Result<AcceptedPriorHistory, Self::Error> {
        load(&self.connection)
    }
}

fn projection_name(projection: LocalPriorProjection) -> &'static str {
    match projection {
        LocalPriorProjection::Active => "active",
        LocalPriorProjection::Refreshed => "refreshed",
        LocalPriorProjection::Revoked(LocalPriorRevocation::Withdrawn) => "withdrawn",
        LocalPriorProjection::Revoked(LocalPriorRevocation::Declined) => "declined",
        LocalPriorProjection::Revoked(LocalPriorRevocation::Expired) => "expired",
    }
}
pub(crate) fn save_projection(
    connection: &Connection,
    current: &AcceptedPriorHistory,
    next: &AcceptedPriorHistory,
) -> Result<(), StorageError> {
    if current.records().len() != next.records().len() {
        return Err(StorageError::InvalidCache);
    }
    let paired_records = current.records().iter().zip(next.records());
    for (previous, record) in paired_records {
        if previous.prior() != record.prior() {
            return Err(StorageError::InvalidCache);
        }
        if previous.projection() == record.projection() {
            continue;
        }
        let prior = record.prior();
        let changed = connection.execute("UPDATE accepted_priors SET projection=?3 WHERE submission_id=?1 AND commodity=?2 AND projection=?4", (i64::try_from(prior.context().submission.0).map_err(invalid)?, prior.commodity().0, projection_name(record.projection()), projection_name(previous.projection())))?;
        if changed != 1 {
            return Err(StorageError::InvalidCache);
        }
    }
    Ok(())
}
