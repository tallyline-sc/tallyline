// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Durable recovery through the public application with real SQLite.

use super::{Backend, controller_support};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{HttpBackend, HttpDriver};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp};
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, JobPhase, NotProcessed, SubmissionResult, ValidatedAccount,
};
use tallyline_submission::api::model::{
    SubmissionCorrection, SubmissionHoldReason, SubmissionJob, SubmissionPauseReason,
    SubmissionRejection, UexErrorCode, VerifiedUexReport,
};

pub(super) struct RecoveryBackend {
    pub(super) posts: Arc<AtomicUsize>,
    pub(super) result: SubmissionResult,
}
impl HttpBackend for RecoveryBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        Backend(Arc::clone(&self.posts)).fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        ids: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Backend(Arc::clone(&self.posts))
            .fetch_terminal_priors(ids)
            .await
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        Backend(Arc::clone(&self.posts)).validate(values).await
    }
    async fn verify_receipts(
        &self,
        _: SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        None
    }
    async fn recheck_refusal(
        &self,
        job: SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<SubmissionResult> {
        match job.result().map(SubmissionResult::final_result) {
            Some(SubmissionResult::NeedsOutcomeCheck { code, .. })
                if code.as_str() == "Future_Code" =>
            {
                None
            }
            Some(SubmissionResult::NeedsOutcomeCheck { code, .. }) => {
                Some(SubmissionResult::Rejected(SubmissionRejection::new(
                    code.clone(),
                    SubmissionCorrection::ReviewClient,
                )))
            }
            _ => None,
        }
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        self.posts.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}
struct ManualClock(Arc<AtomicU64>);
impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(self.0.load(Ordering::SeqCst))
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9_r_sub_11__proven_unprocessed_attempts_back_off_and_stop_at_the_retry_limit() {
    tallyline_app::api::CpuWorker::start("tallyline-recovery-contract", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::NotProcessed(NotProcessed::ServiceUnavailable),
            })
        })
        .unwrap();
        let seconds = Arc::new(AtomicU64::new(1000));
        let mut fixture = controller_support::fixture_with_clock(
            client,
            SubmissionMode::DryRun,
            &format!(
                "retry-limit-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            ManualClock(Arc::clone(&seconds)),
        )
        .unwrap();
        let release = fixture
            .controller
            .release_manual(
                fixture.report.id(),
                fixture.report.version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase() != JobPhase::Held {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < end);
            std::thread::yield_now();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        for _ in 0..50 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        for at in [1005, 1015] {
            seconds.store(at, Ordering::SeqCst);
            let expected = if at == 1005 { 2 } else { 3 };
            while posts.load(Ordering::SeqCst) < expected
                || fixture.controller.jobs()[0].phase() != JobPhase::Held
            {
                fixture.controller.tick(&fixture.application, None).unwrap();
                assert!(
                    std::time::Instant::now() < end,
                    "verified unprocessed work must become retryable after its backoff"
                );
                std::thread::yield_now();
            }
        }
        seconds.store(1100, Ordering::SeqCst);
        for _ in 0..50 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        let job = fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == release.job().id())
            .unwrap();
        assert_eq!(job.attempts().len(), 3);
        assert_eq!(posts.load(Ordering::SeqCst), 3);
        assert_eq!(job.phase(), JobPhase::Held);
        let revision = fixture.controller.jobs()[0].revision();
        fixture
            .controller
            .retry_now(release.job().id(), revision)
            .unwrap();
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        let mut reopened =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
        let restored = tallyline_submission::api::SubmissionJobRepository::load(
            &mut reopened,
            release.job().id(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(restored.attempts().len(), 3);
        tallyline_workflows::api::SubmissionQueueStore::recover_unfinished(
            &mut reopened,
            Timestamp::from_seconds(1100),
        )
        .unwrap();
        let recovered = tallyline_submission::api::SubmissionJobRepository::load(
            &mut reopened,
            release.job().id(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(recovered.phase(), JobPhase::Returned);
        assert_eq!(recovered.attempts().len(), 3);
        drop(reopened);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11__a_token_pause_stops_other_released_jobs_until_explicit_retry() {
    tallyline_app::api::CpuWorker::start("tallyline-pause-contract", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::Paused(SubmissionPauseReason::AppToken),
            })
        })
        .unwrap();
        let mut fixture = controller_support::fixture(
            client,
            SubmissionMode::DryRun,
            &format!(
                "pause-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
        )
        .unwrap();
        let first = fixture
            .controller
            .release_manual(
                fixture.report.id(),
                fixture.report.version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .unwrap();
        let (store, second) = super::support::stored_gate(&fixture.path).unwrap();
        let second = fixture
            .controller
            .release_manual(
                second.source().id(),
                second.source().version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .unwrap();
        drop(store);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == first.job().id())
            .unwrap()
            .phase()
            != JobPhase::Paused
        {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        for _ in 0..100 {
            fixture.controller.tick(&fixture.application, None).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            posts.load(Ordering::SeqCst),
            1,
            "an account failure must stop the entire queue"
        );
        assert_eq!(
            fixture
                .controller
                .jobs()
                .iter()
                .find(|job| job.id() == second.job().id())
                .unwrap()
                .attempts()
                .len(),
            0
        );
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_11__held_receipt_checks_resume_read_only_and_commit_the_final_report() {
    tallyline_app::api::CpuWorker::start("tallyline-held-check", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::NeedsOutcomeCheck {
                    code: UexErrorCode::new("dummy_unknown_error".to_owned()).unwrap(),
                    http_status: 400,
                },
            })
        })
        .unwrap();
        let seconds = Arc::new(AtomicU64::new(1000));
        let mut fixture = controller_support::fixture_with_clock(
            client,
            SubmissionMode::Production,
            &format!(
                "held-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            ManualClock(Arc::clone(&seconds)),
        )
        .unwrap();
        let released = fixture
            .controller
            .release_manual(
                fixture.report.id(),
                fixture.report.version(),
                SubmissionMode::Production,
                &fixture.application,
            )
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase() != JobPhase::Held {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        seconds.store(1900, Ordering::SeqCst);
        while fixture.controller.jobs()[0].phase() != JobPhase::Rejected {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(
                std::time::Instant::now() < deadline,
                "a held safety lookup must be repeated without another POST"
            );
            std::thread::yield_now();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.controller.jobs()[0].attempts().len(), 1);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        let mut store =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
        let restored = tallyline_submission::api::SubmissionJobRepository::load(
            &mut store,
            released.job().id(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(restored.phase(), JobPhase::Rejected);
        assert!(restored.attempts()[0].receipt_checked());
        assert!(
            tallyline_reporting::api::ReportRepository::load_draft(&mut store, fixture.report.id())
                .unwrap()
                .is_none()
        );
        drop(store);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11__acceptance_and_duplicate_holds_recheck_at_their_deadlines() {
    for (reason, delay) in [
        (SubmissionHoldReason::AcceptanceClosed, 900),
        (SubmissionHoldReason::Cooldown, 300),
    ] {
        tallyline_app::api::CpuWorker::start("tallyline-acceptance-check", move || {
            use tallyline_reference_data::api::ReferenceDataCache;
            let posts = Arc::new(AtomicUsize::new(0));
            let backend_posts = Arc::clone(&posts);
            let (driver, client) = HttpDriver::start(move || {
                Ok::<_, ()>(RecoveryBackend {
                    posts: backend_posts,
                    result: SubmissionResult::Held(reason),
                })
            })
            .unwrap();
            let seconds = Arc::new(AtomicU64::new(1000));
            let mut fixture = controller_support::fixture_with_clock(
                client,
                SubmissionMode::DryRun,
                &format!(
                    "acceptance-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ),
                ManualClock(Arc::clone(&seconds)),
            )
            .unwrap();
            fixture
                .controller
                .release_manual(
                    fixture.report.id(),
                    fixture.report.version(),
                    SubmissionMode::DryRun,
                    &fixture.application,
                )
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while fixture.controller.jobs()[0].phase() != JobPhase::Held {
                fixture.controller.tick(&fixture.application, None).unwrap();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            seconds.store(1000 + delay - 1, Ordering::SeqCst);
            for _ in 0..50 {
                fixture.controller.tick(&fixture.application, None).unwrap();
            }
            assert_eq!(posts.load(Ordering::SeqCst), 1);
            let mut cache =
                tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
            for entry in cache.load().unwrap() {
                cache
                    .replace(&tallyline_reference_data::api::model::CachedReference::new(
                        Timestamp::from_seconds(1000 + delay),
                        entry.data().clone(),
                    ))
                    .unwrap();
            }
            drop(cache);
            seconds.store(1000 + delay, Ordering::SeqCst);
            while posts.load(Ordering::SeqCst) < 2 {
                fixture.controller.tick(&fixture.application, None).unwrap();
                assert!(
                    std::time::Instant::now() < deadline,
                    "acceptance must be reconsidered at its next check"
                );
                std::thread::yield_now();
            }
            fixture.controller.shutdown().unwrap();
            drop(fixture.controller);
            driver.finish().unwrap();
            std::fs::remove_file(fixture.path).unwrap();
        })
        .unwrap()
        .finish()
        .unwrap();
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_val_8__a_pending_receipt_check_blocks_release_of_an_older_shared_row() {
    tallyline_app::api::CpuWorker::start("tallyline-held-order", || {
        use tallyline_reporting::api::ReportRepository;
        let posts=Arc::new(AtomicUsize::new(0));let backend_posts=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move || Ok::<_, ()>(RecoveryBackend { posts:backend_posts,result:SubmissionResult::NeedsOutcomeCheck { code: UexErrorCode::new("dummy_unknown_error".to_owned()).unwrap(), http_status: 400 } })).unwrap();
        let mut fixture=controller_support::fixture(client,SubmissionMode::Production,&format!("held-order-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())).unwrap();
        fixture.controller.release_manual(fixture.report.id(),fixture.report.version(),SubmissionMode::Production,&fixture.application).unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase()!=JobPhase::Held {fixture.controller.tick(&fixture.application,None).unwrap();assert!(std::time::Instant::now()<deadline);std::thread::yield_now();}
        let mut store=tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
        let mut older=store.create_manual(fixture.report.key(),Timestamp::from_seconds(800),fixture.report.game_version()).unwrap();
        let row=older.add_commodity(tallyline_shared_kernel::api::model::CommodityId(42)).unwrap();
        older.set_price(row,Some("123.45".parse().unwrap())).unwrap();older.set_stock(row,Some("0".parse().unwrap())).unwrap();older.set_status(row,std::num::NonZeroU8::new(3)).unwrap();store.save_draft(&older,0).unwrap();drop(store);
        let result=fixture.controller.release_manual(older.id(),older.version(),SubmissionMode::Production,&fixture.application);
        assert!(matches!(result,Err(tallyline_app::api::SubmissionControlError::Release(tallyline_workflows::api::ManualReleaseError::Blocked(ref reasons))) if reasons.iter().any(|reason|matches!(reason,tallyline_reporting::api::model::ReleaseBlockReason::UnresolvedNewerObservation { .. }))));
        assert_eq!(posts.load(Ordering::SeqCst),1);
        fixture.controller.shutdown().unwrap();drop(fixture.controller);driver.finish().unwrap();std::fs::remove_file(fixture.path).unwrap();
    }).unwrap().finish().unwrap();
}

struct DuplicateBackend(Arc<AtomicUsize>);
impl HttpBackend for DuplicateBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        Backend(Arc::clone(&self.0)).fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        ids: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Backend(Arc::clone(&self.0))
            .fetch_terminal_priors(ids)
            .await
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        Backend(Arc::clone(&self.0)).validate(values).await
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
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            SubmissionResult::Unknown
        } else {
            SubmissionResult::Held(SubmissionHoldReason::Cooldown)
        }
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_sub_11__duplicate_after_recent_unknown_returns_without_another_retry() {
    tallyline_app::api::CpuWorker::start("tallyline-duplicate-evidence", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) =
            HttpDriver::start(move || Ok::<_, ()>(DuplicateBackend(backend_posts))).unwrap();
        let seconds = Arc::new(AtomicU64::new(1000));
        let mut fixture = controller_support::fixture_with_clock(
            client,
            SubmissionMode::Production,
            &format!(
                "duplicate-evidence-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            ManualClock(Arc::clone(&seconds)),
        )
        .unwrap();
        let first = fixture
            .controller
            .release_manual(
                fixture.report.id(),
                fixture.report.version(),
                SubmissionMode::Production,
                &fixture.application,
            )
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase() != JobPhase::OutcomeUnknown {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let (store, draft) = super::support::stored_gate(&fixture.path).unwrap();
        drop(store);
        let second = fixture
            .controller
            .release_manual(
                draft.source().id(),
                draft.source().version(),
                SubmissionMode::Production,
                &fixture.application,
            )
            .unwrap();
        seconds.store(1300, Ordering::SeqCst);
        while fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == second.job().id())
            .unwrap()
            .attempts()
            .last()
            .is_none_or(|attempt| attempt.ended_at().is_none())
        {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let result = fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == second.job().id())
            .unwrap();
        assert_eq!(result.phase(), JobPhase::Returned);
        assert!(
            format!("{:?}", result.result())
                .contains(&format!("DuplicateEvidence({:?})", first.job().id()))
        );
        assert_eq!(posts.load(Ordering::SeqCst), 2);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}
