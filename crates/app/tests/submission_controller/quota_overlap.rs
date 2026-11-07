// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Overlapping read and submission quota persistence through the public queue owner.

use super::controller_support;
use super::quota::{Clock, QuotaBackend, VerifiedClock};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::{PreparedUexSubmission, ServerTimeTracker};
use tallyline_app::api::{HttpBackend, HttpDriver, SubmissionControlError};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp};
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, JobPhase, NotProcessed, Persistence, ServerTimeEstimate,
    SubmissionJob, SubmissionRepositoryError, SubmissionResult, TokenQuotaObservation,
    TokenQuotaReset, ValidatedAccount, VerifiedUexReport,
};

struct OverlappingQuotaBackend {
    posts: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    release_post: Arc<tokio::sync::Notify>,
    observations: std::sync::Mutex<Vec<TokenQuotaObservation>>,
}
impl HttpBackend for OverlappingQuotaBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        QuotaBackend(Arc::clone(&self.posts)).fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        _: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Ok(vec![])
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        if self.reads.fetch_add(1, Ordering::SeqCst) == 0 {
            self.observations
                .lock()
                .map_err(|_| CredentialFailure::Unavailable)?
                .push(TokenQuotaObservation::new(
                    values.token_scope(),
                    Some(TokenQuotaReset::new(
                        Timestamp::from_seconds(1010),
                        Some(Timestamp::from_seconds(1000)),
                    )),
                ));
            Err(CredentialFailure::Unavailable)
        } else {
            Ok(ValidatedAccount::new("dummy-a".to_owned()))
        }
    }
    async fn verify_receipts(
        &self,
        _: SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        None
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        if self.posts.fetch_add(1, Ordering::SeqCst) == 0 {
            self.release_post.notified().await;
            SubmissionResult::NotProcessed(NotProcessed::DailyQuotaUntil(TokenQuotaReset::new(
                Timestamp::from_seconds(1020),
                Some(Timestamp::from_seconds(1000)),
            )))
        } else {
            SubmissionResult::DryRunSucceeded
        }
    }
    fn take_quota_observations(&self) -> Vec<TokenQuotaObservation> {
        match self.observations.lock() {
            Ok(mut observations) => std::mem::take(&mut *observations),
            Err(_) => {
                vec![TokenQuotaObservation::new(None, None)]
            }
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__read_reset_cannot_acknowledge_unsaved_later_post_quota() {
    tallyline_app::api::CpuWorker::start("tallyline-overlap-quota", overlap_case)
        .unwrap()
        .finish()
        .unwrap()
        .unwrap();
}
fn overlap_case() -> Result<(), String> {
    use tallyline_submission::api::CredentialValidator as _;
    let posts = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let release_post = Arc::new(tokio::sync::Notify::new());
    let backend = OverlappingQuotaBackend {
        posts: Arc::clone(&posts),
        reads: Arc::clone(&reads),
        release_post: Arc::clone(&release_post),
        observations: Default::default(),
    };
    let wall = Arc::new(AtomicU64::new(1_000_000));
    let tick = Arc::new(AtomicU64::new(0));
    let tracker = ServerTimeTracker::from_estimate(
        Arc::new(VerifiedClock {
            wall: Arc::clone(&wall),
            tick: Arc::clone(&tick),
        }),
        ServerTimeEstimate::new(Timestamp::from_seconds(1000), std::time::Duration::ZERO),
    );
    let (driver, client) =
        HttpDriver::start_with_server_time(move || Ok::<_, ()>(backend), tracker)
            .map_err(|error| format!("{error:?}"))?;
    let mut fixture = controller_support::fixture_with_clock(
        client.clone(),
        SubmissionMode::DryRun,
        "overlapping-read-post-quota",
        Clock(Arc::clone(&wall)),
    )
    .map_err(|error| format!("{error:?}"))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("{error:?}"))?;
    runtime
        .block_on(fixture.application.establish_token_scope(
            &mut super::quota::ScopeStore,
            None,
            Persistence::Session,
        ))
        .map_err(|error| format!("{error:?}"))?;
    let scope = fixture
        .application
        .token_scope(None)
        .ok_or("missing fixture token scope")?;
    fixture
        .controller
        .release_manual(
            fixture.report.id(),
            fixture.report.version(),
            SubmissionMode::DryRun,
            &fixture.application,
        )
        .map_err(|error| format!("{error:?}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while posts.load(Ordering::SeqCst) == 0 {
        fixture
            .controller
            .tick(&fixture.application, None)
            .map_err(|error| format!("{error:?}"))?;
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let sending = fixture
        .controller
        .jobs()
        .first()
        .ok_or("missing fixture job")?
        .clone();
    let values = fixture
        .application
        .credentials_for("dummy-a")
        .ok_or("missing fixture account")?;
    assert_eq!(
        runtime.block_on(client.validate(values)),
        Err(CredentialFailure::Unavailable)
    );
    let fault = rusqlite::Connection::open(&fixture.path).map_err(|error| format!("{error:?}"))?;
    fault.execute_batch("CREATE TRIGGER fail_post_quota BEFORE UPDATE ON submission_jobs BEGIN SELECT RAISE(ABORT,'test POST quota save'); END").map_err(|error| format!("{error:?}"))?;
    release_post.notify_one();
    loop {
        let result = fixture.controller.tick(&fixture.application, None);
        if result
            == Err(SubmissionControlError::Repository(
                SubmissionRepositoryError::Unavailable,
            ))
        {
            break;
        }
        result.map_err(|error| format!("{error:?}"))?;
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        fixture
            .controller
            .authenticated_quotas()
            .map_err(|error| format!("{error:?}"))?
            .len(),
        1
    );
    assert!(
        fixture
            .controller
            .service_fences()
            .map_err(|error| format!("{error:?}"))?
            .is_empty()
    );
    wall.store(1_010_000, Ordering::SeqCst);
    tick.store(10_000, Ordering::SeqCst);
    for _ in 0..3 {
        assert_eq!(
            fixture.controller.tick(&fixture.application, None),
            Err(SubmissionControlError::Repository(
                SubmissionRepositoryError::Unavailable
            ))
        );
    }
    assert!(
        fixture
            .controller
            .authenticated_quotas()
            .map_err(|error| format!("{error:?}"))?
            .is_empty()
    );
    assert_eq!(
        fixture
            .controller
            .jobs()
            .first()
            .ok_or("missing fixture job")?,
        &sending
    );
    assert_eq!(
        runtime.block_on(
            client.validate(
                fixture
                    .application
                    .credentials_for("dummy-a")
                    .ok_or("missing fixture account")?
            )
        ),
        Err(CredentialFailure::Unavailable),
        "known unsaved POST quota must retain admission after the GET reset"
    );
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    fault
        .execute_batch("DROP TRIGGER fail_post_quota")
        .map_err(|error| format!("{error:?}"))?;
    drop(fault);
    fixture
        .controller
        .tick(&fixture.application, None)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        fixture
            .controller
            .jobs()
            .first()
            .ok_or("missing fixture job")?
            .phase(),
        JobPhase::Held
    );
    let fence = fixture
        .controller
        .service_fences()
        .map_err(|error| format!("{error:?}"))?
        .first()
        .copied()
        .ok_or("missing committed quota fence")?;
    assert_eq!(fence.token_scope(), Some(scope));
    assert_eq!(fence.source(), sending.id());
    assert_eq!(fence.source_revision(), sending.revision() + 1);
    assert_eq!(fence.quota_reset(), Some(Timestamp::from_seconds(1020)));
    assert_eq!(
        runtime.block_on(
            client.validate(
                fixture
                    .application
                    .credentials_for("dummy-a")
                    .ok_or("missing fixture account")?
            )
        ),
        Err(CredentialFailure::Unavailable)
    );
    wall.store(1_020_000, Ordering::SeqCst);
    tick.store(20_000, Ordering::SeqCst);
    fixture
        .controller
        .tick(&fixture.application, None)
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        fixture
            .controller
            .service_fences()
            .map_err(|error| format!("{error:?}"))?
            .is_empty()
    );
    assert!(
        runtime
            .block_on(
                client.validate(
                    fixture
                        .application
                        .credentials_for("dummy-a")
                        .ok_or("missing fixture account")?
                )
            )
            .is_ok(),
        "the exact committed POST hold must be acknowledged so its own reset reopens admission"
    );
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    fixture
        .controller
        .shutdown()
        .map_err(|error| format!("{error:?}"))?;
    drop(fixture.controller);
    driver.finish().map_err(|error| format!("{error:?}"))?;
    std::fs::remove_file(fixture.path).map_err(|error| format!("{error:?}"))?;
    Ok(())
}
