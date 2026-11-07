// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit unknown decisions on the real queue owner never replay the original POST.

use super::controller_support;
use super::receipt_support::{EvidenceBackend, EvidenceResponse};
use super::recovery::RecoveryBackend;
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::time::Instant;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_app::api::SubmissionControlError;
use tallyline_app::api::{CpuWorker, HttpDriver};
use tallyline_reporting::api::ReportRepository;
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::SubmissionRepositoryError;
use tallyline_submission::api::model::{JobPhase, ReceiptDecision, SubmissionResult};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__controller_received_decision_commits_without_repeating_unknown_post() {
    CpuWorker::start("unknown-decision-contract", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: count,
                result: SubmissionResult::Unknown,
            })
        })
        .unwrap();
        let mut fixture =
            controller_support::fixture(client, SubmissionMode::Production, "unknown-decision")
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
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            fixture.controller.tick(&fixture.application, None).unwrap();
            if fixture.controller.jobs().iter().any(|job| {
                job.id() == released.job().id() && job.phase() == JobPhase::OutcomeUnknown
            }) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let unknown = fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == released.job().id())
            .unwrap()
            .clone();
        let received = fixture.controller.decide_unknown(
            unknown.id(),
            unknown.revision(),
            ReceiptDecision::Received,
        );
        assert!(
            received.is_ok(),
            "the application owner must commit the explicit decision: {received:?}"
        );
        let received = received.unwrap();
        assert_eq!(received.unknown_decision(), Some(ReceiptDecision::Received));
        assert_eq!(received.attempts(), unknown.attempts());
        assert_eq!(received.phase(), JobPhase::OutcomeUnknown);
        for _ in 0..5 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert!(
            fixture
                .controller
                .decide_unknown(
                    unknown.id(),
                    unknown.revision(),
                    ReceiptDecision::NotReceived
                )
                .is_err()
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
fn r_sub_9__stale_async_unknown_proof_after_decision_and_duplicate_is_refused_but_fresh_proof_is_attributed()
 {
    CpuWorker::start("unknown-proof-contract", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(AtomicBool::new(false));
        let backend = EvidenceBackend {
            response: EvidenceResponse::Positive,
            base: RecoveryBackend {
                posts: Arc::clone(&posts),
                result: SubmissionResult::Unknown,
            },
            reads: Arc::clone(&reads),
            release: Arc::clone(&release),
        };
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(backend)).unwrap();
        let mut fixture =
            controller_support::fixture(client, SubmissionMode::Production, "unknown-proof")
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
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            fixture.controller.tick(&fixture.application, None).unwrap();
            if fixture.controller.jobs().iter().any(|job| {
                job.id() == released.job().id() && job.phase() == JobPhase::OutcomeUnknown
            }) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let unknown = fixture.controller.jobs()[0].clone();
        let started = fixture.controller.check_receipts(
            unknown.id(),
            unknown.revision(),
            &fixture.application,
            None,
        );
        assert!(
            started.is_ok(),
            "the explicit check must admit a read-only unknown-origin lookup: {started:?}"
        );
        while reads.load(Ordering::SeqCst) != 1 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let decided = fixture
            .controller
            .decide_unknown(
                unknown.id(),
                unknown.revision(),
                ReceiptDecision::NotReceived,
            )
            .unwrap();
        let duplicated = fixture
            .controller
            .duplicate_resolved(decided.id(), decided.revision())
            .unwrap();
        release.store(true, Ordering::SeqCst);
        let mut stale_refused = false;
        while !stale_refused {
            match fixture.controller.tick(&fixture.application, None) {
                Err(SubmissionControlError::Conflict) => stale_refused = true,
                other => {
                    other.unwrap();
                }
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(fixture.controller.jobs()[0].verified_row_receipts().count(), 0);
        assert_eq!(fixture.controller.jobs()[0].resolution_actions().len(), 2);
        let fault = Connection::open(&fixture.path).unwrap();
        fault.execute_batch("CREATE TRIGGER fail_unknown_proof_save BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'test proof save retry'); END;").unwrap();
        fixture
            .controller
            .check_receipts(
                duplicated.origin.id(),
                duplicated.origin.revision(),
                &fixture.application,
                None,
            )
            .unwrap();
        loop {
            match fixture.controller.tick(&fixture.application, None) {
                Err(SubmissionControlError::Repository(SubmissionRepositoryError::Unavailable)) => break,
                other => { other.unwrap(); }
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.controller.jobs()[0].verified_row_receipts().count(), 0);
        fault.execute_batch("DROP TRIGGER fail_unknown_proof_save").unwrap();
        drop(fault);
        while fixture.controller.jobs()[0].verified_row_receipts().count() == 0 {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            fixture.controller.jobs()[0].unknown_decision(),
            Some(ReceiptDecision::NotReceived)
        );
        assert_eq!(fixture.controller.jobs()[0].resolution_actions().len(), 3);
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        let mut database =
            SqliteReferenceCache::open(&fixture.path).unwrap();
        assert_eq!(
            ReportRepository::drafts(&mut database).unwrap(),
            vec![duplicated.draft]
        );
        drop(database);
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
fn r_sub_9__empty_unknown_checks_never_decide_duplicate_or_replay_even_after_reopen() {
    CpuWorker::start("unknown-empty-contract", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let backend = EvidenceBackend {
            response: EvidenceResponse::Empty,
            base: RecoveryBackend {
                posts: Arc::clone(&posts),
                result: SubmissionResult::Unknown,
            },
            reads: Arc::clone(&reads),
            release: Arc::new(AtomicBool::new(true)),
        };
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(backend)).unwrap();
        let mut fixture =
            controller_support::fixture(client, SubmissionMode::Production, "unknown-empty")
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
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            fixture.controller.tick(&fixture.application, None).unwrap();
            if fixture.controller.jobs().iter().any(|job| {
                job.id() == released.job().id() && job.phase() == JobPhase::OutcomeUnknown
            }) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let unknown = fixture.controller.jobs()[0].clone();
        fixture
            .controller
            .check_receipts(unknown.id(), unknown.revision(), &fixture.application, None)
            .unwrap();
        loop {
            fixture.controller.tick(&fixture.application, None).unwrap();
            if reads.load(Ordering::SeqCst) == 1
                && fixture
                    .controller
                    .check_receipts(unknown.id(), unknown.revision(), &fixture.application, None)
                    .is_ok()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        while reads.load(Ordering::SeqCst) < 2 {
            fixture.controller.tick(&fixture.application, None).unwrap();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        for _ in 0..5 {
            fixture.controller.tick(&fixture.application, None).unwrap();
        }
        assert_eq!(fixture.controller.jobs(), std::slice::from_ref(&unknown));
        assert!(!unknown.can_duplicate_unknown());
        assert!(unknown.unknown_decision().is_none());
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        let mut reopened = SqliteReferenceCache::open(&fixture.path).unwrap();
        assert_eq!(
            SubmissionJobRepository::load(&mut reopened, unknown.id())
                .unwrap()
                .as_ref(),
            Some(&unknown)
        );
        drop(reopened);
        std::fs::remove_file(fixture.path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}
