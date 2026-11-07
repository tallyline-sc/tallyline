// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atomic split commands on the real SQLite queue owner with an owned HTTP backend.

use super::{Backend, controller_support, three_rows};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_app::api::{CpuWorker, HttpDriver};
use tallyline_reference_data::api::ReferenceDataCache;
use tallyline_reference_data::api::model::CachedReference;
use tallyline_shared_kernel::api::model::SubmissionMode;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::CredentialApplication;
use tallyline_submission::api::model::{JobPhase, SendReturnReason};

struct SplitClock(Arc<AtomicU64>);
impl tallyline_shared_kernel::api::Clock for SplitClock {
    fn now(&self) -> tallyline_shared_kernel::api::model::Timestamp {
        tallyline_shared_kernel::api::model::Timestamp::from_seconds(self.0.load(Ordering::SeqCst))
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_4_r_sub_2__controller_split_sends_only_ready_rows_and_failed_atomic_save_sends_nothing() {
    CpuWorker::start("cooldown-split-controller-contract", || {
        let posts=Arc::new(AtomicUsize::new(0));
        let count=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move||Ok::<_,()>(Backend(count))).unwrap();
        let time=Arc::new(AtomicU64::new(1000));
        let mut fixture=controller_support::fixture_with_clock(client,SubmissionMode::DryRun,"cooldown-split",SplitClock(Arc::clone(&time))).unwrap();
        let prior=fixture.controller.release_manual(fixture.report.id(),fixture.report.version(),SubmissionMode::DryRun,&fixture.application).unwrap();
        let deadline=Instant::now()+Duration::from_secs(5);
        loop {
            fixture.controller.tick(&fixture.application,None).unwrap();
            if fixture.controller.jobs().iter().any(|job|job.id()==prior.job().id()&&job.phase()==JobPhase::Succeeded) {break;}
            assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(5));
        }
        let (store,gate)=three_rows::gate(&fixture.path).unwrap();
        drop(store);
        let released=fixture.controller.release_manual(gate.source().id(),gate.source().version(),SubmissionMode::DryRun,&fixture.application).unwrap();
        let origin=released.job().clone();
        assert!(fixture.controller.split_cooldown(origin.id(),origin.revision(),&CredentialApplication::default(),None).is_err());
        let db=rusqlite::Connection::open(&fixture.path).unwrap();
        db.execute_batch("CREATE TRIGGER refuse_split BEFORE INSERT ON submission_splits BEGIN SELECT RAISE(ABORT,'injected split failure'); END;").unwrap();
        assert!(fixture.controller.split_cooldown(origin.id(),origin.revision(),&fixture.application,None).is_err());
        for _ in 0..5 {fixture.controller.tick(&fixture.application,None).unwrap();}
        assert_eq!(posts.load(Ordering::SeqCst),1);
        assert_eq!(fixture.controller.jobs().iter().find(|job|job.id()==origin.id()).unwrap(),&origin);
        db.execute_batch("DROP TRIGGER refuse_split").unwrap();
        let split=fixture.controller.split_cooldown(origin.id(),origin.revision(),&fixture.application,None).unwrap();
        fixture.controller.tick(&CredentialApplication::default(),None).unwrap();
        assert_eq!(posts.load(Ordering::SeqCst),1);
        assert_eq!(split.ready().job().request().rows().len(),2);
        assert_eq!(split.cooling().job().request().rows().len(),1);
        assert!(fixture.controller.split_cooldown(origin.id(),origin.revision(),&fixture.application,None).is_err());
        let deadline=Instant::now()+Duration::from_secs(5);
        loop {
            fixture.controller.tick(&fixture.application,None).unwrap();
            if fixture.controller.jobs().iter().any(|job|job.id()==split.ready().job().id()&&job.phase()==JobPhase::Succeeded) {break;}
            assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(posts.load(Ordering::SeqCst),2);
        assert_eq!(fixture.controller.jobs().iter().find(|job|job.id()==split.cooling().job().id()).unwrap().phase(),JobPhase::Queued);
        let mut references=SqliteReferenceCache::open(&fixture.path).unwrap();
        for record in references.load().unwrap() {
            references.replace(&CachedReference::new(Timestamp::from_seconds(87400),record.data().clone())).unwrap();
        }
        drop(references);
        time.store(87401,Ordering::SeqCst);
        let deadline=Instant::now()+Duration::from_secs(5);
        loop {
            let progress=fixture.controller.tick(&fixture.application,None);
            let cooling=fixture.controller.jobs().iter().find(|job|job.id()==split.cooling().job().id()).unwrap();
            assert_eq!(posts.load(Ordering::SeqCst),2);
            if cooling.phase()==JobPhase::Returned {
                assert!(progress.is_err());
                assert_eq!(cooling.review_reason(),Some(SendReturnReason::ObservationTooOld));
                break;
            }
            assert!(progress.is_ok());
            assert!(Instant::now()<deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        fixture.controller.shutdown().unwrap();drop(fixture.controller);drop(db);driver.finish().unwrap();std::fs::remove_file(fixture.path).unwrap();
    }).unwrap().finish().unwrap();
}
