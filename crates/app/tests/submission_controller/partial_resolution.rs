// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public controller partial-resolution contracts without a repeated POST.

use super::controller_support;
use super::receipt_support::{EvidenceBackend, EvidenceResponse};
use super::recovery::RecoveryBackend;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_app::api::{CpuWorker, HttpDriver, SubmissionControlError};
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    JobPhase, ReceiptDecision, SubmissionResult, SubmissionRowResult,
};

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.0.exists() {
            assert!(
                std::fs::remove_file(&self.0).is_ok(),
                "partial controller database cleanup failed"
            );
        }
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_val_7__initial_partial_outcome_verifies_positive_rows_once_without_replaying_post() {
    CpuWorker::start("partial-initial-proof-contract",||{
        let posts=Arc::new(AtomicUsize::new(0));let reads=Arc::new(AtomicUsize::new(0));
        let post_count=Arc::clone(&posts);let read_count=Arc::clone(&reads);
        let (driver,client)=HttpDriver::start(move||Ok::<_,()>(EvidenceBackend{response:EvidenceResponse::Positive,base:RecoveryBackend{posts:post_count,result:SubmissionResult::Partial(vec![])},reads:read_count,release:Arc::new(AtomicBool::new(true))})).unwrap();
        let cleanup;
        let mut fixture=controller_support::fixture(client,SubmissionMode::Production,"partial-initial-proof").unwrap();
        cleanup=Cleanup(fixture.path.clone());
        let released=fixture.controller.release_manual(fixture.report.id(),fixture.report.version(),SubmissionMode::Production,&fixture.application).unwrap();
        let deadline=Instant::now()+Duration::from_secs(5);
        loop{
            fixture.controller.tick(&fixture.application,None).unwrap();
            if fixture.controller.jobs().iter().any(|job|job.id()==released.job().id() && job.verified_row_receipts().next().is_some()){break;}
            assert!(Instant::now()<deadline,"the committed incomplete production acceptance must schedule its initial positive receipt check");
            std::thread::sleep(Duration::from_millis(5));
        }
        let job=fixture.controller.jobs().iter().find(|job|job.id()==released.job().id()).unwrap();
        assert_eq!(job.phase(),JobPhase::PartiallyAccepted);
        assert_eq!(job.result(),Some(&SubmissionResult::Partial(vec![])));
        assert_eq!(job.attempts().len(),1);
        assert_eq!(job.verified_row_receipts().next().unwrap().id(),718);
        for _ in 0..5{fixture.controller.tick(&fixture.application,None).unwrap();}
        assert_eq!(posts.load(Ordering::SeqCst),1);assert_eq!(reads.load(Ordering::SeqCst),1);
        fixture.controller.shutdown().unwrap();drop(fixture.controller);driver.finish().unwrap();drop(cleanup);
    }).unwrap().finish().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__received_row_statement_survives_reopen_without_remote_identity_or_replay() {
    CpuWorker::start("partial-row-decision-contract", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let post_count = Arc::clone(&posts);
        let read_count = Arc::clone(&reads);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(EvidenceBackend {
                response: EvidenceResponse::Empty,
                base: RecoveryBackend {
                    posts: post_count,
                    result: SubmissionResult::Partial(vec![]),
                },
                reads: read_count,
                release: Arc::new(AtomicBool::new(true)),
            })
        })
        .unwrap();
        let cleanup;
        let mut fixture =
            controller_support::fixture(client, SubmissionMode::Production, "partial-row-received")
                .unwrap();
        cleanup = Cleanup(fixture.path.clone());
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
                job.id() == released.job().id() && job.phase() == JobPhase::PartiallyAccepted
            }) && reads.load(Ordering::SeqCst) == 1
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let original = fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == released.job().id())
            .unwrap()
            .clone();
        let row = original.request().rows()[0].id();
        let received = fixture
            .controller
            .decide_row(
                original.id(),
                original.revision(),
                row,
                ReceiptDecision::Received,
            )
            .unwrap();
        assert_eq!(received.phase(), JobPhase::PartiallyAccepted);
        assert_eq!(received.attempts(), original.attempts());
        assert_eq!(received.result(), original.result());
        assert_eq!(
            received.row_result(row),
            Some(SubmissionRowResult::ReceivedByUser)
        );
        assert!(received.verified_row_receipts().next().is_none());
        assert!(matches!(
            fixture.controller.decide_row(
                original.id(),
                original.revision(),
                row,
                ReceiptDecision::NotReceived
            ),
            Err(SubmissionControlError::Conflict)
        ));
        assert!(
            fixture
                .controller
                .decide_row(
                    received.id(),
                    received.revision(),
                    row,
                    ReceiptDecision::NotReceived
                )
                .is_err()
        );
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        fixture.controller.shutdown().unwrap();
        drop(fixture.controller);
        driver.finish().unwrap();
        let mut reopened = SqliteReferenceCache::open(&fixture.path).unwrap();
        let loaded = SubmissionJobRepository::load(&mut reopened, received.id())
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.row_result(row),
            Some(SubmissionRowResult::ReceivedByUser)
        );
        assert!(loaded.verified_row_receipts().next().is_none());
        drop(reopened);
        drop(cleanup);
    })
    .unwrap()
    .finish()
    .unwrap();
}
