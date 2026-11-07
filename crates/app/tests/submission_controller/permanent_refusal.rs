// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Permanent correction classification through SQLite and its legacy document reader.

use super::controller_support;
use super::recovery::RecoveryBackend;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_app::api::{CpuWorker, HttpDriver};
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    JobPhase, SubmissionCorrection, SubmissionRejection, SubmissionResult, UexErrorCode,
};

fn error(value: impl std::fmt::Debug) -> std::io::Error {
    std::io::Error::other(format!("{value:?}"))
}
fn round_trip(correction: SubmissionCorrection, name: &str) -> std::io::Result<()> {
    let result = SubmissionResult::Rejected(SubmissionRejection::new(
        UexErrorCode::new("invalid_game_version".to_owned())
            .ok_or_else(|| error("fixture code"))?,
        correction,
    ));
    let expected = result.clone();
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(RecoveryBackend {
            posts: Arc::new(AtomicUsize::new(0)),
            result,
        })
    })?;
    let mut fixture =
        controller_support::fixture(client, SubmissionMode::Production, name).map_err(error)?;
    let release = fixture
        .controller
        .release_manual(
            fixture.report.id(),
            fixture.report.version(),
            SubmissionMode::Production,
            &fixture.application,
        )
        .map_err(error)?;
    let id = release.job().id();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while fixture
        .controller
        .jobs()
        .iter()
        .all(|job| job.id() != id || job.phase() != JobPhase::Rejected)
    {
        fixture
            .controller
            .tick(&fixture.application, None)
            .map_err(error)?;
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    fixture.controller.shutdown().map_err(error)?;
    drop(fixture.controller);
    let mut store = SqliteReferenceCache::open(&fixture.path).map_err(error)?;
    assert_eq!(
        SubmissionJobRepository::load(&mut store, id)
            .map_err(error)?
            .and_then(|job| job.result().cloned()),
        Some(expected.clone())
    );
    drop(store);
    if correction == SubmissionCorrection::NewCapture {
        let connection = rusqlite::Connection::open(&fixture.path).map_err(error)?;
        let stored_id = i64::try_from(id.0).map_err(error)?;
        let payload: String = connection
            .query_row(
                "SELECT payload FROM submission_jobs WHERE id=?1",
                [stored_id],
                |row| row.get(0),
            )
            .map_err(error)?;
        let mut document: serde_json::Value = serde_json::from_str(&payload).map_err(error)?;
        *document
            .get_mut("result")
            .ok_or_else(|| error("missing fixture result"))? =
            serde_json::json!({"rejected":"invalid_game_version"});
        connection
            .execute(
                "UPDATE submission_jobs SET payload=?1 WHERE id=?2",
                (serde_json::to_string(&document).map_err(error)?, stored_id),
            )
            .map_err(error)?;
        drop(connection);
        let mut restored = SqliteReferenceCache::open(&fixture.path).map_err(error)?;
        assert_eq!(
            SubmissionJobRepository::load(&mut restored, id)
                .map_err(error)?
                .and_then(|job| job.result().cloned()),
            Some(expected)
        );
    }
    driver.finish()?;
    std::fs::remove_file(fixture.path)
}
pub(super) fn verify() -> std::io::Result<()> {
    CpuWorker::start("tallyline-permanent-refusal", || {
        round_trip(SubmissionCorrection::NewCapture, "permanent-capture")?;
        round_trip(SubmissionCorrection::ReviewClient, "permanent-opaque")
    })?
    .finish()?
}
