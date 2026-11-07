// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Committed permanent-refusal Draft publication through the app-owned bridge.

use super::controller_support;
use super::recovery::RecoveryBackend;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tallyline_app::api::{CpuWorker, CredentialQueue, HttpDriver, SubmissionQueueBridge};
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::model::{
    JobPhase, SubmissionCorrection, SubmissionRejection, SubmissionResult, UexErrorCode,
};
use tallyline_ui::api::model::{
    BackgroundEvent, ResolutionQueueAction, SubmissionControl, SubmissionFeedback,
};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_ui_16__bridge_publishes_only_committed_copy_and_conflict_failure_feedback_without_resending()
 {
    CpuWorker::start("rejected-copy-publication",|| {
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move || Ok::<_,()>(RecoveryBackend{posts:recorded,result:SubmissionResult::Rejected(SubmissionRejection::new(UexErrorCode::new("opaque_refusal".to_owned()).unwrap(),SubmissionCorrection::ReviewClient))})).unwrap();
        let controller_support::Fixture{mut controller,application,report,path}=controller_support::fixture(client,SubmissionMode::Production,"rejected-copy-publication").unwrap();
        controller.release_manual(report.id(),report.version(),SubmissionMode::Production,&application).unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        while controller.jobs()[0].phase()!=JobPhase::Rejected {
            controller.tick(&application,None).unwrap();
            assert!(std::time::Instant::now()<deadline);
            std::thread::yield_now();
        }
        let original=controller.jobs()[0].clone();
        let (commands,incoming)=std::sync::mpsc::sync_channel(1);
        let (outgoing,events)=std::sync::mpsc::sync_channel(8);
        let mut bridge=SubmissionQueueBridge::new(controller,incoming,outgoing,||{});
        bridge.poll(&application,None,Timestamp::from_seconds(1000)).unwrap();
        drop(events.try_iter().collect::<Vec<_>>());
        let mut perform=|expected| {
            commands.send(SubmissionControl::Resolve{id:original.id(),expected,action:ResolutionQueueAction::DuplicateAsDraft}).unwrap();
            bridge.poll(&application,None,Timestamp::from_seconds(1000)).unwrap();
            events.try_iter().find_map(|event| if let BackgroundEvent::SubmissionUpdated(update) = event { update.completed.then_some(update) } else { None }).unwrap()
        };
        let stale=perform(original.revision()-1);
        assert_eq!(stale.feedback,Some(SubmissionFeedback::Changed));
        assert!(stale.reports.is_empty());
        assert_eq!(stale.jobs,vec![original.clone()]);
        let fault=rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("CREATE TRIGGER fail_bridge_copy BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'injected publication rollback'); END;").unwrap();
        let failed=perform(original.revision());
        assert_eq!(failed.feedback,Some(SubmissionFeedback::Unavailable));
        assert!(failed.reports.is_empty());
        assert_eq!(failed.jobs,vec![original.clone()]);
        fault.execute_batch("DROP TRIGGER fail_bridge_copy").unwrap();
        drop(fault);
        let committed=perform(original.revision());
        assert_eq!(committed.feedback,None);
        assert_eq!(committed.reports.len(),1);
        let draft=&committed.reports[0];
        assert_ne!(draft.id(),report.id());
        assert_eq!(draft.state(),tallyline_reporting::api::model::ReportState::Draft);
        assert!(draft.release_binding().is_none());
        assert_eq!(committed.jobs[0].attempts(),original.attempts());
        assert_eq!(committed.jobs[0].request(),original.request());
        assert_eq!(committed.jobs[0].result(),original.result());
        assert_eq!(committed.jobs[0].revision(),original.revision()+1);
        assert_eq!(committed.jobs[0].resolution_actions().len(),1);
        assert_eq!(posts.load(Ordering::SeqCst),1);
        bridge.poll(&application,None,Timestamp::from_seconds(1000)).unwrap();
        assert_eq!(posts.load(Ordering::SeqCst),1);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}
