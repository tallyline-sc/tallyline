// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Forward cache migration within the caller's SQLite transaction.

use crate::api::StorageError;
use crate::reference_cache::{decode, encode};
use rusqlite::{Connection, OptionalExtension};
use tallyline_reference_data::api::model::{EndpointData, Terminal, UexEnvironment};

pub(crate) fn schema_two(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch("CREATE TABLE ref_terminals(id INTEGER PRIMARY KEY,name TEXT NOT NULL,nickname TEXT NOT NULL,display_name TEXT NOT NULL,location TEXT NOT NULL,available_live INTEGER NOT NULL,max_container_size INTEGER CHECK(max_container_size IS NULL OR max_container_size>0)); CREATE INDEX ref_terminals_name ON ref_terminals(name); CREATE INDEX ref_terminals_nickname ON ref_terminals(nickname); CREATE INDEX ref_terminals_display_name ON ref_terminals(display_name); CREATE INDEX ref_terminals_location ON ref_terminals(location);")?;
    for endpoint in ["commodities", "terminals"] {
        let payload: Option<String> = connection
            .query_row(
                "SELECT payload FROM ref_cache WHERE endpoint=?1",
                [endpoint],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(payload) = payload {
            let mapped = decode(endpoint, &payload)?;
            if let EndpointData::Terminals(terminals) = &mapped {
                index_terminals(connection, terminals)?;
            }
            let (_, payload) = encode(&mapped)?;
            connection.execute(
                "UPDATE ref_cache SET payload=?1 WHERE endpoint=?2",
                (&payload, endpoint),
            )?;
        }
    }
    connection.pragma_update(None, "user_version", 2)?;
    Ok(())
}

pub(crate) fn index_terminals(
    connection: &Connection,
    terminals: &[Terminal],
) -> Result<(), StorageError> {
    connection.execute("DELETE FROM ref_terminals", [])?;
    for terminal in terminals {
        let text = terminal.text();
        connection.execute("INSERT INTO ref_terminals(id,name,nickname,display_name,location,available_live,max_container_size) VALUES(?1,?2,?3,?4,?5,?6,?7)",(terminal.id().0,&text.name,&text.nickname,&text.display_name,&text.location,terminal.available(UexEnvironment::Live),terminal.max_container_size().map(std::num::NonZeroU32::get)))?;
    }
    Ok(())
}

pub(crate) fn schema_three(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch("CREATE TABLE reports(id INTEGER PRIMARY KEY AUTOINCREMENT,version INTEGER NOT NULL CHECK(version>=0),payload TEXT NOT NULL); CREATE TABLE report_recents(report_id INTEGER PRIMARY KEY REFERENCES reports(id),changed_order INTEGER NOT NULL); PRAGMA user_version=3;")?;
    Ok(())
}

pub(crate) fn schema_four(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection.prepare("SELECT id,payload FROM reports")?;
    let records = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, payload) in records {
        let mut document: serde_json::Value =
            serde_json::from_str(&payload).map_err(|_| StorageError::InvalidCache)?;
        let rows = document
            .get_mut("rows")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or(StorageError::InvalidCache)?;
        for row in rows {
            let row = row.as_object_mut().ok_or(StorageError::InvalidCache)?;
            row.insert("status_confirmation".to_owned(), serde_json::Value::Null);
            row.insert("status_deviation".to_owned(), serde_json::Value::Null);
            row.insert(
                "status_confirmation_revoked".to_owned(),
                serde_json::Value::Bool(false),
            );
            row.insert(
                "status_unreviewed".to_owned(),
                serde_json::Value::Bool(false),
            );
        }
        connection.execute(
            "UPDATE reports SET payload=?1 WHERE id=?2",
            (document.to_string(), id),
        )?;
    }
    connection.pragma_update(None, "user_version", 4)?;
    Ok(())
}

pub(crate) fn schema_five(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection.prepare("SELECT id,payload FROM reports")?;
    let records = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, payload) in records {
        let mut document: serde_json::Value =
            serde_json::from_str(&payload).map_err(|_| StorageError::InvalidCache)?;
        document
            .as_object_mut()
            .ok_or(StorageError::InvalidCache)?
            .insert("attachment".to_owned(), serde_json::Value::Null);
        connection.execute(
            "UPDATE reports SET payload=?1 WHERE id=?2",
            (document.to_string(), id),
        )?;
    }
    connection.pragma_update(None, "user_version", 5)?;
    Ok(())
}

pub(crate) fn schema_six(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection.prepare("SELECT id,payload FROM reports")?;
    let records = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, payload) in records {
        let mut document: serde_json::Value =
            serde_json::from_str(&payload).map_err(|_| StorageError::InvalidCache)?;
        let attachment = document
            .get_mut("attachment")
            .ok_or(StorageError::InvalidCache)?;
        if !attachment.is_null() {
            attachment
                .as_object_mut()
                .ok_or(StorageError::InvalidCache)?
                .insert("working_copy".to_owned(), serde_json::Value::Null);
        }
        connection.execute(
            "UPDATE reports SET payload=?1 WHERE id=?2",
            (document.to_string(), id),
        )?;
    }
    connection.execute_batch("CREATE TABLE screenshot_requirements(account_digest BLOB PRIMARY KEY CHECK(length(account_digest)=32),observed_at TEXT NOT NULL); PRAGMA user_version=6;")?;
    Ok(())
}

pub(crate) fn schema_seven(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection.prepare("SELECT id,payload FROM reports")?;
    let records = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, payload) in records {
        let mut document: serde_json::Value =
            serde_json::from_str(&payload).map_err(|_| StorageError::InvalidCache)?;
        let rows = document
            .get_mut("rows")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or(StorageError::InvalidCache)?;
        for row in rows {
            row.as_object_mut().ok_or(StorageError::InvalidCache)?.insert("price_decision".to_owned(), serde_json::json!({"confirmation": null, "deviation": null, "unreviewed": false, "revoked": false}));
        }
        connection.execute(
            "UPDATE reports SET payload=?1 WHERE id=?2",
            (document.to_string(), id),
        )?;
    }
    connection.pragma_update(None, "user_version", 7)?;
    Ok(())
}

pub(crate) fn schema_eight(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection.prepare("SELECT id,payload FROM reports")?;
    let records = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, payload) in records {
        let mut document: serde_json::Value =
            serde_json::from_str(&payload).map_err(|_| StorageError::InvalidCache)?;
        let rows = document
            .get_mut("rows")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or(StorageError::InvalidCache)?;
        for row in rows {
            row.as_object_mut().ok_or(StorageError::InvalidCache)?.insert(
                "stock_decision".to_owned(),
                serde_json::json!({"confirmation":null,"deviation":null,"unreviewed":false,"revoked":false}),
            );
        }
        connection.execute(
            "UPDATE reports SET payload=?1 WHERE id=?2",
            (document.to_string(), id),
        )?;
    }
    connection.pragma_update(None, "user_version", 8)?;
    Ok(())
}

pub(crate) fn schema_nine(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection.prepare("SELECT id,payload FROM reports")?;
    let records = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, payload) in records {
        let mut document: serde_json::Value =
            serde_json::from_str(&payload).map_err(|_| StorageError::InvalidCache)?;
        let object = document.as_object_mut().ok_or(StorageError::InvalidCache)?;
        let lifecycle = serde_json::to_value(crate::report_lifecycle::LifecycleDocument::from(
            tallyline_reporting::api::model::StoredReportLifecycle::default(),
        ))
        .map_err(|_| StorageError::InvalidCache)?;
        if object.insert("lifecycle".to_owned(), lifecycle).is_some() {
            return Err(StorageError::InvalidCache);
        }
        connection.execute(
            "UPDATE reports SET payload=?1 WHERE id=?2",
            (document.to_string(), id),
        )?;
    }
    connection.execute_batch("ALTER TABLE reports ADD COLUMN phase TEXT NOT NULL DEFAULT 'draft'; CREATE TABLE submission_jobs(id INTEGER PRIMARY KEY AUTOINCREMENT,report_id INTEGER NOT NULL REFERENCES reports(id),revision INTEGER NOT NULL CHECK(revision>0),phase TEXT NOT NULL,payload TEXT NOT NULL,release_report TEXT NOT NULL,release_report_version INTEGER NOT NULL CHECK(release_report_version>0)); CREATE UNIQUE INDEX submission_one_sending ON submission_jobs((1)) WHERE phase='sending'; CREATE TABLE domain_outbox(event_source TEXT NOT NULL,aggregate_id INTEGER NOT NULL,revision INTEGER NOT NULL,event_kind TEXT NOT NULL,delivered INTEGER NOT NULL DEFAULT 0 CHECK(delivered IN(0,1)),PRIMARY KEY(event_source,aggregate_id,revision)); CREATE TABLE accepted_priors(sequence INTEGER PRIMARY KEY AUTOINCREMENT,submission_id INTEGER NOT NULL REFERENCES submission_jobs(id),commodity INTEGER NOT NULL,environment TEXT NOT NULL,projection TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(submission_id,commodity)); CREATE INDEX accepted_priors_projection ON accepted_priors(environment,projection); CREATE TABLE submission_receipt_enrichment(job_id INTEGER PRIMARY KEY REFERENCES submission_jobs(id),job_revision INTEGER NOT NULL CHECK(job_revision>0),payload TEXT); PRAGMA user_version=9;")?;
    Ok(())
}

pub(crate) fn schema_ten(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch("CREATE TABLE submission_service_fences(cause TEXT NOT NULL CHECK(cause IN('api_changed','daily_quota','account_pause')),source_job INTEGER NOT NULL REFERENCES submission_jobs(id),source_revision INTEGER NOT NULL CHECK(source_revision>0),PRIMARY KEY(cause,source_job));")?;
    connection.pragma_update(None, "user_version", 10)?;
    Ok(())
}

pub(crate) fn schema_eleven(connection: &Connection) -> Result<(), StorageError> {
    connection.pragma_update(None, "user_version", 11)?;
    Ok(())
}

pub(crate) fn schema_twelve(connection: &Connection) -> Result<(), StorageError> {
    connection.pragma_update(None, "user_version", 12)?;
    Ok(())
}

pub(crate) fn schema_thirteen(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch("ALTER TABLE submission_service_fences ADD COLUMN token_scope BLOB; ALTER TABLE submission_service_fences ADD COLUMN reset_at TEXT; CREATE TABLE submission_token_bindings(job_id INTEGER NOT NULL REFERENCES submission_jobs(id),attempt INTEGER NOT NULL CHECK(attempt>0),token_scope BLOB CHECK(token_scope IS NULL OR length(token_scope)=32),PRIMARY KEY(job_id,attempt)); CREATE TABLE authenticated_token_quotas(id INTEGER PRIMARY KEY AUTOINCREMENT,payload TEXT NOT NULL UNIQUE,retired_at TEXT); CREATE TABLE submission_quota_expiries(source_job INTEGER NOT NULL REFERENCES submission_jobs(id),source_revision INTEGER NOT NULL CHECK(source_revision>0),expired_at TEXT NOT NULL,token_scope BLOB,reset_at TEXT NOT NULL,PRIMARY KEY(source_job,source_revision)); PRAGMA user_version=13;")?;
    Ok(())
}

pub(crate) fn schema_fourteen(connection: &Connection) -> Result<(), StorageError> {
    connection.pragma_update(None, "user_version", 14)?;
    Ok(())
}

pub(crate) fn schema_fifteen(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch("CREATE TABLE submission_splits(source_job INTEGER PRIMARY KEY REFERENCES submission_jobs(id),ready_job INTEGER NOT NULL UNIQUE REFERENCES submission_jobs(id),cooling_job INTEGER NOT NULL UNIQUE REFERENCES submission_jobs(id),created_at TEXT NOT NULL,CHECK(source_job<>ready_job AND source_job<>cooling_job AND ready_job<>cooling_job)); PRAGMA user_version=15;")?;
    Ok(())
}

pub(crate) fn schema_sixteen(connection: &Connection) -> Result<(), StorageError> {
    connection.pragma_update(None, "user_version", 16)?;
    Ok(())
}

pub(crate) fn schema_seventeen(connection: &Connection) -> Result<(), StorageError> {
    connection.execute_batch("CREATE TABLE submission_lifecycles(job_id INTEGER PRIMARY KEY REFERENCES submission_jobs(id),account TEXT NOT NULL CHECK(length(account)>0),admitted_at TEXT NOT NULL CHECK(length(admitted_at)>0),manual_admitted_at TEXT CHECK(manual_admitted_at IS NULL OR length(manual_admitted_at)>0),revision INTEGER NOT NULL CHECK(revision>0),payload TEXT NOT NULL); CREATE INDEX lifecycle_account_manual_admission ON submission_lifecycles(account,length(manual_admitted_at),manual_admitted_at); PRAGMA user_version=17;")?;
    Ok(())
}
