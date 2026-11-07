// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit queue-wide service acknowledgement without replaying a cancelled Report.

use super::{controller_support, recovery::RecoveryBackend, support};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tallyline_app::api::HttpDriver;
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::model::{JobPhase, SubmissionHoldReason, SubmissionResult};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11__cancelled_api_change_cause_needs_explicit_queue_retry_without_replaying_it() {
    tallyline_app::api::CpuWorker::start("tallyline-api-ack", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::Held(SubmissionHoldReason::ApiChanged),
            })
        })
        .unwrap();
        let mut fixture = controller_support::fixture(
            client,
            SubmissionMode::DryRun,
            &format!(
                "api-ack-{}",
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
        let (store, second) = support::stored_gate(&fixture.path).unwrap();
        drop(store);
        let second = fixture
            .controller
            .release_manual(
                second.source().id(),
                second.source().version(),
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
        let revision = fixture.controller.jobs()[0].revision();
        let cancelled = fixture
            .controller
            .cancel(first.job().id(), revision)
            .unwrap();
        for _ in 0..50 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        fixture
            .controller
            .retry_now(cancelled.id(), cancelled.revision())
            .unwrap();
        while fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == second.job().id())
            .unwrap()
            .phase()
            != JobPhase::Held
        {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let original = fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == first.job().id())
            .unwrap();
        assert_eq!(original.phase(), JobPhase::Cancelled);
        assert_eq!(original.attempts().len(), 1);
        assert_eq!(posts.load(Ordering::SeqCst), 2);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        let mut store =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
        let original =
            tallyline_submission::api::SubmissionJobRepository::load(&mut store, first.job().id())
                .unwrap()
                .unwrap();
        assert_eq!(original.phase(), JobPhase::Cancelled);
        assert_eq!(original.attempts().len(), 1);
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
fn r_sub_2_r_sub_11__global_daily_quota_fence_survives_origin_cancellation_and_restart() {
    tallyline_app::api::CpuWorker::start("tallyline-quota-fence", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::NotProcessed(
                    tallyline_submission::api::model::NotProcessed::DailyQuota,
                ),
            })
        })
        .unwrap();
        let mut fixture = controller_support::fixture(
            client,
            SubmissionMode::DryRun,
            &format!(
                "quota-fence-{}",
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase() != JobPhase::Held {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let fences = fixture.controller.service_fences().unwrap();
        assert_eq!(fences.len(), 1);
        assert_eq!(
            fences[0].cause(),
            tallyline_submission::api::model::SubmissionServiceCause::DailyQuota
        );
        assert_eq!(fences[0].source(), first.job().id());
        let cancelled = fixture
            .controller
            .cancel(first.job().id(), fixture.controller.jobs()[0].revision())
            .unwrap();
        assert_eq!(fixture.controller.service_fences().unwrap(), fences);
        assert!(
            fixture
                .controller
                .retry_now(cancelled.id(), cancelled.revision())
                .is_err()
        );
        let (store, second) = support::stored_gate(&fixture.path).unwrap();
        drop(store);
        fixture
            .controller
            .release_manual(
                second.source().id(),
                second.source().version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .unwrap();
        for _ in 0..50 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        let mut store =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
        tallyline_workflows::api::SubmissionQueueStore::recover_unfinished(
            &mut store,
            tallyline_shared_kernel::api::model::Timestamp::from_seconds(1010),
        )
        .unwrap();
        assert_eq!(
            tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(&mut store)
                .unwrap(),
            fences
        );
        let fault = rusqlite::Connection::open(&fixture.path).unwrap();
        fault
            .execute(
                "UPDATE submission_service_fences SET source_revision=source_revision+1000",
                [],
            )
            .unwrap();
        assert_eq!(
            tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(&mut store),
            Err(tallyline_submission::api::model::SubmissionRepositoryError::Invalid)
        );
        drop(fault);
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
fn r_sub_2_r_sub_11_r_nf_11__failed_service_fence_save_retries_only_the_local_outcome() {
    for result in [
        SubmissionResult::NotProcessed(tallyline_submission::api::model::NotProcessed::DailyQuota),
        SubmissionResult::AccountRefusal {
            check: tallyline_submission::api::model::SubmissionAccountCheck::NotRequired,
            code: tallyline_submission::api::model::UexErrorCode::new("no_api_found".to_owned()),
            http_status: 403,
            reason: tallyline_submission::api::model::SubmissionPauseReason::AppToken,
            outcome: Box::new(SubmissionResult::NeedsOutcomeCheck {
                code: tallyline_submission::api::model::UexErrorCode::new(
                    "no_api_found".to_owned(),
                )
                .unwrap(),
                http_status: 403,
            }),
        },
    ] {
        tallyline_app::api::CpuWorker::start("tallyline-fence-save", move || {
        let posts=Arc::new(AtomicUsize::new(0));let backend_posts=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move||Ok::<_,()>(RecoveryBackend { posts:backend_posts,result })).unwrap();
        let mut fixture=controller_support::fixture(client,SubmissionMode::DryRun,&format!("fence-save-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())).unwrap();
        let first=fixture.controller.release_manual(fixture.report.id(),fixture.report.version(),SubmissionMode::DryRun,&fixture.application).unwrap();
        let (store,second)=support::stored_gate(&fixture.path).unwrap();drop(store);
        let second=fixture.controller.release_manual(second.source().id(),second.source().version(),SubmissionMode::DryRun,&fixture.application).unwrap();
        let fault=rusqlite::Connection::open(&fixture.path).unwrap();
        fault.execute_batch("CREATE TRIGGER fail_service_fence BEFORE INSERT ON submission_service_fences BEGIN SELECT RAISE(ABORT,'test service fence save'); END;").unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            if let Err(error) = fixture.controller.tick(&fixture.application,None) {assert_eq!(error,tallyline_app::api::SubmissionControlError::Repository(tallyline_submission::api::model::SubmissionRepositoryError::Unavailable));break;}
            assert!(std::time::Instant::now()<deadline);std::thread::yield_now();
        }
        assert_eq!(posts.load(Ordering::SeqCst),1);
        assert_eq!(fixture.controller.jobs().iter().find(|job|job.id()==first.job().id()).unwrap().phase(),JobPhase::Sending);
        assert!(fixture.controller.service_fences().unwrap().is_empty());
        for _ in 0..3 {assert!(fixture.controller.tick(&fixture.application,None).is_err());}
        assert_eq!(posts.load(Ordering::SeqCst),1);
        fault.execute_batch("DROP TRIGGER fail_service_fence;").unwrap();
        while fixture.controller.jobs().iter().find(|job|job.id()==first.job().id()).unwrap().phase()!=JobPhase::Held {fixture.controller.tick(&fixture.application,None).unwrap();assert!(std::time::Instant::now()<deadline);std::thread::yield_now();}
        for _ in 0..30 {fixture.controller.tick(&fixture.application,None).unwrap();}
        assert_eq!(fixture.controller.service_fences().unwrap().len(),1);
        assert_eq!(fixture.controller.jobs().iter().find(|job|job.id()==second.job().id()).unwrap().attempts().len(),0);
        assert_eq!(posts.load(Ordering::SeqCst),1);
        fixture.controller.shutdown().unwrap();drop(fixture.controller);drop(fault);driver.finish().unwrap();std::fs::remove_file(fixture.path).unwrap();
    }).unwrap().finish().unwrap();
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11__cancelled_account_pause_survives_restart_and_requires_explicit_queue_retry() {
    tallyline_app::api::CpuWorker::start("tallyline-account-ack", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::Paused(
                    tallyline_submission::api::model::SubmissionPauseReason::KeyChecked,
                ),
            })
        })
        .unwrap();
        let mut fixture = controller_support::fixture(
            client,
            SubmissionMode::DryRun,
            &format!(
                "account-ack-{}",
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
        let (store, second) = support::stored_gate(&fixture.path).unwrap();
        drop(store);
        let second = fixture
            .controller
            .release_manual(
                second.source().id(),
                second.source().version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase() != JobPhase::Paused {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let revision = fixture.controller.jobs()[0].revision();
        let cancelled = fixture
            .controller
            .cancel(first.job().id(), revision)
            .unwrap();
        let mut restarted_store =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();

        let persisted =
            tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(
                &mut restarted_store,
            )
            .unwrap();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].source(), cancelled.id());
        drop(restarted_store);
        for _ in 0..50 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        fixture
            .controller
            .retry_now(cancelled.id(), cancelled.revision())
            .unwrap();
        while fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == second.job().id())
            .unwrap()
            .phase()
            != JobPhase::Paused
        {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let original = fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == first.job().id())
            .unwrap();
        assert_eq!(original.phase(), JobPhase::Cancelled);
        assert_eq!(original.attempts().len(), 1);
        assert_eq!(posts.load(Ordering::SeqCst), 2);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        let mut store =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
        let original =
            tallyline_submission::api::SubmissionJobRepository::load(&mut store, first.job().id())
                .unwrap()
                .unwrap();
        assert_eq!(original.phase(), JobPhase::Cancelled);
        assert_eq!(original.attempts().len(), 1);
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
fn r_sub_11__global_account_pause_fence_survives_origin_cancellation_and_recovery() {
    super::permanent_refusal::verify().unwrap();
    super::repeated_account::verify().unwrap();
    super::blocked_read::verify().unwrap();
    super::account_receipt::verify().unwrap();
    tallyline_app::api::CpuWorker::start("tallyline-account-fence", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result: SubmissionResult::Paused(
                    tallyline_submission::api::model::SubmissionPauseReason::UpdateRequired,
                ),
            })
        })
        .unwrap();
        let mut fixture = controller_support::fixture(
            client,
            SubmissionMode::DryRun,
            &format!(
                "account-fence-{}",
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture.controller.jobs()[0].phase() != JobPhase::Paused {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let fences = fixture.controller.service_fences().unwrap();
        assert_eq!(fences.len(), 1);
        assert_eq!(
            fences[0].cause(),
            tallyline_submission::api::model::SubmissionServiceCause::AccountPause
        );
        assert_eq!(fences[0].source(), first.job().id());
        let cancelled = fixture
            .controller
            .cancel(first.job().id(), fixture.controller.jobs()[0].revision())
            .unwrap();
        assert_eq!(fixture.controller.service_fences().unwrap(), fences);
        assert!(
            fixture
                .controller
                .retry_now(cancelled.id(), cancelled.revision())
                .is_err()
        );
        let (store, second) = support::stored_gate(&fixture.path).unwrap();
        drop(store);
        fixture
            .controller
            .release_manual(
                second.source().id(),
                second.source().version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .unwrap();
        for _ in 0..50 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        let mut store =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&fixture.path).unwrap();
        tallyline_workflows::api::SubmissionQueueStore::recover_unfinished(
            &mut store,
            tallyline_shared_kernel::api::model::Timestamp::from_seconds(1010),
        )
        .unwrap();
        assert_eq!(
            tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(&mut store)
                .unwrap(),
            fences
        );
        let fault = rusqlite::Connection::open(&fixture.path).unwrap();
        fault
            .execute(
                "UPDATE submission_service_fences SET source_revision=source_revision+1000",
                [],
            )
            .unwrap();
        assert_eq!(
            tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(&mut store),
            Err(tallyline_submission::api::model::SubmissionRepositoryError::Invalid)
        );
        drop(fault);
        drop(store);
        driver.finish().unwrap();
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}
