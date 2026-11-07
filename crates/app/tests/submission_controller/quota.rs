// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Token-quota expiry through the real queue controller and SQLite store.

use super::{Backend, controller_support};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{HttpBackend, HttpDriver};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp};
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, JobPhase, NotProcessed, SubmissionResult, ValidatedAccount,
};

#[derive(Clone)]
pub(super) struct Clock(pub(super) Arc<AtomicU64>);
impl tallyline_shared_kernel::api::Clock for Clock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(self.0.load(Ordering::SeqCst))
    }
}
pub(super) struct QuotaBackend(pub(super) Arc<AtomicUsize>);
impl HttpBackend for QuotaBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        Backend(Arc::clone(&self.0)).fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        _: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Ok(vec![])
    }
    async fn validate(&self, _: CredentialValues) -> Result<ValidatedAccount, CredentialFailure> {
        Ok(ValidatedAccount::new("dummy-a".to_owned()))
    }
    async fn verify_receipts(
        &self,
        _: tallyline_submission::api::model::SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        None
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            SubmissionResult::NotProcessed(NotProcessed::DailyQuotaUntil(
                tallyline_submission::api::model::TokenQuotaReset::new(
                    Timestamp::from_seconds(1010),
                    Some(Timestamp::from_seconds(1000)),
                ),
            ))
        } else {
            SubmissionResult::DryRunSucceeded
        }
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__valid_quota_reset_resumes_only_at_deadline_with_immutable_attempt() {
    tallyline_app::api::CpuWorker::start("tallyline-quota-reset", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let owned = Arc::clone(&posts);
        let now = Arc::new(AtomicU64::new(1_000_000));
        let tick = Arc::new(AtomicU64::new(0));
        let tracker = tallyline_adapter_uex::api::model::ServerTimeTracker::from_estimate(
            Arc::new(VerifiedClock {
                wall: Arc::clone(&now),
                tick: Arc::clone(&tick),
            }),
            tallyline_submission::api::model::ServerTimeEstimate::new(
                Timestamp::from_seconds(1000),
                std::time::Duration::ZERO,
            ),
        );
        let (driver, client) =
            HttpDriver::start_with_server_time(move || Ok::<_, ()>(QuotaBackend(owned)), tracker)
                .unwrap();
        let mut fixture = controller_support::fixture_with_clock(
            client,
            SubmissionMode::DryRun,
            "quota-exact-reset",
            Clock(Arc::clone(&now)),
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
        let original = fixture.controller.jobs()[0].attempts()[0].clone();
        now.store(1_009_999, Ordering::SeqCst);
        tick.store(9_999, Ordering::SeqCst);
        for _ in 0..10 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        now.store(1_010_000, Ordering::SeqCst);
        tick.store(10_000, Ordering::SeqCst);
        while fixture.controller.jobs()[0].phase() != JobPhase::Succeeded {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(
                std::time::Instant::now() < deadline,
                "quota did not resume at its reset"
            );
            std::thread::yield_now();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.controller.jobs()[0].attempts()[0], original);
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

pub(super) struct ScopeStore;
impl tallyline_submission::api::SecretStore for ScopeStore {
    async fn load(
        &mut self,
    ) -> Result<
        Option<tallyline_submission::api::model::SavedCredentials>,
        tallyline_submission::api::model::StorageFailure,
    > {
        Ok(None)
    }
    async fn save(
        &mut self,
        _: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Ok(())
    }
    async fn save_passive(
        &mut self,
        _: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Ok(())
    }
    async fn remove(&mut self) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Ok(())
    }
    async fn token_scope(
        &mut self,
        token: &str,
        _: tallyline_submission::api::model::Persistence,
    ) -> Result<
        Option<tallyline_submission::api::model::TokenScope>,
        tallyline_submission::api::model::StorageFailure,
    > {
        let mut id = [1; 32];
        id[16..].fill(if token == "dummy-packaged-a" { 2 } else { 3 });
        Ok(tallyline_submission::api::model::TokenScope::new(id))
    }
}
fn select_packaged(
    application: &mut tallyline_submission::api::CredentialApplication,
    token: &str,
) -> Result<(), String> {
    use tallyline_submission::api::model::{CredentialCandidate, Persistence};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("{error:?}"))?;
    let request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), None)
            .map_err(|error| format!("{error:?}"))?,
        Persistence::Session,
    ));
    runtime
        .block_on(application.accept(
            request,
            Ok(ValidatedAccount::new("dummy-a".to_owned())),
            Timestamp::from_seconds(1000),
            &mut ScopeStore,
        ))
        .map_err(|error| format!("{error:?}"))?;
    runtime
        .block_on(application.establish_token_scope(
            &mut ScopeStore,
            Some(token),
            Persistence::Session,
        ))
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__packaged_token_quota_binds_attempted_scope_and_keeps_rotation_independent()
 {
    tallyline_app::api::CpuWorker::start("tallyline-quota-scope", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let owned = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(QuotaBackend(owned))).unwrap();
        let mut fixture =
            controller_support::fixture(client, SubmissionMode::DryRun, "quota-packaged-scope")
                .unwrap();
        let a = tallyline_submission::api::model::ReleaseToken::new("dummy-packaged-a".to_owned())
            .unwrap();
        let b = tallyline_submission::api::model::ReleaseToken::new("dummy-packaged-b".to_owned())
            .unwrap();
        select_packaged(&mut fixture.application, a.for_validation()).unwrap();
        let scope_a = fixture
            .application
            .token_scope(Some(a.for_validation()))
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
            fixture
                .controller
                .tick(&fixture.application, Some(&a))
                .unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let fence = fixture.controller.service_fences().unwrap()[0];
        assert_eq!(fence.token_scope(), Some(scope_a));
        let source = fixture.controller.jobs()[0].clone();
        fixture
            .controller
            .cancel(source.id(), source.revision())
            .unwrap();
        let (store, next) = super::support::stored_gate(&fixture.path).unwrap();
        drop(store);
        fixture
            .controller
            .release_manual(
                next.source().id(),
                next.source().version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .unwrap();
        for _ in 0..10 {
            fixture
                .controller
                .tick(&fixture.application, Some(&a))
                .unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        select_packaged(&mut fixture.application, b.for_validation()).unwrap();
        while posts.load(Ordering::SeqCst) < 2 {
            fixture
                .controller
                .tick(&fixture.application, Some(&b))
                .unwrap();
            assert!(
                std::time::Instant::now() < deadline,
                "independent rotated token stayed fenced"
            );
            std::thread::yield_now();
        }
        assert_eq!(fixture.controller.service_fences().unwrap()[0], fence);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

pub(super) struct ReadQuotaBackend {
    pub(super) reads: Arc<AtomicUsize>,
    pub(super) observations:
        std::sync::Mutex<Vec<tallyline_submission::api::model::TokenQuotaObservation>>,
}
impl HttpBackend for ReadQuotaBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        Backend(Arc::clone(&self.reads)).fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        _: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Ok(vec![])
    }
    async fn validate(&self, _: CredentialValues) -> Result<ValidatedAccount, CredentialFailure> {
        Ok(ValidatedAccount::new("dummy-a".to_owned()))
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        SubmissionResult::Unknown
    }
    async fn verify_receipts(
        &self,
        _: tallyline_submission::api::model::SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        None
    }
    async fn verify_row_receipts(
        &self,
        _: tallyline_submission::api::model::SubmissionJob,
        values: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.observations.lock().ok()?.push(
            tallyline_submission::api::model::TokenQuotaObservation::new(
                values.token_scope(),
                Some(tallyline_submission::api::model::TokenQuotaReset::new(
                    Timestamp::from_seconds(1010),
                    Some(Timestamp::from_seconds(1000)),
                )),
            ),
        );
        None
    }
    fn take_quota_observations(
        &self,
    ) -> Vec<tallyline_submission::api::model::TokenQuotaObservation> {
        match self.observations.lock() {
            Ok(mut observations) => std::mem::take(&mut *observations),
            Err(_) => {
                vec![tallyline_submission::api::model::TokenQuotaObservation::new(None, None)]
            }
        }
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__read_quota_is_committed_and_blocks_another_receipt_check_without_replay()
 {
    tallyline_app::api::CpuWorker::start("tallyline-read-quota", || {
        let reads = Arc::new(AtomicUsize::new(0));
        let backend_reads = Arc::clone(&reads);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(ReadQuotaBackend {
                reads: backend_reads,
                observations: Default::default(),
            })
        })
        .unwrap();
        let mut fixture =
            controller_support::fixture(client, SubmissionMode::Production, "read-quota-owner")
                .unwrap();
        fixture
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
        let original = fixture.controller.jobs()[0].clone();
        fixture
            .controller
            .check_receipts(
                original.id(),
                original.revision(),
                &fixture.application,
                None,
            )
            .unwrap();
        let mut failures = Vec::new();
        while fixture
            .controller
            .authenticated_quotas()
            .unwrap()
            .is_empty()
        {
            if let Err(error) = fixture.controller.tick(&fixture.application, None) {
                failures.push(error);
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(fixture.controller.jobs()[0], original);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert!(
            fixture
                .controller
                .check_receipts(
                    original.id(),
                    original.revision(),
                    &fixture.application,
                    None
                )
                .is_err()
        );
        for _ in 0..10 {
            if let Err(error) = fixture.controller.tick(&fixture.application, None) {
                failures.push(error);
            }
        }
        assert_eq!(
            failures,
            vec![tallyline_app::api::SubmissionControlError::Unavailable]
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
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

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__shared_owner_stops_receipt_checks_as_soon_as_post_quota_is_known() {
    use tallyline_shared_kernel::api::model::{
        CommodityId, ReportId, RowId, TradeSide, UexEnvironment, UexReportId,
    };
    use tallyline_submission::api::SubmissionGateway;
    use tallyline_submission::api::model::{
        JobId, PayloadHash, SubmissionHeader, SubmissionJob, SubmissionRequest, SubmissionRow,
    };
    let count = Arc::new(AtomicUsize::new(0));
    let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(QuotaBackend(count))).unwrap();
    let request = SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(1),
            report_version: 1,
            account: "dummy-a".to_owned(),
            mode: SubmissionMode::Production,
            terminal: TerminalId(11),
            side: TradeSide::Buy,
            environment: UexEnvironment::Live,
            game_version: "4.10.2".to_owned(),
            faction_affinity: None,
            observed_at: Timestamp::from_seconds(900),
        },
        vec![
            SubmissionRow::new(
                RowId(1),
                CommodityId(42),
                "123.45".parse().unwrap(),
                "0".parse().unwrap(),
                3,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result =
        runtime.block_on(client.submit(client.prepare(&request).unwrap().0, &values, None));
    assert!(result.is_daily_quota());
    let accepted = SubmissionJob::queued(JobId(9), request)
        .unwrap()
        .begin_prepared(
            Timestamp::from_seconds(990),
            PayloadHash::new([9; 32]),
            Timestamp::from_seconds(900),
        )
        .unwrap()
        .complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(900)).unwrap(),
            ]),
            Timestamp::from_seconds(995),
        )
        .unwrap();
    assert!(matches!(
        client.begin_receipt_verification(&accepted, &values, None),
        Err(tallyline_app::api::model::HttpAdmissionFailure::Busy)
    ));
    use tallyline_submission::api::CredentialValidator as _;
    assert_eq!(
        runtime.block_on(client.validate(&values)),
        Err(CredentialFailure::Unavailable)
    );
    driver.finish().unwrap();
}

pub(super) struct VerifiedClock {
    pub(super) wall: Arc<AtomicU64>,
    pub(super) tick: Arc<AtomicU64>,
}
impl tallyline_shared_kernel::api::Clock for VerifiedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(self.wall.load(Ordering::SeqCst))
    }
}
impl tallyline_submission::api::SubmissionClock for VerifiedClock {
    fn monotonic(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.tick.load(Ordering::SeqCst))
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__quota_expiry_uses_server_progress_across_local_clock_jumps() {
    tallyline_app::api::CpuWorker::start("tallyline-quota-clock", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let wall = Arc::new(AtomicU64::new(1_000_000));
        let tick = Arc::new(AtomicU64::new(0));
        let tracker = tallyline_adapter_uex::api::model::ServerTimeTracker::from_estimate(
            Arc::new(VerifiedClock {
                wall: Arc::clone(&wall),
                tick: Arc::clone(&tick),
            }),
            tallyline_submission::api::model::ServerTimeEstimate::new(
                Timestamp::from_seconds(1000),
                std::time::Duration::ZERO,
            ),
        );
        let (driver, client) = HttpDriver::start_with_server_time(
            move || Ok::<_, ()>(QuotaBackend(backend_posts)),
            tracker,
        )
        .unwrap();
        let mut fixture = controller_support::fixture_with_clock(
            client,
            SubmissionMode::DryRun,
            "quota-clock-jumps",
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
        while fixture.controller.jobs()[0].phase() != JobPhase::Held {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let original = fixture.controller.jobs()[0].attempts()[0].clone();
        wall.store(4_000_000, Ordering::SeqCst);
        fixture.controller.tick(&fixture.application, None).unwrap();
        assert_eq!(fixture.controller.service_fences().unwrap().len(), 1);
        wall.store(999_000, Ordering::SeqCst);
        fixture.controller.tick(&fixture.application, None).unwrap();
        assert_eq!(fixture.controller.service_fences().unwrap().len(), 1);
        wall.store(1_000_000, Ordering::SeqCst);
        tick.store(10_000, Ordering::SeqCst);
        while fixture.controller.jobs()[0].phase() != JobPhase::Succeeded {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.controller.jobs()[0].attempts()[0], original);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}
