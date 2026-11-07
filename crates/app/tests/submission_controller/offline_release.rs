// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offline remembered release through the public bridge and durable SQLite ports.

use super::{Backend, RecordedOfflineRefresh, controller_support};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tallyline_app::api::{CredentialQueue, HttpDriver, SubmissionQueueBridge};
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, JobPhase, SavedCredentials, StorageFailure,
    SubmissionWaitReason, ValidatedAccount,
};
use tallyline_submission::api::{CredentialApplication, SecretStore, SubmissionJobRepository};
use tallyline_ui::api::model::{BackgroundEvent, SubmissionControl, SubmissionIntent};

struct DurableStore(SavedCredentials);
impl SecretStore for DurableStore {
    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        Ok(Some(self.0.clone()))
    }
    async fn save(&mut self, saved: &SavedCredentials) -> Result<(), StorageFailure> {
        self.0 = saved.clone();
        Ok(())
    }
    async fn save_passive(&mut self, saved: &SavedCredentials) -> Result<(), StorageFailure> {
        self.save(saved).await
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        Err(StorageFailure::Unavailable)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_7_r_sub_2_r_sub_8__offline_restored_account_releases_durably_and_waits_for_current_validation()
 {
    tallyline_app::api::CpuWorker::start("tallyline-offline-release", || {
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let validator = client.clone();
        let controller_support::Fixture {
            controller,
            report,
            path,
            ..
        } = controller_support::fixture(
            client,
            SubmissionMode::DryRun,
            "offline-remembered-release",
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut store = DurableStore(
            SavedCredentials::from_storage(
                CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))
                    .unwrap(),
                ValidatedAccount::new("dummy-a".to_owned()),
                Timestamp::from_seconds(900),
                None,
            )
            .unwrap(),
        );
        let mut application = runtime
            .block_on(CredentialApplication::load(&mut store))
            .unwrap();
        let request = application.begin_revalidation().unwrap();
        assert_eq!(
            runtime.block_on(application.accept(
                request,
                Err(CredentialFailure::Unavailable),
                Timestamp::from_seconds(1000),
                &mut store
            )),
            Err(CredentialFailure::Unavailable)
        );
        let (commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(4);
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {})
            .with_release_refresh(RecordedOfflineRefresh);
        commands
            .send(SubmissionControl::Release {
                id: report.id(),
                expected: report.version(),
                account: "dummy-a".to_owned(),
                intent: SubmissionIntent::Manual(SubmissionMode::DryRun),
            })
            .unwrap();
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let BackgroundEvent::SubmissionUpdated(update) = events.try_recv().unwrap() else {
            panic!("release acknowledgement expected")
        };
        assert!(update.completed);
        assert_eq!(update.feedback, None);
        assert_eq!(
            update.jobs.len(),
            1,
            "remembered validation permits an offline durable release"
        );
        let id = update.jobs[0].id();
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let queued = SubmissionJobRepository::load(&mut observer, id)
            .unwrap()
            .unwrap();
        assert_eq!(queued.phase(), JobPhase::Queued);
        assert!(queued.attempts().is_empty());
        assert_eq!(queued.request().header().account, "dummy-a");
        assert_eq!(
            update.waiting[0].reason,
            SubmissionWaitReason::AccountUnavailable
        );
        for _ in 0..4 {
            bridge
                .poll(&application, None, Timestamp::from_seconds(1000))
                .unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        let request = application.begin_revalidation().unwrap();
        let result = runtime.block_on(request.validate(&validator));
        runtime
            .block_on(application.accept(
                request,
                result,
                Timestamp::from_seconds(1000),
                &mut store,
            ))
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            bridge
                .poll(&application, None, Timestamp::from_seconds(1000))
                .unwrap();
            while events.try_recv().is_ok() {}
            if SubmissionJobRepository::load(&mut observer, id)
                .unwrap()
                .unwrap()
                .phase()
                == JobPhase::Succeeded
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        let succeeded = SubmissionJobRepository::load(&mut observer, id)
            .unwrap()
            .unwrap();
        assert_eq!(succeeded.attempts().len(), 1);
        assert_eq!(
            succeeded.result(),
            Some(&tallyline_submission::api::model::SubmissionResult::DryRunSucceeded)
        );
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        drop(observer);
        drop(validator);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}
