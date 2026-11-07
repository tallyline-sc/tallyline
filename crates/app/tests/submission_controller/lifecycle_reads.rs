// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lifecycle completion is committed before publication and failed saves never replay HTTP.

use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64};
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::model::{
    LifecycleReadBatch, LifecycleReadFailure, LifecycleReadResponse, RowEditability,
    UexLifecycleRow, UexReportStatus,
};

struct Clock(Arc<AtomicU64>);
impl tallyline_shared_kernel::api::Clock for Clock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(self.0.load(Ordering::SeqCst))
    }
}
struct LifecycleBackend {
    base: ProductionBackend,
    reads: Arc<AtomicUsize>,
    release: Arc<AtomicBool>,
    status: UexReportStatus,
}
impl HttpBackend for LifecycleBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        self.base.fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        ids: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        self.base.fetch_terminal_priors(ids).await
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        self.base.validate(values).await
    }
    async fn verify_receipts(
        &self,
        job: tallyline_submission::api::model::SubmissionJob,
        values: CredentialValues,
        token: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        self.base.verify_receipts(job, values, token).await
    }
    async fn submit(
        &self,
        prepared: PreparedUexSubmission,
        values: CredentialValues,
        token: Option<String>,
    ) -> SubmissionResult {
        self.base.submit(prepared, values, token).await
    }
    async fn read_lifecycle(
        &self,
        batch: LifecycleReadBatch,
        _: CredentialValues,
        _: Option<String>,
    ) -> Result<LifecycleReadResponse, LifecycleReadFailure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        while !self.release.load(Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let rows = batch
            .claims()
            .iter()
            .flat_map(|claim| claim.targets())
            .map(|proof| {
                UexLifecycleRow::new(proof.clone(), self.status, RowEditability::NotEditable)
            })
            .collect();
        LifecycleReadResponse::try_new(batch, rows)
            .map_err(|_| LifecycleReadFailure::InvalidResponse)
    }
}
struct Scenario {
    fixture: controller_support::Fixture,
    driver: HttpDriver,
    seconds: Arc<AtomicU64>,
    reads: Arc<AtomicUsize>,
    release: Arc<AtomicBool>,
    posts: Arc<AtomicUsize>,
}
fn scenario(name: &str, status: UexReportStatus) -> std::io::Result<Scenario> {
    let reads = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let posts = Arc::new(AtomicUsize::new(0));
    let backend = LifecycleBackend {
        base: ProductionBackend {
            posts: Arc::clone(&posts),
            lookups: Arc::new(AtomicUsize::new(0)),
        },
        reads: Arc::clone(&reads),
        release: Arc::clone(&release),
        status,
    };
    let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(backend))?;
    let seconds = Arc::new(AtomicU64::new(1000));
    let fixture = controller_support::fixture_with_clock(
        client,
        SubmissionMode::Production,
        name,
        Clock(Arc::clone(&seconds)),
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    Ok(Scenario {
        fixture,
        driver,
        seconds,
        reads,
        release,
        posts,
    })
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !condition() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_val_7_r_nf_11__one_pending_read_retains_check_time_across_failed_save_and_publishes_only_committed_evidence()
 {
    tallyline_app::api::CpuWorker::start("tallyline-lifecycle-contract", || {
        let Scenario { mut fixture, driver, seconds, reads, release, posts } = scenario("lifecycle-save", UexReportStatus::Declined).unwrap();
        fixture.controller.release_manual(fixture.report.id(), fixture.report.version(), SubmissionMode::Production, &fixture.application).unwrap();
        until(|| { fixture.controller.tick(&fixture.application, None).unwrap(); !fixture.controller.mappings().is_empty() });
        let original = fixture.controller.jobs()[0].clone();
        seconds.store(1299, Ordering::SeqCst);
        fixture.controller.tick(&fixture.application, None).unwrap();
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        seconds.store(1300, Ordering::SeqCst);
        until(|| { fixture.controller.tick(&fixture.application, None).unwrap(); reads.load(Ordering::SeqCst) == 1 });
        assert!(fixture.controller.lifecycles()[0].observations().is_empty());
        assert!(fixture.controller.refresh_lifecycle(original.id(), original.revision(), Some(fixture.controller.lifecycles()[0].revision()), &fixture.application, None).is_err());
        let failure = rusqlite::Connection::open(&fixture.path).unwrap();
        failure.execute_batch("CREATE TRIGGER fail_lifecycle BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission_lifecycle' BEGIN SELECT RAISE(ABORT,'fixture lifecycle save'); END;").unwrap();
        seconds.store(1301, Ordering::SeqCst);
        release.store(true, Ordering::SeqCst);
        until(|| { fixture.controller.tick(&fixture.application, None).unwrap(); fixture.controller.lifecycle_failure().is_some() });
        assert!(fixture.controller.lifecycles()[0].observations().is_empty());
        seconds.store(1400, Ordering::SeqCst);
        for _ in 0..3 { fixture.controller.tick(&fixture.application, None).unwrap(); }
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        failure.execute_batch("DROP TRIGGER fail_lifecycle;").unwrap();
        until(|| { fixture.controller.tick(&fixture.application, None).unwrap(); !fixture.controller.lifecycles()[0].observations().is_empty() });
        let observed = &fixture.controller.lifecycles()[0].observations()[0];
        assert_eq!(observed.checked_at(), Timestamp::from_seconds(1301));
        assert_eq!(observed.status(), UexReportStatus::Declined);
        assert_eq!(fixture.controller.jobs(), &[original]);
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller); drop(failure); driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    }).unwrap().finish().unwrap();
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_ui_16__manual_deadline_publishes_one_due_transition_without_new_http_or_unrelated_changes()
 {
    tallyline_app::api::CpuWorker::start("tallyline-lifecycle-deadline", || {
        let Scenario {
            mut fixture,
            driver,
            seconds,
            reads,
            release,
            ..
        } = scenario("lifecycle-deadline", UexReportStatus::Pending).unwrap();
        fixture
            .controller
            .release_manual(
                fixture.report.id(),
                fixture.report.version(),
                SubmissionMode::Production,
                &fixture.application,
            )
            .unwrap();
        until(|| {
            fixture.controller.tick(&fixture.application, None).unwrap();
            !fixture.controller.mappings().is_empty()
        });
        let job = fixture.controller.jobs()[0].clone();
        assert!(
            fixture
                .controller
                .refresh_lifecycle(job.id(), job.revision(), None, &fixture.application, None)
                .unwrap()
        );
        release.store(true, Ordering::SeqCst);
        until(|| {
            fixture.controller.tick(&fixture.application, None).unwrap();
            !fixture.controller.lifecycles()[0].observations().is_empty()
        });
        seconds.store(1059, Ordering::SeqCst);
        assert!(!fixture.controller.tick(&fixture.application, None).unwrap());
        seconds.store(1060, Ordering::SeqCst);
        assert!(
            fixture.controller.tick(&fixture.application, None).unwrap(),
            "the manual deadline must publish a new assessment without unrelated progress"
        );
        assert!(
            !fixture.controller.tick(&fixture.application, None).unwrap(),
            "one deadline must not generate a perpetual wake"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

fn receive_projection(
    events: &std::sync::mpsc::Receiver<tallyline_ui::api::model::BackgroundEvent>,
    queue: &mut tallyline_ui::api::model::SubmissionQueue,
) -> usize {
    let mut count = 0;
    for event in events.try_iter() {
        if let tallyline_ui::api::model::BackgroundEvent::SubmissionUpdated(update) = event {
            count += 1;
            queue.received(update.jobs);
            queue.received_schedule_time(update.assessed_at);
            queue.received_lifecycle_busy(update.lifecycle_busy);
            queue.received_lifecycle_account(update.lifecycle_account);
            for mapping in update.mappings {
                queue.received_mapping(mapping);
            }
            for ledger in update.lifecycles {
                queue.received_lifecycle(ledger);
            }
        }
    }
    count
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_val_7_r_ui_16__bridge_routes_manual_read_and_publishes_one_deadline_and_cached_reference_signal()
 {
    use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
    use tallyline_ui::api::model::{SubmissionControl, SubmissionQueue};
    tallyline_app::api::CpuWorker::start("tallyline-lifecycle-bridge", || {
        let Scenario {
            mut fixture,
            driver,
            seconds,
            reads,
            release,
            ..
        } = scenario("lifecycle-bridge", UexReportStatus::Pending).unwrap();
        fixture
            .controller
            .release_manual(
                fixture.report.id(),
                fixture.report.version(),
                SubmissionMode::Production,
                &fixture.application,
            )
            .unwrap();
        until(|| {
            fixture.controller.tick(&fixture.application, None).unwrap();
            !fixture.controller.mappings().is_empty()
        });
        let job = fixture.controller.jobs()[0].clone();
        let (commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(1);
        let publications = Arc::new(AtomicUsize::new(0));
        let published = Arc::clone(&publications);
        let mut bridge = SubmissionQueueBridge::new(fixture.controller, incoming, outgoing, || {})
            .with_reference_publication(move || {
                published.fetch_add(1, Ordering::SeqCst);
                true
            });
        let mut queue = SubmissionQueue::default();
        for _ in 0..4 {
            bridge
                .poll(&fixture.application, None, Timestamp::from_seconds(1000))
                .unwrap();
            receive_projection(&events, &mut queue);
        }
        let baseline = publications.load(Ordering::SeqCst);
        commands
            .send(SubmissionControl::RefreshLifecycle {
                id: job.id(),
                expected: job.revision(),
                expected_lifecycle: None,
            })
            .unwrap();
        bridge
            .poll(&fixture.application, None, Timestamp::from_seconds(1000))
            .unwrap();
        receive_projection(&events, &mut queue);
        assert!(!queue.can_refresh_lifecycle(job.id()));
        release.store(true, Ordering::SeqCst);
        until(|| {
            bridge
                .poll(&fixture.application, None, Timestamp::from_seconds(1000))
                .unwrap();
            receive_projection(&events, &mut queue);
            queue
                .lifecycle(job.id())
                .is_some_and(|ledger| !ledger.observations().is_empty())
        });
        for _ in 0..4 {
            bridge
                .poll(&fixture.application, None, Timestamp::from_seconds(1000))
                .unwrap();
            receive_projection(&events, &mut queue);
        }
        assert_eq!(publications.load(Ordering::SeqCst), baseline + 1);
        seconds.store(1059, Ordering::SeqCst);
        bridge
            .poll(&fixture.application, None, Timestamp::from_seconds(1059))
            .unwrap();
        assert_eq!(receive_projection(&events, &mut queue), 0);
        assert!(!queue.can_refresh_lifecycle(job.id()));
        seconds.store(1060, Ordering::SeqCst);
        bridge
            .poll(&fixture.application, None, Timestamp::from_seconds(1060))
            .unwrap();
        assert_eq!(receive_projection(&events, &mut queue), 1);
        assert!(queue.can_refresh_lifecycle(job.id()));
        bridge
            .poll(&fixture.application, None, Timestamp::from_seconds(1060))
            .unwrap();
        assert_eq!(receive_projection(&events, &mut queue), 0);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        bridge.shutdown(Timestamp::from_seconds(1060)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}
