// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restart, timing and failed-transaction quota acceptance at the public controller seam.

use super::controller_support;
use super::quota::{Clock, QuotaBackend, VerifiedClock};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tallyline_adapter_uex::api::model::ServerTimeTracker;
use tallyline_app::api::{HttpClient, HttpDriver, SubmissionControlError};
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::model::{JobPhase, ServerTimeEstimate, SubmissionRepositoryError};

struct Case {
    fixture: controller_support::Fixture,
    driver: HttpDriver,
    client: HttpClient,
    posts: Arc<AtomicUsize>,
    wall: Arc<AtomicU64>,
    tick: Arc<AtomicU64>,
}
impl Case {
    fn new(name: &str, estimate: Option<ServerTimeEstimate>) -> Result<Self, String> {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let wall = Arc::new(AtomicU64::new(1_000_000));
        let tick = Arc::new(AtomicU64::new(0));
        let clock = Arc::new(VerifiedClock {
            wall: Arc::clone(&wall),
            tick: Arc::clone(&tick),
        });
        let tracker = match estimate {
            Some(estimate) => ServerTimeTracker::from_estimate(clock, estimate),
            None => ServerTimeTracker::new(clock),
        };
        let (driver, client) = HttpDriver::start_with_server_time(
            move || Ok::<_, ()>(QuotaBackend(backend_posts)),
            tracker,
        )
        .map_err(|error| format!("{error:?}"))?;
        let fixture = controller_support::fixture_with_clock(
            client.clone(),
            SubmissionMode::DryRun,
            name,
            Clock(Arc::clone(&wall)),
        )
        .map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            fixture,
            driver,
            client,
            posts,
            wall,
            tick,
        })
    }
    fn release(&mut self) -> Result<(), String> {
        self.fixture
            .controller
            .release_manual(
                self.fixture.report.id(),
                self.fixture.report.version(),
                SubmissionMode::DryRun,
                &self.fixture.application,
            )
            .map_err(|error| format!("{error:?}"))?;
        self.wait_for(JobPhase::Held)
    }
    fn wait_for(&mut self, phase: JobPhase) -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while self
            .fixture
            .controller
            .jobs()
            .first()
            .ok_or("missing job")?
            .phase()
            != phase
        {
            self.fixture
                .controller
                .tick(&self.fixture.application, None)
                .map_err(|error| format!("{error:?}"))?;
            assert!(
                std::time::Instant::now() < deadline,
                "expected quota phase was not reached"
            );
            std::thread::yield_now();
        }
        Ok(())
    }
    fn finish(mut self) -> Result<(), String> {
        self.fixture
            .controller
            .shutdown()
            .map_err(|error| format!("{error:?}"))?;
        drop(self.fixture.controller);
        self.driver.finish().map_err(|error| format!("{error:?}"))?;
        std::fs::remove_file(self.fixture.path).map_err(|error| format!("{error:?}"))?;
        Ok(())
    }
}
fn fresh() -> Option<ServerTimeEstimate> {
    Some(ServerTimeEstimate::new(
        Timestamp::from_seconds(1000),
        std::time::Duration::ZERO,
    ))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__missing_or_stale_server_time_never_expires_quota_from_local_wall_time()
{
    for (name, estimate, tick) in [
        ("quota-missing-time", None, 10_000),
        ("quota-stale-time", fresh(), 900_001),
    ] {
        tallyline_app::api::CpuWorker::start("tallyline-quota-time", move || {
            let mut case = Case::new(name, estimate).unwrap();
            case.release().unwrap();
            case.wall.store(1_100_000, Ordering::SeqCst);
            case.tick.store(tick, Ordering::SeqCst);
            for _ in 0..10 {
                case.fixture
                    .controller
                    .tick(&case.fixture.application, None)
                    .unwrap();
            }
            assert_eq!(case.posts.load(Ordering::SeqCst), 1);
            assert_eq!(case.fixture.controller.jobs()[0].phase(), JobPhase::Held);
            assert_eq!(case.fixture.controller.service_fences().unwrap().len(), 1);
            case.finish().unwrap();
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
fn r_sub_2_r_sub_11_r_api_3__restored_quota_expiry_does_not_resurrect_the_original_released_report()
{
    tallyline_app::api::CpuWorker::start("tallyline-quota-restart", || {
        let mut case = Case::new("quota-no-resurrection", fresh()).unwrap();
        case.release().unwrap();
        let original = case.fixture.controller.jobs()[0].attempts()[0].clone();
        case.fixture.controller.shutdown().unwrap();
        drop(case.fixture.controller);
        case.tick.store(10_000, Ordering::SeqCst);
        let information = tallyline_app::api::compiled_build_information().unwrap();
        case.fixture.controller = controller_support::open_with_build_information(
            &case.fixture.path,
            case.client.clone(),
            information,
        )
        .unwrap();
        assert_eq!(
            case.fixture.controller.jobs()[0].phase(),
            JobPhase::Returned
        );
        assert!(
            case.fixture
                .controller
                .tick(&case.fixture.application, None)
                .unwrap()
        );
        assert!(case.fixture.controller.service_fences().unwrap().is_empty());
        assert_eq!(
            case.fixture.controller.jobs()[0].phase(),
            JobPhase::Returned
        );
        assert_eq!(case.fixture.controller.jobs()[0].attempts()[0], original);
        assert_eq!(case.posts.load(Ordering::SeqCst), 1);
        case.finish().unwrap();
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
fn r_sub_2_r_sub_11_r_api_3__failed_expiry_transaction_retains_fence_and_retries_only_local_persistence()
 {
    tallyline_app::api::CpuWorker::start("tallyline-quota-rollback",||{
        let mut case=Case::new("quota-expiry-rollback",fresh()).unwrap();case.release().unwrap();
        let original=case.fixture.controller.jobs()[0].clone();
        let fault=rusqlite::Connection::open(&case.fixture.path).unwrap();
        fault.execute_batch("CREATE TRIGGER fail_quota_expiry BEFORE INSERT ON submission_quota_expiries BEGIN SELECT RAISE(ABORT,'test expiry rollback'); END").unwrap();
        case.wall.store(1_010_000,Ordering::SeqCst);case.tick.store(10_000,Ordering::SeqCst);
        for _ in 0..3 {
            assert_eq!(case.fixture.controller.tick(&case.fixture.application,None),Err(SubmissionControlError::Repository(SubmissionRepositoryError::Unavailable)));
            assert_eq!(case.fixture.controller.jobs()[0],original);
            assert_eq!(case.fixture.controller.service_fences().unwrap().len(),1);
            assert_eq!(case.posts.load(Ordering::SeqCst),1);
        }
        fault.execute_batch("DROP TRIGGER fail_quota_expiry").unwrap();drop(fault);
        case.wait_for(JobPhase::Succeeded).unwrap();
        assert_eq!(case.posts.load(Ordering::SeqCst),2);
        assert_eq!(case.fixture.controller.jobs()[0].attempts()[0],original.attempts()[0]);
        case.finish().unwrap();
    }).unwrap().finish().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__failed_read_quota_save_retries_persistence_without_another_get() {
    tallyline_app::api::CpuWorker::start("tallyline-read-quota-save",||{
        let reads=Arc::new(AtomicUsize::new(0));let backend_reads=Arc::clone(&reads);
        let (driver,client)=HttpDriver::start(move||Ok::<_,()>(super::quota::ReadQuotaBackend {reads:backend_reads,observations:Default::default()})).unwrap();
        let mut fixture=controller_support::fixture(client,SubmissionMode::Production,"read-quota-save-failure").unwrap();
        fixture.controller.release_manual(fixture.report.id(),fixture.report.version(),SubmissionMode::Production,&fixture.application).unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase()!=JobPhase::OutcomeUnknown {fixture.controller.tick(&fixture.application,None).unwrap();assert!(std::time::Instant::now()<deadline);std::thread::yield_now();}
        let original=fixture.controller.jobs()[0].clone();
        let fault=rusqlite::Connection::open(&fixture.path).unwrap();
        fault.execute_batch("CREATE TRIGGER fail_read_quota BEFORE INSERT ON authenticated_token_quotas BEGIN SELECT RAISE(ABORT,'test quota save'); END").unwrap();
        fixture.controller.check_receipts(original.id(),original.revision(),&fixture.application,None).unwrap();
        loop {
            let result=fixture.controller.tick(&fixture.application,None);
            if result==Err(SubmissionControlError::Repository(SubmissionRepositoryError::Unavailable)){break;}
            assert!(matches!(result,Ok(_) | Err(SubmissionControlError::Unavailable)));
            assert!(std::time::Instant::now()<deadline);std::thread::yield_now();
        }
        for _ in 0..3 {assert_eq!(fixture.controller.tick(&fixture.application,None),Err(SubmissionControlError::Repository(SubmissionRepositoryError::Unavailable)));}
        assert_eq!(reads.load(Ordering::SeqCst),1);assert!(fixture.controller.authenticated_quotas().unwrap().is_empty());
        fault.execute_batch("DROP TRIGGER fail_read_quota").unwrap();drop(fault);
        while fixture.controller.authenticated_quotas().unwrap().is_empty(){
            let result=fixture.controller.tick(&fixture.application,None);assert!(matches!(result,Ok(_) | Err(SubmissionControlError::Unavailable)));
            assert!(std::time::Instant::now()<deadline);
        }
        assert_eq!(fixture.controller.jobs()[0],original);assert_eq!(reads.load(Ordering::SeqCst),1);
        fixture.controller.shutdown().unwrap();drop(fixture.controller);driver.finish().unwrap();std::fs::remove_file(fixture.path).unwrap();
    }).unwrap().finish().unwrap();
}

struct RepeatingQuotaBackend(Arc<AtomicUsize>);
impl tallyline_app::api::HttpBackend for RepeatingQuotaBackend {
    async fn fetch(
        &self,
        endpoint: tallyline_reference_data::api::model::Endpoint,
    ) -> Result<
        tallyline_reference_data::api::model::EndpointData,
        tallyline_adapter_uex::api::ReadError,
    > {
        QuotaBackend(Arc::clone(&self.0)).fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        _: Vec<tallyline_shared_kernel::api::model::TerminalId>,
    ) -> Result<
        Vec<tallyline_reference_data::api::model::TerminalPriors>,
        tallyline_adapter_uex::api::ReadError,
    > {
        Ok(vec![])
    }
    async fn validate(
        &self,
        _: tallyline_submission::api::model::CredentialValues,
    ) -> Result<
        tallyline_submission::api::model::ValidatedAccount,
        tallyline_submission::api::model::CredentialFailure,
    > {
        Ok(tallyline_submission::api::model::ValidatedAccount::new(
            "dummy-a".to_owned(),
        ))
    }
    async fn verify_receipts(
        &self,
        _: tallyline_submission::api::model::SubmissionJob,
        _: tallyline_submission::api::model::CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        None
    }
    async fn submit(
        &self,
        _: tallyline_adapter_uex::api::model::PreparedUexSubmission,
        _: tallyline_submission::api::model::CredentialValues,
        _: Option<String>,
    ) -> tallyline_submission::api::model::SubmissionResult {
        let index = self.0.fetch_add(1, Ordering::SeqCst);
        let observed = 1000 + 10 * index.min(3) as u64;
        tallyline_submission::api::model::SubmissionResult::NotProcessed(
            tallyline_submission::api::model::NotProcessed::DailyQuotaUntil(
                tallyline_submission::api::model::TokenQuotaReset::new(
                    Timestamp::from_seconds(observed + 10),
                    Some(Timestamp::from_seconds(observed)),
                ),
            ),
        )
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__quota_reset_does_not_exceed_the_automatic_attempt_ceiling() {
    tallyline_app::api::CpuWorker::start("tallyline-quota-ceiling", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let wall = Arc::new(AtomicU64::new(1_000_000));
        let tick = Arc::new(AtomicU64::new(0));
        let tracker = ServerTimeTracker::from_estimate(
            Arc::new(VerifiedClock {
                wall: Arc::clone(&wall),
                tick: Arc::clone(&tick),
            }),
            fresh().unwrap(),
        );
        let (driver, client) = HttpDriver::start_with_server_time(
            move || Ok::<_, ()>(RepeatingQuotaBackend(backend_posts)),
            tracker,
        )
        .unwrap();
        let mut fixture = controller_support::fixture_with_clock(
            client,
            SubmissionMode::DryRun,
            "quota-attempt-ceiling",
            Clock(Arc::clone(&wall)),
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
        for (at, expected) in [(1000, 1), (1010, 2), (1020, 3)] {
            wall.store(at * 1000, Ordering::SeqCst);
            tick.store((at - 1000) * 1000, Ordering::SeqCst);
            while posts.load(Ordering::SeqCst) < expected
                || fixture.controller.jobs()[0].phase() != JobPhase::Held
            {
                fixture.controller.tick(&fixture.application, None).unwrap();
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
        let original = fixture.controller.jobs()[0].attempts().to_vec();
        wall.store(1_030_000, Ordering::SeqCst);
        tick.store(30_000, Ordering::SeqCst);
        for _ in 0..10 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 3);
        assert_eq!(fixture.controller.jobs()[0].attempts(), original);
        assert_eq!(fixture.controller.jobs()[0].phase(), JobPhase::Held);
        assert!(!fixture.controller.jobs()[0].can_retry());
        assert!(fixture.controller.service_fences().unwrap().is_empty());
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}
