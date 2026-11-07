// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Repeated read-only account results through the public queue owner and SQLite.

use super::controller_support;
use super::recovery::RecoveryBackend;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::model::{BuildInformation, BuildTarget};
use tallyline_app::api::{CpuWorker, HttpBackend, HttpDriver};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp};
use tallyline_submission::api::model::VerifiedUexReport;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SubmissionAccountCheck, SubmissionJob,
    SubmissionPauseReason, SubmissionResult, UexErrorCode, ValidatedAccount,
};
use tallyline_submission::api::{SubmissionJobRepository, SubmissionServiceFenceRepository};

fn error(value: impl std::fmt::Debug) -> std::io::Error {
    std::io::Error::other(format!("{value:?}"))
}
struct ManualClock(Arc<AtomicU64>);
impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(self.0.load(Ordering::SeqCst))
    }
}
struct Backend {
    inner: RecoveryBackend,
    checks: Arc<AtomicUsize>,
    final_action: SubmissionPauseReason,
}
impl HttpBackend for Backend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        self.inner.fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        ids: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        self.inner.fetch_terminal_priors(ids).await
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
    ) -> Option<Vec<VerifiedUexReport>> {
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
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<SubmissionResult> {
        if matches!(
            job.result().map(SubmissionResult::final_result),
            Some(SubmissionResult::NeedsOutcomeCheck { .. })
        ) && job
            .result()
            .and_then(SubmissionResult::account_evidence)
            .is_some_and(|evidence| evidence.check() != SubmissionAccountCheck::Pending)
        {
            return Some(SubmissionResult::Paused(
                SubmissionPauseReason::KeyCheckUnavailable,
            ));
        }
        let reason = if self.checks.fetch_add(1, Ordering::SeqCst) == 0 {
            SubmissionPauseReason::KeyCheckUnavailable
        } else {
            self.final_action
        };
        job.resolve_account_check(reason).ok()?.result().cloned()
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Interlude {
    None,
    Receipt,
    ReceiptAndAcknowledgement,
}
fn repeated(
    final_action: SubmissionPauseReason,
    name: &str,
    interlude: Interlude,
) -> std::io::Result<()> {
    let posts = Arc::new(AtomicUsize::new(0));
    let checks = Arc::new(AtomicUsize::new(0));
    let backend_posts = Arc::clone(&posts);
    let backend_checks = Arc::clone(&checks);
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, std::io::Error>(Backend {
            inner: RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::AccountRefusal {
                    code: UexErrorCode::new("invalid_secret_key".to_owned()),
                    http_status: 403,
                    reason: SubmissionPauseReason::KeyCheckUnavailable,
                    check: SubmissionAccountCheck::Pending,
                    outcome: Box::new(if interlude == Interlude::None {
                        SubmissionResult::Paused(SubmissionPauseReason::KeyCheckUnavailable)
                    } else {
                        SubmissionResult::NeedsOutcomeCheck {
                            code: UexErrorCode::new("invalid_secret_key".to_owned())
                                .ok_or_else(|| error("fixture code"))?,
                            http_status: 403,
                        }
                    }),
                },
            },
            checks: backend_checks,
            final_action,
        })
    })?;
    let seconds = Arc::new(AtomicU64::new(1000));
    let mut fixture = controller_support::fixture_with_clock(
        client.clone(),
        SubmissionMode::Production,
        name,
        ManualClock(Arc::clone(&seconds)),
    )
    .map_err(error)?;
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
    for (at, expected, revision) in [
        (1000, SubmissionPauseReason::KeyCheckUnavailable, 4),
        (
            1900,
            final_action,
            match interlude {
                Interlude::None => 5,
                Interlude::Receipt => 6,
                Interlude::ReceiptAndAcknowledgement => 7,
            },
        ),
    ] {
        seconds.store(at, Ordering::SeqCst);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            fixture
                .controller
                .tick(&fixture.application, None)
                .map_err(error)?;
            if fixture
                .controller
                .jobs()
                .iter()
                .any(|job| job.id() == id && job.revision() == revision)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "account check must complete"
            );
            std::thread::yield_now();
        }
        let mut reopened = SqliteReferenceCache::open(&fixture.path).map_err(error)?;
        let restored = SubmissionJobRepository::load(&mut reopened, id)
            .map_err(error)?
            .ok_or_else(|| error("missing restored account refusal"))?;
        assert_eq!(restored.revision(), revision);
        assert_eq!(
            restored.result().and_then(SubmissionResult::account_pause),
            Some(expected)
        );
        assert_eq!(
            SubmissionServiceFenceRepository::service_fences(&mut reopened)
                .map_err(error)?
                .len(),
            usize::from(interlude != Interlude::ReceiptAndAcknowledgement || revision == 4)
        );
        if revision == 4 && interlude == Interlude::ReceiptAndAcknowledgement {
            let acknowledged = fixture.controller.retry_now(id, revision).map_err(error)?;
            assert!(acknowledged.service_pause_acknowledged());
            assert_eq!(acknowledged.attempts(), restored.attempts());
        }
    }
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    assert_eq!(checks.load(Ordering::SeqCst), 2);
    fixture.controller.shutdown().map_err(error)?;
    drop(fixture.controller);
    let mut restarted = controller_support::open_with_build_information(
        &fixture.path,
        client,
        BuildInformation::new(None, None, BuildTarget::WindowsX64).map_err(error)?,
    )
    .map_err(error)?;
    restarted.tick(&fixture.application, None).map_err(error)?;
    assert_eq!(
        restarted
            .jobs()
            .iter()
            .find(|job| job.id() == id)
            .and_then(|job| job.result())
            .and_then(SubmissionResult::account_pause),
        Some(final_action)
    );
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    restarted.shutdown().map_err(error)?;
    drop(restarted);
    driver.finish()?;
    std::fs::remove_file(fixture.path)
}
pub(super) fn verify() -> std::io::Result<()> {
    CpuWorker::start("tallyline-repeated-account", || {
        repeated(
            SubmissionPauseReason::KeyChecked,
            "account-repeat-success",
            Interlude::None,
        )?;
        repeated(
            SubmissionPauseReason::Reauthenticate,
            "account-repeat-refused",
            Interlude::None,
        )?;
        repeated(
            SubmissionPauseReason::KeyChecked,
            "account-repeat-receipt",
            Interlude::Receipt,
        )?;
        repeated(
            SubmissionPauseReason::Reauthenticate,
            "account-repeat-ack",
            Interlude::ReceiptAndAcknowledgement,
        )
    })?
    .finish()?
}
