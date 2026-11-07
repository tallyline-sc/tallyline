// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable refusal visibility before a controlled pending read-only operation.

use super::controller_support;
use super::recovery::RecoveryBackend;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{CpuWorker, HttpBackend, HttpDriver, SubmissionControlError};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp};
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, JobPhase, SubmissionAccountCheck, SubmissionJob,
    SubmissionPauseReason, SubmissionRepositoryError, SubmissionResult, UexErrorCode,
    ValidatedAccount,
};
use tallyline_submission::api::{SubmissionJobRepository, SubmissionServiceFenceRepository};

fn error(value: impl std::fmt::Debug) -> std::io::Error {
    std::io::Error::other(format!("{value:?}"))
}
enum ReadReply {
    Blocked,
    Account(Arc<AtomicUsize>),
}
struct Backend {
    inner: RecoveryBackend,
    started: Arc<AtomicBool>,
    reply: ReadReply,
}
impl HttpBackend for Backend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        self.inner.fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        terminals: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        self.inner.fetch_terminal_priors(terminals).await
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        self.inner.validate(values).await
    }
    async fn verify_receipts(
        &self,
        job: SubmissionJob,
        values: CredentialValues,
        token: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        self.inner.verify_receipts(job, values, token).await
    }
    async fn submit(
        &self,
        prepared: PreparedUexSubmission,
        values: CredentialValues,
        token: Option<String>,
    ) -> SubmissionResult {
        self.inner.submit(prepared, values, token).await
    }
    async fn recheck_refusal(
        &self,
        job: SubmissionJob,
        _values: CredentialValues,
        _token: Option<String>,
    ) -> Option<SubmissionResult> {
        self.started.store(true, Ordering::SeqCst);
        match &self.reply {
            ReadReply::Blocked => std::future::pending().await,
            ReadReply::Account(checks) => {
                checks.fetch_add(1, Ordering::SeqCst);
                job.resolve_account_check(SubmissionPauseReason::KeyChecked)
                    .ok()?
                    .result()
                    .cloned()
            }
        }
    }
}
fn verify_one(
    outcome: SubmissionResult,
    check: SubmissionAccountCheck,
    phase: JobPhase,
    name: &str,
) -> std::io::Result<()> {
    let posts = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(AtomicBool::new(false));
    let backend_posts = Arc::clone(&posts);
    let backend_started = Arc::clone(&started);
    let reason = if check == SubmissionAccountCheck::NotRequired {
        SubmissionPauseReason::AppToken
    } else {
        SubmissionPauseReason::KeyCheckUnavailable
    };
    let result = SubmissionResult::AccountRefusal {
        code: UexErrorCode::new(
            if check == SubmissionAccountCheck::NotRequired {
                "no_api_found"
            } else {
                "invalid_secret_key"
            }
            .to_owned(),
        ),
        http_status: 403,
        reason,
        check,
        outcome: Box::new(outcome),
    };
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(Backend {
            inner: RecoveryBackend {
                posts: backend_posts,
                result,
            },
            started: backend_started,
            reply: ReadReply::Blocked,
        })
    })?;
    let mut fixture =
        controller_support::fixture(client, SubmissionMode::Production, name).map_err(error)?;
    let released = fixture
        .controller
        .release_manual(
            fixture.report.id(),
            fixture.report.version(),
            SubmissionMode::Production,
            &fixture.application,
        )
        .map_err(error)?;
    let fault = rusqlite::Connection::open(&fixture.path).map_err(error)?;
    fault.execute_batch("CREATE TRIGGER reject_early_fence BEFORE INSERT ON submission_service_fences BEGIN SELECT RAISE(ABORT,'fixture save refusal'); END;").map_err(error)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Err(failure) = fixture.controller.tick(&fixture.application, None) {
            assert_eq!(
                failure,
                SubmissionControlError::Repository(SubmissionRepositoryError::Unavailable)
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    assert!(!started.load(Ordering::SeqCst));
    let mut reopened = SqliteReferenceCache::open(&fixture.path).map_err(error)?;
    assert!(
        SubmissionServiceFenceRepository::service_fences(&mut reopened)
            .map_err(error)?
            .is_empty()
    );
    drop(reopened);
    fault
        .execute_batch("DROP TRIGGER reject_early_fence;")
        .map_err(error)?;
    drop(fault);
    while !started.load(Ordering::SeqCst) {
        fixture
            .controller
            .tick(&fixture.application, None)
            .map_err(error)?;
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let mut reopened = SqliteReferenceCache::open(&fixture.path).map_err(error)?;
    let job = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .map_err(error)?
        .ok_or_else(|| error("missing committed refusal"))?;
    assert_eq!(job.phase(), phase);
    assert_eq!(job.attempts().len(), 1);
    let attempt = job
        .attempts()
        .first()
        .ok_or_else(|| error("missing committed attempt"))?;
    assert!(!attempt.receipt_checked());
    assert_eq!(
        attempt
            .account_evidence()
            .ok_or_else(|| error("missing account evidence"))?
            .check(),
        check
    );
    let fences = SubmissionServiceFenceRepository::service_fences(&mut reopened).map_err(error)?;
    assert_eq!(fences.len(), 1);
    assert_eq!(
        fences
            .first()
            .ok_or_else(|| error("missing account fence"))?
            .source_revision(),
        attempt
            .account_evidence()
            .ok_or_else(|| error("missing account evidence"))?
            .origin_revision()
            .ok_or_else(|| error("unbound account origin"))?
    );
    drop(reopened);
    fixture.controller.shutdown().map_err(error)?;
    drop(fixture.controller);
    driver.finish()?;
    let mut restarted = SqliteReferenceCache::open(&fixture.path).map_err(error)?;
    tallyline_workflows::api::SubmissionQueueStore::recover_unfinished(
        &mut restarted,
        Timestamp::from_seconds(1001),
    )
    .map_err(error)?;
    assert_eq!(
        SubmissionServiceFenceRepository::service_fences(&mut restarted).map_err(error)?,
        fences
    );
    let restored = SubmissionJobRepository::load(&mut restarted, released.job().id())
        .map_err(error)?
        .ok_or_else(|| error("missing recovered refusal"))?;
    assert_eq!(restored.attempts(), job.attempts());
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    drop(restarted);
    std::fs::remove_file(fixture.path)
}
pub(super) fn verify() -> std::io::Result<()> {
    CpuWorker::start("tallyline-durable-account-read", || {
        verify_account_completion()?;
        verify_one(
            SubmissionResult::NeedsOutcomeCheck {
                code: UexErrorCode::new("no_api_found".to_owned())
                    .ok_or_else(|| error("fixture code"))?,
                http_status: 403,
            },
            SubmissionAccountCheck::NotRequired,
            JobPhase::Held,
            "blocked-receipt",
        )?;
        verify_one(
            SubmissionResult::Paused(SubmissionPauseReason::KeyCheckUnavailable),
            SubmissionAccountCheck::Pending,
            JobPhase::Paused,
            "blocked-account",
        )
    })?
    .finish()?
}

fn verify_account_completion() -> std::io::Result<()> {
    let posts = Arc::new(AtomicUsize::new(0));
    let checks = Arc::new(AtomicUsize::new(0));
    let backend_posts = Arc::clone(&posts);
    let backend_checks = Arc::clone(&checks);
    let result = SubmissionResult::AccountRefusal {
        code: UexErrorCode::new("invalid_secret_key".to_owned()),
        http_status: 503,
        reason: SubmissionPauseReason::KeyCheckUnavailable,
        check: SubmissionAccountCheck::Pending,
        outcome: Box::new(SubmissionResult::Paused(
            SubmissionPauseReason::KeyCheckUnavailable,
        )),
    };
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(Backend {
            inner: RecoveryBackend {
                posts: backend_posts,
                result,
            },
            started: Arc::new(AtomicBool::new(false)),
            reply: ReadReply::Account(backend_checks),
        })
    })?;
    let mut fixture =
        controller_support::fixture(client, SubmissionMode::Production, "account-completion")
            .map_err(error)?;
    let released = fixture
        .controller
        .release_manual(
            fixture.report.id(),
            fixture.report.version(),
            SubmissionMode::Production,
            &fixture.application,
        )
        .map_err(error)?;
    let fault = rusqlite::Connection::open(&fixture.path).map_err(error)?;
    fault.execute_batch("CREATE TRIGGER reject_account_check BEFORE UPDATE ON submission_jobs WHEN OLD.phase='paused' AND NEW.revision>OLD.revision BEGIN SELECT RAISE(ABORT,'fixture check save refusal'); END;").map_err(error)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Err(failure) = fixture.controller.tick(&fixture.application, None) {
            assert_eq!(
                failure,
                SubmissionControlError::Repository(SubmissionRepositoryError::Unavailable)
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(checks.load(Ordering::SeqCst), 1);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    for _ in 0..3 {
        assert!(fixture.controller.tick(&fixture.application, None).is_err());
    }
    assert_eq!(checks.load(Ordering::SeqCst), 1);
    fault
        .execute_batch("DROP TRIGGER reject_account_check;")
        .map_err(error)?;
    drop(fault);
    while fixture
        .controller
        .jobs()
        .first()
        .and_then(|job| job.attempts().first())
        .and_then(|attempt| attempt.account_evidence())
        .is_none_or(|evidence| !matches!(evidence.check(), SubmissionAccountCheck::Completed(_)))
    {
        fixture
            .controller
            .tick(&fixture.application, None)
            .map_err(error)?;
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let current = fixture
        .controller
        .jobs()
        .first()
        .ok_or_else(|| error("missing completed account job"))?
        .clone();
    assert_eq!(current.phase(), JobPhase::Paused);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    let attempt = current
        .attempts()
        .first()
        .ok_or_else(|| error("missing completed account attempt"))?;
    assert!(!attempt.receipt_checked());
    let evidence = attempt
        .account_evidence()
        .ok_or_else(|| error("missing completed account evidence"))?;
    assert_eq!(
        evidence.check(),
        SubmissionAccountCheck::Completed(SubmissionPauseReason::KeyChecked)
    );
    assert_eq!(evidence.origin_revision(), Some(3));
    assert_eq!(evidence.http_status(), 503);
    assert_eq!(
        evidence.original_action(),
        SubmissionPauseReason::KeyCheckUnavailable
    );
    let mut reopened = SqliteReferenceCache::open(&fixture.path).map_err(error)?;
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, released.job().id()).map_err(error)?,
        Some(current.clone())
    );
    assert_eq!(
        SubmissionServiceFenceRepository::service_fences(&mut reopened)
            .map_err(error)?
            .first()
            .ok_or_else(|| error("missing completed account fence"))?
            .source_revision(),
        3
    );
    drop(reopened);
    let retried = fixture
        .controller
        .retry_now(current.id(), current.revision())
        .map_err(error)?;
    assert_eq!(retried.phase(), JobPhase::Queued);
    assert_eq!(retried.attempts(), current.attempts());
    fixture.controller.shutdown().map_err(error)?;
    drop(fixture.controller);
    driver.finish()?;
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    std::fs::remove_file(fixture.path)
}
