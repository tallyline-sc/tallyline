// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real queue control with SQLite and a fake owned HTTP backend, never a remote write.

#[path = "../../workflows/tests/support/mod.rs"]
mod support;

#[path = "submission_controller/controller_support.rs"]
mod controller_support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{HttpBackend, HttpDriver};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::TerminalId;
use tallyline_submission::api::SecretStore;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SavedCredentials, StorageFailure, SubmissionResult,
    ValidatedAccount,
};

struct Backend(Arc<AtomicUsize>);
impl HttpBackend for Backend {
    async fn fetch(&self, _: Endpoint) -> Result<EndpointData, ReadError> {
        Err(ReadError::Transport("recorded unavailable".to_owned()))
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
        self.0.fetch_add(1, Ordering::SeqCst);
        SubmissionResult::DryRunSucceeded
    }
}
struct Store;
impl SecretStore for Store {
    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        Ok(None)
    }
    async fn save(&mut self, _: &SavedCredentials) -> Result<(), StorageFailure> {
        Ok(())
    }
    async fn save_passive(&mut self, _: &SavedCredentials) -> Result<(), StorageFailure> {
        Ok(())
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        Ok(())
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__controller_retains_a_known_result_after_failed_save_and_never_repeats_the_post()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract",||{
        use tallyline_app::api::{SubmissionQueueController,SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{AcceptanceSwitch,CommodityConstraints,ReferenceParameters,SubmissionAcceptance,CachedReference};
        use tallyline_shared_kernel::api::model::{ObservationAgeSettings,SubmissionMode,Timestamp};
        use tallyline_submission::api::{CredentialApplication,SubmissionJobRepository};
        use tallyline_submission::api::model::{CredentialCandidate,Persistence,JobPhase};
        let path=std::env::temp_dir().join(format!("tallyline-controller-{}.db",std::process::id()));
        let (mut database,draft)=support::stored_gate(&path).unwrap();
        database.replace(&CachedReference::new(Timestamp::from_seconds(950),EndpointData::Parameters(ReferenceParameters::new(SubmissionAcceptance{datacenter:AcceptanceSwitch::Enabled,reports:AcceptanceSwitch::Enabled,commodity:AcceptanceSwitch::Enabled,ptu:AcceptanceSwitch::Disabled},CommodityConstraints{price_variation_percent:20,prior_ttl_days:std::num::NonZeroU32::new(15).unwrap(),scu_variation_percent:20},None)))).unwrap();
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move ||Ok::<_,()>(Backend(recorded))).unwrap();
        let mut application=CredentialApplication::default();
        let request=application.begin(CredentialCandidate::new(CredentialValues::new("dummy-key".to_owned(),Some("dummy-token".to_owned())).unwrap(),Persistence::Session));
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(application.accept(request,Ok(ValidatedAccount::new("dummy-a".to_owned())),Timestamp::from_seconds(1000),&mut Store)).unwrap();
        let mut controller=SubmissionQueueController::new(database,client,move ||Ok(SubmissionQueuePreferences{reviewer:ManualReviewer::new(EnvironmentMapping::default(),StockSettings::default(),None),age:ObservationAgeSettings::default(),mode:SubmissionMode::DryRun}),None,support::Fixed).unwrap();
        let released=controller.release_manual(draft.source().id(),draft.source().version(),SubmissionMode::DryRun,&application).unwrap();
        let failure=rusqlite::Connection::open(&path).unwrap();
        failure.execute_batch("CREATE TRIGGER fail_outcome BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'test outcome save'); END;").unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);

        loop {
            let result=controller.tick(&application,None);

            if result.is_err(){break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(posts.load(Ordering::SeqCst),1);
        assert_eq!(controller.jobs()[0].phase(),JobPhase::Sending);
        let mut observer=tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let sending=SubmissionJobRepository::load(&mut observer,released.job().id()).unwrap().unwrap();
        assert_eq!(sending.phase(),JobPhase::Sending);
        assert_eq!(sending.attempts().len(),1);
        failure.execute_batch("DROP TRIGGER fail_outcome").unwrap();
        controller.tick(&application,None).unwrap();
        assert_eq!(controller.jobs()[0].phase(),JobPhase::Succeeded);
        assert_eq!(posts.load(Ordering::SeqCst),1);
        assert_eq!(controller.jobs()[0].result(),Some(&SubmissionResult::DryRunSucceeded));
        assert_eq!(controller.jobs()[0].attempts().len(),1);
        controller.shutdown().unwrap();
        drop(controller);
        drop(observer);
        drop(failure);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__failure_after_write_ahead_returns_for_renewed_review_without_a_post()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract",||{
        use tallyline_app::api::{SubmissionQueueController,SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{AcceptanceSwitch,CommodityConstraints,ReferenceParameters,SubmissionAcceptance,CachedReference};
        use tallyline_shared_kernel::api::model::{ObservationAgeSettings,SubmissionMode,Timestamp};
        use tallyline_submission::api::{CredentialApplication,SubmissionJobRepository};
        use tallyline_submission::api::model::{CredentialCandidate,Persistence,JobPhase};
        let path=std::env::temp_dir().join(format!("tallyline-controller-after-begin-{}.db",std::process::id()));
        let (mut database,draft)=support::stored_gate(&path).unwrap();
        database.replace(&CachedReference::new(Timestamp::from_seconds(950),EndpointData::Parameters(ReferenceParameters::new(SubmissionAcceptance{datacenter:AcceptanceSwitch::Enabled,reports:AcceptanceSwitch::Enabled,commodity:AcceptanceSwitch::Enabled,ptu:AcceptanceSwitch::Disabled},CommodityConstraints{price_variation_percent:20,prior_ttl_days:std::num::NonZeroU32::new(15).unwrap(),scu_variation_percent:20},None)))).unwrap();
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move ||Ok::<_,()>(Backend(recorded))).unwrap();
        let mut application=CredentialApplication::default();
        let request=application.begin(CredentialCandidate::new(CredentialValues::new("dummy-key".to_owned(),Some("dummy-token".to_owned())).unwrap(),Persistence::Session));
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(application.accept(request,Ok(ValidatedAccount::new("dummy-a".to_owned())),Timestamp::from_seconds(1000),&mut Store)).unwrap();
        let mut controller=SubmissionQueueController::new(database,client,move ||Ok(SubmissionQueuePreferences{reviewer:ManualReviewer::new(EnvironmentMapping::default(),StockSettings::default(),None),age:ObservationAgeSettings::default(),mode:SubmissionMode::DryRun}),None,support::Fixed).unwrap();
        let released=controller.release_manual(draft.source().id(),draft.source().version(),SubmissionMode::DryRun,&application).unwrap();
        let failure=rusqlite::Connection::open(&path).unwrap();
        failure.execute_batch("CREATE TRIGGER corrupt_after_begin AFTER UPDATE OF phase ON submission_jobs WHEN NEW.phase='sending' BEGIN INSERT INTO submission_jobs(report_id,revision,phase,payload,release_report,release_report_version) VALUES(NEW.report_id,1,'queued','invalid','invalid',1); END;").unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            if controller.tick(&application,None).is_err(){break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(posts.load(Ordering::SeqCst),0);
        let mut observer=tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        assert_eq!(SubmissionJobRepository::load(&mut observer,released.job().id()).unwrap().unwrap().phase(),JobPhase::Sending);
        controller.shutdown().unwrap();
        assert_eq!(SubmissionJobRepository::load(&mut observer,released.job().id()).unwrap().unwrap().phase(),JobPhase::Returned,"A proven local abort must return for renewed review without pretending a request reached UEX");
        assert_eq!(posts.load(Ordering::SeqCst),0);
        drop(controller);
        drop(observer);
        drop(failure);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__fresh_major_stock_review_returns_the_exact_reassessed_report_without_an_attempt()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract",||{
        use tallyline_app::api::{SubmissionQueueController,SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{AcceptanceSwitch,CommodityConstraints,ReferenceParameters,SubmissionAcceptance,CachedReference};
        use tallyline_shared_kernel::api::model::{ObservationAgeSettings,SubmissionMode,Timestamp};
        use tallyline_submission::api::{CredentialApplication,SubmissionJobRepository};
        use tallyline_submission::api::model::{CredentialCandidate,Persistence,JobPhase};
        let path=std::env::temp_dir().join(format!("tallyline-controller-fresh-stock-{}.db",std::process::id()));
        let (mut database,draft)=support::stored_gate(&path).unwrap();
        database.replace(&CachedReference::new(Timestamp::from_seconds(950),EndpointData::Parameters(ReferenceParameters::new(SubmissionAcceptance{datacenter:AcceptanceSwitch::Enabled,reports:AcceptanceSwitch::Enabled,commodity:AcceptanceSwitch::Enabled,ptu:AcceptanceSwitch::Disabled},CommodityConstraints{price_variation_percent:20,prior_ttl_days:std::num::NonZeroU32::new(15).unwrap(),scu_variation_percent:20},None)))).unwrap();
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move ||Ok::<_,()>(Backend(recorded))).unwrap();
        let mut application=CredentialApplication::default();
        let request=application.begin(CredentialCandidate::new(CredentialValues::new("dummy-key".to_owned(),Some("dummy-token".to_owned())).unwrap(),Persistence::Session));
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(application.accept(request,Ok(ValidatedAccount::new("dummy-a".to_owned())),Timestamp::from_seconds(1000),&mut Store)).unwrap();
        let mut controller=SubmissionQueueController::new(database,client,move ||Ok(SubmissionQueuePreferences{reviewer:ManualReviewer::new(EnvironmentMapping::default(),StockSettings::default(),None),age:ObservationAgeSettings::default(),mode:SubmissionMode::DryRun}),None,support::Fixed).unwrap();
        let released=controller.release_manual(draft.source().id(),draft.source().version(),SubmissionMode::DryRun,&application).unwrap();
        use tallyline_reporting::api::ReportRepository;
        use tallyline_reporting::api::model::{Deviation,ReportFinding};
        use tallyline_reference_data::api::model::{PricePrior,PriorValues,PriorProvenance,TerminalPriors};
        use tallyline_shared_kernel::api::model::{CommodityId,TradeSide};
        let mut observer=tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let prior=PricePrior::new(CommodityId(42),TradeSide::Buy,PriorValues{price:"123.45".parse().unwrap(),stock:"100".parse().unwrap(),status:std::num::NonZeroU8::new(3),container_sizes:vec![]},PriorProvenance{game_version:"4.10.2".to_owned(),modified_at:Timestamp::from_seconds(900)}).unwrap();
        observer.replace(&CachedReference::new(Timestamp::from_seconds(1000),EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11),vec![prior]).unwrap()))).unwrap();
        let failure=rusqlite::Connection::open(&path).unwrap();
        failure.execute_batch("CREATE TRIGGER fail_review_return BEFORE UPDATE OF phase ON reports WHEN NEW.phase='draft' BEGIN SELECT RAISE(ABORT,'test review return'); END;").unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            if controller.tick(&application,None).is_err(){break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(controller.jobs()[0].phase(),JobPhase::Queued);
        assert_eq!(SubmissionJobRepository::load(&mut observer,released.job().id()).unwrap().unwrap().phase(),JobPhase::Queued);
        assert!(observer.load_draft(released.report().id()).unwrap().is_none());
        failure.execute_batch("DROP TRIGGER fail_review_return").unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            if controller.tick(&application,None).is_err(){break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        drop(failure);
        assert_eq!(posts.load(Ordering::SeqCst),0);
        assert_eq!(controller.jobs()[0].phase(),JobPhase::Returned,"A queued report with a freshly mandatory decision must become editable through renewed release");
        let restored=observer.load_draft(released.report().id()).unwrap().unwrap();
        assert_eq!(restored.rows()[0].stock_deviation(),Some(Deviation::Major));
        assert!(restored.rows()[0].stock_needs_confirmation());
        assert_eq!(restored.finding(),Some(ReportFinding::ReviewRequired));
        assert_eq!(restored.rows()[0].stock(),Some("0".parse().unwrap()));
        assert_eq!(restored.observed_at(),Timestamp::from_seconds(900));
        assert!(SubmissionJobRepository::load(&mut observer,released.job().id()).unwrap().unwrap().attempts().is_empty());
        controller.shutdown().unwrap();
        drop(controller);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__release_rechecks_the_clock_after_payload_preparation_before_persistence()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        struct AdvancingClock(std::sync::atomic::AtomicUsize);
        impl tallyline_shared_kernel::api::Clock for AdvancingClock {
            fn now(&self) -> tallyline_shared_kernel::api::model::Timestamp {
                let reads = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tallyline_shared_kernel::api::model::Timestamp::from_seconds(if reads < 2 {
                    1000
                } else {
                    5000
                })
            }
        }
        use tallyline_app::api::{SubmissionQueueController, SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{
            AcceptanceSwitch, CachedReference, CommodityConstraints, ReferenceParameters,
            SubmissionAcceptance,
        };
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_shared_kernel::api::model::{
            ObservationAgeSettings, SubmissionMode, Timestamp,
        };
        use tallyline_submission::api::CredentialApplication;
        use tallyline_submission::api::model::{CredentialCandidate, Persistence};
        let path = std::env::temp_dir().join(format!(
            "tallyline-controller-final-release-clock-{}.db",
            std::process::id()
        ));
        let (mut database, draft) = support::stored_gate(&path).unwrap();
        database
            .replace(&CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Parameters(ReferenceParameters::new(
                    SubmissionAcceptance {
                        datacenter: AcceptanceSwitch::Enabled,
                        reports: AcceptanceSwitch::Enabled,
                        commodity: AcceptanceSwitch::Enabled,
                        ptu: AcceptanceSwitch::Disabled,
                    },
                    CommodityConstraints {
                        price_variation_percent: 20,
                        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                        scu_variation_percent: 20,
                    },
                    None,
                )),
            ))
            .unwrap();
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let mut application = CredentialApplication::default();
        let request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
            Persistence::Session,
        ));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(application.accept(
                request,
                Ok(ValidatedAccount::new("dummy-a".to_owned())),
                Timestamp::from_seconds(1000),
                &mut Store,
            ))
            .unwrap();
        let mut controller = SubmissionQueueController::new(
            database,
            client,
            move || {
                Ok(SubmissionQueuePreferences {
                    reviewer: ManualReviewer::new(
                        EnvironmentMapping::default(),
                        StockSettings::default(),
                        None,
                    ),
                    age: ObservationAgeSettings::default(),
                    mode: SubmissionMode::DryRun,
                })
            },
            None,
            AdvancingClock(std::sync::atomic::AtomicUsize::new(0)),
        )
        .unwrap();
        use tallyline_reporting::api::ReportRepository;
        let released = controller.release_manual(
            draft.source().id(),
            draft.source().version(),
            SubmissionMode::DryRun,
            &application,
        );
        assert!(
            released.is_err(),
            "Observation age must be checked again after payload work, before committing a release"
        );
        assert!(controller.jobs().is_empty());
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        assert_eq!(
            observer.load_draft(draft.source().id()).unwrap().as_ref(),
            Some(draft.source())
        );
        controller.shutdown().unwrap();
        drop(controller);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__release_refuses_a_global_mode_change_during_payload_preparation()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract",||{
        use tallyline_app::api::{SubmissionQueueController,SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{AcceptanceSwitch,CommodityConstraints,ReferenceParameters,SubmissionAcceptance,CachedReference};
        use tallyline_shared_kernel::api::model::{ObservationAgeSettings,SubmissionMode,Timestamp};
        use tallyline_submission::api::CredentialApplication;
        use tallyline_submission::api::model::{CredentialCandidate,Persistence};
        let path=std::env::temp_dir().join(format!("tallyline-controller-release-mode-race-{}.db",std::process::id()));
        let (mut database,draft)=support::stored_gate(&path).unwrap();
        database.replace(&CachedReference::new(Timestamp::from_seconds(950),EndpointData::Parameters(ReferenceParameters::new(SubmissionAcceptance{datacenter:AcceptanceSwitch::Enabled,reports:AcceptanceSwitch::Enabled,commodity:AcceptanceSwitch::Enabled,ptu:AcceptanceSwitch::Disabled},CommodityConstraints{price_variation_percent:20,prior_ttl_days:std::num::NonZeroU32::new(15).unwrap(),scu_variation_percent:20},None)))).unwrap();
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move ||Ok::<_,()>(Backend(recorded))).unwrap();
        let mut application=CredentialApplication::default();
        let request=application.begin(CredentialCandidate::new(CredentialValues::new("dummy-key".to_owned(),Some("dummy-token".to_owned())).unwrap(),Persistence::Session));
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(application.accept(request,Ok(ValidatedAccount::new("dummy-a".to_owned())),Timestamp::from_seconds(1000),&mut Store)).unwrap();
        let preference_reads=std::sync::atomic::AtomicUsize::new(0);
        let mut controller=SubmissionQueueController::new(database,client,move ||Ok(SubmissionQueuePreferences{reviewer:ManualReviewer::new(EnvironmentMapping::default(),StockSettings::default(),None),age:ObservationAgeSettings::default(),mode:if preference_reads.fetch_add(1,Ordering::SeqCst)==0{SubmissionMode::DryRun}else{SubmissionMode::Production}}),None,support::Fixed).unwrap();
        let released=controller.release_manual(draft.source().id(),draft.source().version(),SubmissionMode::DryRun,&application);
        assert!(matches!(released,Err(tallyline_app::api::SubmissionControlError::Conflict)),"The release must bind the current chosen mode after preparation, rather than an earlier settings read");
        assert!(controller.jobs().is_empty());
        assert_eq!(posts.load(Ordering::SeqCst),0);
        controller.shutdown().unwrap();
        drop(controller);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}

struct ProductionBackend {
    posts: Arc<AtomicUsize>,
    lookups: Arc<AtomicUsize>,
}
impl HttpBackend for ProductionBackend {
    async fn fetch(&self, _: Endpoint) -> Result<EndpointData, ReadError> {
        Err(ReadError::Transport("recorded unavailable".to_owned()))
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
        job: tallyline_submission::api::model::SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        Some(vec![
            tallyline_submission::api::model::VerifiedUexReport::from_lookup_at(
                123,
                TerminalId(11),
                tallyline_shared_kernel::api::model::CommodityId(42),
                tallyline_shared_kernel::api::model::Timestamp::from_seconds(900),
                job.request(),
                tallyline_shared_kernel::api::model::Timestamp::from_seconds(900),
            )
            .ok()?,
        ])
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        self.posts.fetch_add(1, Ordering::SeqCst);
        let Some(id) = tallyline_shared_kernel::api::model::UexReportId::new(
            123,
            tallyline_shared_kernel::api::model::Timestamp::from_seconds(900),
        ) else {
            return SubmissionResult::Unknown;
        };
        SubmissionResult::ProductionAccepted(vec![id])
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__production_success_commits_before_one_optional_lookup_and_restart_never_replays_it()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract",||{
        use tallyline_app::api::{SubmissionQueueController,SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{AcceptanceSwitch,CommodityConstraints,ReferenceParameters,SubmissionAcceptance,CachedReference};
        use tallyline_shared_kernel::api::model::{ObservationAgeSettings,SubmissionMode,Timestamp};
        use tallyline_submission::api::CredentialApplication;
        use tallyline_submission::api::model::{CredentialCandidate,Persistence,JobPhase};
        let path=std::env::temp_dir().join(format!("tallyline-controller-receipt-{}.db",std::process::id()));
        let (mut database,draft)=support::stored_gate(&path).unwrap();
        database.replace(&CachedReference::new(Timestamp::from_seconds(950),EndpointData::Parameters(ReferenceParameters::new(SubmissionAcceptance{datacenter:AcceptanceSwitch::Enabled,reports:AcceptanceSwitch::Enabled,commodity:AcceptanceSwitch::Enabled,ptu:AcceptanceSwitch::Disabled},CommodityConstraints{price_variation_percent:20,prior_ttl_days:std::num::NonZeroU32::new(15).unwrap(),scu_variation_percent:20},None)))).unwrap();
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let lookups=Arc::new(AtomicUsize::new(0));
        let recorded_lookups=Arc::clone(&lookups);
        let (driver,client)=HttpDriver::start(move ||Ok::<_,()>(ProductionBackend{posts:recorded,lookups:recorded_lookups})).unwrap();
        let reopen_client=client.clone();
        let mut application=CredentialApplication::default();
        let request=application.begin(CredentialCandidate::new(CredentialValues::new("dummy-key".to_owned(),Some("dummy-token".to_owned())).unwrap(),Persistence::Session));
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(application.accept(request,Ok(ValidatedAccount::new("dummy-a".to_owned())),Timestamp::from_seconds(1000),&mut Store)).unwrap();
        let mut controller=SubmissionQueueController::new(database,client,move ||Ok(SubmissionQueuePreferences{reviewer:ManualReviewer::new(EnvironmentMapping::default(),StockSettings::default(),None),age:ObservationAgeSettings::default(),mode:SubmissionMode::Production}),None,support::Fixed).unwrap();
        let released=controller.release_manual(draft.source().id(),draft.source().version(),SubmissionMode::Production,&application).unwrap();
        let failure=rusqlite::Connection::open(&path).unwrap();
        failure.execute_batch("CREATE TRIGGER fail_outcome BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'test outcome save'); END;").unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            if controller.tick(&application,None).is_err(){break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(posts.load(Ordering::SeqCst),1);
        assert_eq!(lookups.load(Ordering::SeqCst),0,"No ID lookup can precede the durable full acceptance transition");
        failure.execute_batch("DROP TRIGGER fail_outcome").unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            controller.tick(&application,None).unwrap();
            if !controller.mappings().is_empty(){break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(controller.jobs()[0].phase(),JobPhase::Succeeded);
        assert_eq!(controller.mappings()[0].receipts()[0].id(),123);
        assert_eq!(controller.mappings()[0].receipts()[0].row(),released.job().request().rows()[0].id());
        assert_eq!(lookups.load(Ordering::SeqCst),1);
        controller.shutdown().unwrap();
        drop(controller);
        let database=tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let mut reopened=SubmissionQueueController::new(database,reopen_client,move ||Ok(SubmissionQueuePreferences{reviewer:ManualReviewer::new(EnvironmentMapping::default(),StockSettings::default(),None),age:ObservationAgeSettings::default(),mode:SubmissionMode::Production}),None,support::Fixed).unwrap();
        assert_eq!(reopened.mappings().len(),1);
        assert_eq!(reopened.mappings()[0].receipts()[0].id(),123);
        reopened.tick(&application,None).unwrap();
        assert_eq!(lookups.load(Ordering::SeqCst),1);
        assert_eq!(posts.load(Ordering::SeqCst),1);
        reopened.shutdown().unwrap();
        drop(reopened);
        drop(failure);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__onboarding_test_forces_dry_mode_and_sending_cancellation_is_refused()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{SubmissionQueueController, SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{
            AcceptanceSwitch, CachedReference, CommodityConstraints, ReferenceParameters,
            SubmissionAcceptance,
        };
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_shared_kernel::api::model::{
            ObservationAgeSettings, SubmissionMode, Timestamp,
        };
        use tallyline_submission::api::model::{CredentialCandidate, JobPhase, Persistence};
        use tallyline_submission::api::{CredentialApplication, SubmissionJobRepository};
        let path = std::env::temp_dir().join(format!(
            "tallyline-controller-test-cancel-{}.db",
            std::process::id()
        ));
        let (mut database, draft) = support::stored_gate(&path).unwrap();
        database
            .replace(&CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Parameters(ReferenceParameters::new(
                    SubmissionAcceptance {
                        datacenter: AcceptanceSwitch::Enabled,
                        reports: AcceptanceSwitch::Enabled,
                        commodity: AcceptanceSwitch::Enabled,
                        ptu: AcceptanceSwitch::Disabled,
                    },
                    CommodityConstraints {
                        price_variation_percent: 20,
                        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                        scu_variation_percent: 20,
                    },
                    None,
                )),
            ))
            .unwrap();
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let mut application = CredentialApplication::default();
        let request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
            Persistence::Session,
        ));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(application.accept(
                request,
                Ok(ValidatedAccount::new("dummy-a".to_owned())),
                Timestamp::from_seconds(1000),
                &mut Store,
            ))
            .unwrap();
        let mut controller = SubmissionQueueController::new(
            database,
            client,
            move || {
                Ok(SubmissionQueuePreferences {
                    reviewer: ManualReviewer::new(
                        EnvironmentMapping::default(),
                        StockSettings::default(),
                        None,
                    ),
                    age: ObservationAgeSettings::default(),
                    mode: SubmissionMode::Production,
                })
            },
            None,
            support::Fixed,
        )
        .unwrap();
        use tallyline_reporting::api::ReportRepository;
        let first = controller
            .release_test(draft.source().id(), draft.source().version(), &application)
            .unwrap();
        assert_eq!(first.job().request().header().mode, SubmissionMode::DryRun);
        let cancelled = controller
            .cancel(first.job().id(), first.job().revision())
            .unwrap();
        assert_eq!(cancelled.phase(), JobPhase::Cancelled);
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let restored = observer.load_draft(draft.source().id()).unwrap().unwrap();
        assert_eq!(restored.rows(), first.report().rows());
        assert_eq!(restored.observed_at(), Timestamp::from_seconds(900));
        let confirmed = controller
            .confirm_observation_age(restored.id(), restored.version())
            .unwrap();
        assert_eq!(
            confirmed.age_confirmation().unwrap().observed_at(),
            Timestamp::from_seconds(900)
        );
        let second = controller
            .release_test(confirmed.id(), confirmed.version(), &application)
            .unwrap();
        assert_eq!(second.job().request().header().mode, SubmissionMode::DryRun);
        assert_eq!(
            second.job().request().header().observed_at,
            Timestamp::from_seconds(900)
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            controller.tick(&application, None).unwrap();
            if controller
                .jobs()
                .iter()
                .any(|job| job.id() == second.job().id() && job.phase() == JobPhase::Sending)
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let sending = SubmissionJobRepository::load(&mut observer, second.job().id())
            .unwrap()
            .unwrap();
        assert_eq!(
            controller.cancel(sending.id(), sending.revision()),
            Err(tallyline_app::api::SubmissionControlError::Refused(
                tallyline_submission::api::model::QueueRefusal::AlreadySending
            ))
        );
        loop {
            controller.tick(&application, None).unwrap();
            if controller
                .jobs()
                .iter()
                .any(|job| job.id() == second.job().id() && job.phase() == JobPhase::Succeeded)
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(
            SubmissionJobRepository::load(&mut observer, second.job().id())
                .unwrap()
                .unwrap()
                .result(),
            Some(&SubmissionResult::DryRunSucceeded)
        );
        controller.shutdown().unwrap();
        drop(controller);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_sub_2_r_sub_8_r_sub_9_r_nf_11__session_bridge_refuses_a_stale_displayed_account_and_publishes_committed_queue_state()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{SubmissionQueueController, SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{
            AcceptanceSwitch, CachedReference, CommodityConstraints, ReferenceParameters,
            SubmissionAcceptance,
        };
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_shared_kernel::api::model::{
            ObservationAgeSettings, SubmissionMode, Timestamp,
        };
        use tallyline_submission::api::CredentialApplication;
        use tallyline_submission::api::model::{CredentialCandidate, JobPhase, Persistence};
        let path = std::env::temp_dir().join(format!(
            "tallyline-controller-session-bridge-{}.db",
            std::process::id()
        ));
        let (mut database, draft) = support::stored_gate(&path).unwrap();
        database
            .replace(&CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Parameters(ReferenceParameters::new(
                    SubmissionAcceptance {
                        datacenter: AcceptanceSwitch::Enabled,
                        reports: AcceptanceSwitch::Enabled,
                        commodity: AcceptanceSwitch::Enabled,
                        ptu: AcceptanceSwitch::Disabled,
                    },
                    CommodityConstraints {
                        price_variation_percent: 20,
                        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                        scu_variation_percent: 20,
                    },
                    None,
                )),
            ))
            .unwrap();
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let mut application = CredentialApplication::default();
        let request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
            Persistence::Session,
        ));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(application.accept(
                request,
                Ok(ValidatedAccount::new("dummy-a".to_owned())),
                Timestamp::from_seconds(1000),
                &mut Store,
            ))
            .unwrap();
        let controller = SubmissionQueueController::new(
            database,
            client,
            move || {
                Ok(SubmissionQueuePreferences {
                    reviewer: ManualReviewer::new(
                        EnvironmentMapping::default(),
                        StockSettings::default(),
                        None,
                    ),
                    age: ObservationAgeSettings::default(),
                    mode: SubmissionMode::Production,
                })
            },
            None,
            support::Fixed,
        )
        .unwrap();

        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_ui::api::model::{
            BackgroundEvent, SubmissionControl, SubmissionFeedback, SubmissionIntent,
        };
        let (commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(1);
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {})
            .with_release_refresh(RecordedOfflineRefresh);
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let _initial = events.try_recv().unwrap();
        commands
            .send(SubmissionControl::Release {
                id: draft.source().id(),
                expected: draft.source().version(),
                account: "dummy-other".to_owned(),
                intent: SubmissionIntent::OnboardingTest,
            })
            .unwrap();
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let BackgroundEvent::SubmissionUpdated(update) = events.try_recv().unwrap() else {
            panic!("queue feedback expected")
        };
        assert_eq!(update.feedback, Some(SubmissionFeedback::Changed));
        assert!(update.jobs.is_empty());
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        commands
            .send(SubmissionControl::Release {
                id: draft.source().id(),
                expected: draft.source().version(),
                account: "dummy-a".to_owned(),
                intent: SubmissionIntent::OnboardingTest,
            })
            .unwrap();
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let BackgroundEvent::SubmissionUpdated(update) = events.try_recv().unwrap() else {
            panic!("committed queue expected")
        };
        assert_eq!(update.feedback, None);
        assert_eq!(update.jobs.len(), 1);
        assert_eq!(update.jobs[0].phase(), JobPhase::Queued);
        assert_eq!(
            update.jobs[0].request().header().mode,
            SubmissionMode::DryRun
        );
        assert!(update.reports.is_empty());
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_val_8_r_sub_9__older_row_confirmation_requires_actual_newest_committed_evidence_and_keeps_cooldown()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{SubmissionQueueController, SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{
            AcceptanceSwitch, CachedReference, CommodityConstraints, ReferenceParameters,
            SubmissionAcceptance,
        };
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_shared_kernel::api::model::{
            ObservationAgeSettings, SubmissionMode, Timestamp,
        };
        use tallyline_submission::api::model::{CredentialCandidate, JobPhase, Persistence};
        use tallyline_submission::api::{CredentialApplication, SubmissionJobRepository};
        let path = std::env::temp_dir().join(format!(
            "tallyline-controller-older-confirmation-{}.db",
            std::process::id()
        ));
        let (mut database, draft) = support::stored_gate(&path).unwrap();
        database
            .replace(&CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Parameters(ReferenceParameters::new(
                    SubmissionAcceptance {
                        datacenter: AcceptanceSwitch::Enabled,
                        reports: AcceptanceSwitch::Enabled,
                        commodity: AcceptanceSwitch::Enabled,
                        ptu: AcceptanceSwitch::Disabled,
                    },
                    CommodityConstraints {
                        price_variation_percent: 20,
                        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                        scu_variation_percent: 20,
                    },
                    None,
                )),
            ))
            .unwrap();
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let mut application = CredentialApplication::default();
        let request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
            Persistence::Session,
        ));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(application.accept(
                request,
                Ok(ValidatedAccount::new("dummy-a".to_owned())),
                Timestamp::from_seconds(1000),
                &mut Store,
            ))
            .unwrap();
        use tallyline_reporting::api::ReportRepository;
        use tallyline_reporting::api::model::{
            NewerObservationEvidence, ReleaseAccountStanding, ReleaseContext,
            ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        };
        use tallyline_shared_kernel::api::model::{CommodityId, UexEnvironment};
        use tallyline_submission::api::model::{PayloadHash, SubmissionCommand};
        use tallyline_workflows::api::ManualReleaseStore;
        let mut newer = database
            .create_manual(
                draft.source().key(),
                Timestamp::from_seconds(950),
                Some("4.10.2"),
            )
            .unwrap();
        let row = newer.add_commodity(CommodityId(42)).unwrap();
        newer
            .set_price(row, Some("123.45".parse().unwrap()))
            .unwrap();
        newer.set_stock(row, Some("0".parse().unwrap())).unwrap();
        newer.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
        database.save_draft(&newer, 0).unwrap();
        let current = tallyline_reference_data::api::ReferenceApplication::load(
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap(),
            support::Fixed,
        )
        .unwrap()
        .snapshot(UexEnvironment::Live);
        let gate = ManualReviewer::new(
            EnvironmentMapping::default(),
            StockSettings::default(),
            None,
        )
        .submission_gate(
            newer,
            &current,
            ReleaseContext::new(
                ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
                SubmissionMode::DryRun,
                ReleaseScreenshotPolicy::Unknown,
                ReleaseWorkingCopy::Absent,
            ),
            ObservationAgeSettings::default(),
            Timestamp::from_seconds(1000),
        );
        let release = database.release_manual(&gate).unwrap();
        let sending = release
            .job()
            .apply(SubmissionCommand::Begin {
                at: Timestamp::from_seconds(1000),
                payload_hash: PayloadHash::new([7; 32]),
                date_added: Timestamp::from_seconds(950),
            })
            .unwrap();
        SubmissionJobRepository::save(&mut database, &sending).unwrap();
        let accepted = sending
            .state()
            .apply(SubmissionCommand::Complete {
                result: SubmissionResult::DryRunSucceeded,
                at: Timestamp::from_seconds(1000),
            })
            .unwrap();
        SubmissionJobRepository::save(&mut database, &accepted).unwrap();
        let mut controller = SubmissionQueueController::new(
            database,
            client,
            move || {
                Ok(SubmissionQueuePreferences {
                    reviewer: ManualReviewer::new(
                        EnvironmentMapping::default(),
                        StockSettings::default(),
                        None,
                    ),
                    age: ObservationAgeSettings::default(),
                    mode: SubmissionMode::Production,
                })
            },
            None,
            support::Fixed,
        )
        .unwrap();

        let row = draft.source().rows()[0].id();
        let evidence = |at| {
            NewerObservationEvidence::new(
                "dummy-a",
                SubmissionMode::DryRun,
                UexEnvironment::Live,
                Timestamp::from_seconds(at),
            )
            .unwrap()
        };
        assert_eq!(
            controller.confirm_newer_observation(
                draft.source().id(),
                draft.source().version(),
                row,
                evidence(951),
                &application
            ),
            Err(tallyline_app::api::SubmissionControlError::Conflict)
        );
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        assert_eq!(
            observer.load_draft(draft.source().id()).unwrap().as_ref(),
            Some(draft.source())
        );
        let confirmed = controller
            .confirm_newer_observation(
                draft.source().id(),
                draft.source().version(),
                row,
                evidence(950),
                &application,
            )
            .unwrap();
        assert_eq!(confirmed.observed_at(), Timestamp::from_seconds(900));
        assert_eq!(confirmed.newer_observation_confirmations().len(), 1);
        assert_eq!(
            confirmed.newer_observation_confirmations()[0].observed_at(),
            Timestamp::from_seconds(950)
        );
        assert_eq!(confirmed.rows(), draft.source().rows());
        let released = controller
            .release_test(confirmed.id(), confirmed.version(), &application)
            .unwrap();
        assert_eq!(
            released
                .job()
                .request()
                .newer_observation_confirmations()
                .len(),
            1
        );
        for _ in 0..3 {
            controller.tick(&application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        assert_eq!(
            controller
                .jobs()
                .iter()
                .find(|job| job.id() == released.job().id())
                .unwrap()
                .phase(),
            JobPhase::Queued
        );
        controller.shutdown().unwrap();
        drop(controller);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_sub_2_r_sub_5_r_sub_9__unavailable_context_holds_the_fixed_job_before_review_and_closed_acceptance_never_starts_an_attempt()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{SubmissionQueueController, SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{
            AcceptanceSwitch, CachedReference, CommodityConstraints, ReferenceParameters,
            SubmissionAcceptance,
        };
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_shared_kernel::api::model::{
            ObservationAgeSettings, SubmissionMode, Timestamp,
        };
        use tallyline_submission::api::CredentialApplication;
        use tallyline_submission::api::model::{CredentialCandidate, JobPhase, Persistence};
        let path = std::env::temp_dir().join(format!(
            "tallyline-controller-fresh-context-hold-{}.db",
            std::process::id()
        ));
        let (mut database, draft) = support::stored_gate(&path).unwrap();
        database
            .replace(&CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Parameters(ReferenceParameters::new(
                    SubmissionAcceptance {
                        datacenter: AcceptanceSwitch::Enabled,
                        reports: AcceptanceSwitch::Enabled,
                        commodity: AcceptanceSwitch::Enabled,
                        ptu: AcceptanceSwitch::Disabled,
                    },
                    CommodityConstraints {
                        price_variation_percent: 20,
                        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                        scu_variation_percent: 20,
                    },
                    None,
                )),
            ))
            .unwrap();
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let mut application = CredentialApplication::default();
        let request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
            Persistence::Session,
        ));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(application.accept(
                request,
                Ok(ValidatedAccount::new("dummy-a".to_owned())),
                Timestamp::from_seconds(1000),
                &mut Store,
            ))
            .unwrap();
        let mut controller = SubmissionQueueController::new(
            database,
            client,
            move || {
                Ok(SubmissionQueuePreferences {
                    reviewer: ManualReviewer::new(
                        EnvironmentMapping::default(),
                        StockSettings::default(),
                        None,
                    ),
                    age: ObservationAgeSettings::default(),
                    mode: SubmissionMode::Production,
                })
            },
            None,
            support::Fixed,
        )
        .unwrap();

        use tallyline_app::api::SubmissionControlError;
        use tallyline_reference_data::api::model::GameVersions;
        use tallyline_reporting::api::ReportRepository;
        use tallyline_submission::api::model::SendHoldReason;
        use tallyline_workflows::api::ManualDispatchError;
        let released = controller
            .release_test(draft.source().id(), draft.source().version(), &application)
            .unwrap();
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        observer
            .replace(&CachedReference::new(
                Timestamp::from_seconds(1),
                EndpointData::GameVersions(GameVersions {
                    live: "4.10.3".to_owned(),
                    ptu: None,
                }),
            ))
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let error = loop {
            if let Err(error) = controller.tick(&application, None) {
                break error;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(
            error,
            SubmissionControlError::Dispatch(ManualDispatchError::Held(
                SendHoldReason::ReferenceUnavailable
            ))
        );
        assert_eq!(controller.jobs()[0].phase(), JobPhase::Queued);
        assert_eq!(
            controller.waiting()[0].reason,
            tallyline_submission::api::model::SubmissionWaitReason::Context(
                SendHoldReason::ReferenceUnavailable
            )
        );
        assert_eq!(controller.waiting()[0].job, released.job().id());
        assert_eq!(controller.waiting()[0].revision, released.job().revision());
        assert_eq!(
            controller.waiting()[0].next_check,
            Some(Timestamp::from_seconds(1001))
        );
        assert!(controller.jobs()[0].attempts().is_empty());
        assert!(observer.load_draft(draft.source().id()).unwrap().is_none());
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        observer
            .replace(&CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::GameVersions(GameVersions {
                    live: "4.10.2".to_owned(),
                    ptu: None,
                }),
            ))
            .unwrap();
        let parameters = |reports| {
            CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Parameters(ReferenceParameters::new(
                    SubmissionAcceptance {
                        datacenter: AcceptanceSwitch::Enabled,
                        reports,
                        commodity: AcceptanceSwitch::Enabled,
                        ptu: AcceptanceSwitch::Disabled,
                    },
                    CommodityConstraints {
                        price_variation_percent: 20,
                        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                        scu_variation_percent: 20,
                    },
                    None,
                )),
            )
        };
        observer
            .replace(&parameters(AcceptanceSwitch::Disabled))
            .unwrap();
        let error = loop {
            if let Err(error) = controller.tick(&application, None) {
                break error;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(
            error,
            SubmissionControlError::Dispatch(ManualDispatchError::Held(
                SendHoldReason::AcceptanceClosed
            ))
        );
        assert!(controller.jobs()[0].attempts().is_empty());
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        observer
            .replace(&parameters(AcceptanceSwitch::Enabled))
            .unwrap();
        loop {
            controller.tick(&application, None).unwrap();
            if controller.jobs()[0].phase() == JobPhase::Succeeded {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(controller.jobs()[0].request(), released.job().request());
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        controller.shutdown().unwrap();
        drop(controller);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_sub_2_r_sub_9_r_ui_16__backpressure_retains_the_explicit_action_result_when_another_job_reports_a_background_hold()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{SubmissionQueueController, SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reference_data::api::ReferenceDataCache;
        use tallyline_reference_data::api::model::{
            AcceptanceSwitch, CachedReference, CommodityConstraints, ReferenceParameters,
            SubmissionAcceptance,
        };
        use tallyline_reporting::api::ManualReviewer;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_shared_kernel::api::model::{
            ObservationAgeSettings, SubmissionMode, Timestamp,
        };
        use tallyline_submission::api::CredentialApplication;
        use tallyline_submission::api::model::{CredentialCandidate, JobPhase, Persistence};
        let path = std::env::temp_dir().join(format!(
            "tallyline-controller-feedback-pressure-{}.db",
            std::process::id()
        ));
        let (mut database, draft) = support::stored_gate(&path).unwrap();
        database
            .replace(&CachedReference::new(
                Timestamp::from_seconds(950),
                EndpointData::Parameters(ReferenceParameters::new(
                    SubmissionAcceptance {
                        datacenter: AcceptanceSwitch::Enabled,
                        reports: AcceptanceSwitch::Enabled,
                        commodity: AcceptanceSwitch::Enabled,
                        ptu: AcceptanceSwitch::Disabled,
                    },
                    CommodityConstraints {
                        price_variation_percent: 20,
                        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
                        scu_variation_percent: 20,
                    },
                    None,
                )),
            ))
            .unwrap();
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let mut application = CredentialApplication::default();
        let request = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
            Persistence::Session,
        ));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(application.accept(
                request,
                Ok(ValidatedAccount::new("dummy-a".to_owned())),
                Timestamp::from_seconds(1000),
                &mut Store,
            ))
            .unwrap();
        let observations = Arc::new(AtomicUsize::new(0));
        let policy_reads = Arc::clone(&observations);
        let mut controller = SubmissionQueueController::new(
            database,
            client,
            move || {
                policy_reads.fetch_add(1, Ordering::SeqCst);
                Ok(SubmissionQueuePreferences {
                    reviewer: ManualReviewer::new(
                        EnvironmentMapping::default(),
                        StockSettings::default(),
                        None,
                    ),
                    age: ObservationAgeSettings::default(),
                    mode: SubmissionMode::Production,
                })
            },
            None,
            support::Fixed,
        )
        .unwrap();

        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_ui::api::model::{
            BackgroundEvent, SubmissionControl, SubmissionFeedback, SubmissionIntent,
        };
        controller
            .release_test(draft.source().id(), draft.source().version(), &application)
            .unwrap();
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        observer
            .replace(&CachedReference::new(
                Timestamp::from_seconds(1),
                EndpointData::GameVersions(tallyline_reference_data::api::model::GameVersions {
                    live: "4.10.3".to_owned(),
                    ptu: None,
                }),
            ))
            .unwrap();
        let baseline = observations.load(Ordering::SeqCst);
        let (commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(1);
        outgoing
            .send(BackgroundEvent::Failure(
                "recorded full view channel".to_owned(),
            ))
            .unwrap();
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {})
            .with_release_refresh(RecordedOfflineRefresh);
        commands
            .send(SubmissionControl::Release {
                id: draft.source().id(),
                expected: draft.source().version(),
                account: "dummy-other".to_owned(),
                intent: SubmissionIntent::OnboardingTest,
            })
            .ok()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            bridge
                .poll(&application, None, Timestamp::from_seconds(1000))
                .unwrap();
            if observations.load(Ordering::SeqCst) > baseline {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(
            events.try_recv().unwrap(),
            BackgroundEvent::Failure(_)
        ));
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let BackgroundEvent::SubmissionUpdated(update) = events.try_recv().unwrap() else {
            panic!("retained action acknowledgement expected")
        };
        assert!(update.completed);
        assert_eq!(
            update.feedback,
            Some(SubmissionFeedback::Changed),
            "an unrelated background hold cannot replace the explicit action result"
        );
        assert_eq!(
            update.background_feedback,
            Some(SubmissionFeedback::Held(
                tallyline_submission::api::model::SendHoldReason::ReferenceUnavailable
            ))
        );
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        assert_eq!(update.jobs.len(), 1);
        assert_eq!(update.jobs[0].phase(), JobPhase::Queued);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_val_7_r_sub_8_r_nf_3__reference_publication_does_not_run_for_initial_state_waits_or_dry_success()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract",||{
        use tallyline_app::api::{CredentialQueue,SubmissionQueueBridge};
        use tallyline_shared_kernel::api::model::{SubmissionMode,Timestamp};
        use tallyline_ui::api::model::{BackgroundEvent,SubmissionControl,SubmissionIntent};
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move ||Ok::<_,()>(Backend(recorded))).unwrap();
        let controller_support::Fixture{controller,application,report,path}=controller_support::fixture(client,SubmissionMode::DryRun,"publication-dry").unwrap();
        let publications=Arc::new(AtomicUsize::new(0));
        let published=Arc::clone(&publications);
        let (commands,incoming)=std::sync::mpsc::sync_channel(1);
        let (outgoing,events)=std::sync::mpsc::sync_channel(4);
        let mut bridge=SubmissionQueueBridge::new(controller,incoming,outgoing,||{}).with_release_refresh(RecordedOfflineRefresh).with_reference_publication(move ||{published.fetch_add(1,Ordering::SeqCst);true});
        bridge.poll(&application,None,Timestamp::from_seconds(1000)).unwrap();
        let _initial=events.try_recv().unwrap();
        assert_eq!(publications.load(Ordering::SeqCst),0,"initial queue publication must not wake the reference owner");
        commands.send(SubmissionControl::Release{id:report.id(),expected:report.version(),account:"dummy-a".to_owned(),intent:SubmissionIntent::OnboardingTest}).ok().unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            bridge.poll(&application,None,Timestamp::from_seconds(1000)).unwrap();
            if events.try_iter().any(|event|matches!(event,BackgroundEvent::SubmissionUpdated(update) if update.jobs.iter().any(|job|job.phase()==tallyline_submission::api::model::JobPhase::Succeeded))){break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(publications.load(Ordering::SeqCst),0);
        assert_eq!(posts.load(Ordering::SeqCst),1);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_sub_9_r_nf_3__reference_publication_follows_durable_production_acceptance_once_and_retains_a_full_signal_channel()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
        use tallyline_ui::api::model::{SubmissionControl, SubmissionIntent};
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let lookups = Arc::new(AtomicUsize::new(0));
        let recorded_lookups = Arc::clone(&lookups);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(ProductionBackend {
                posts: recorded,
                lookups: recorded_lookups,
            })
        })
        .unwrap();
        let controller_support::Fixture {
            controller,
            application,
            report,
            path,
        } = controller_support::fixture(
            client,
            SubmissionMode::Production,
            "publication-production",
        )
        .unwrap();
        let attempts = Arc::new(AtomicUsize::new(0));
        let tried = Arc::clone(&attempts);
        let publications = Arc::new(AtomicUsize::new(0));
        let published = Arc::clone(&publications);
        let evidence = path.clone();
        let (commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(4);
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {})
            .with_release_refresh(RecordedOfflineRefresh)
            .with_reference_publication(move || {
                use tallyline_reference_data::api::AcceptedPriorHistoryStore;
                assert!(
                    tallyline_adapter_platform::api::worker_priority()
                        .unwrap()
                        .is_below_normal()
                );
                assert!(tokio::runtime::Handle::try_current().is_err());
                let mut database =
                    tallyline_adapter_storage::api::SqliteReferenceCache::open(&evidence).unwrap();
                let history = database.accepted_history().unwrap();
                assert_eq!(
                    history.records().len(),
                    1,
                    "only a committed accepted local row can request publication"
                );
                assert_eq!(history.records()[0].prior().values().stock.to_string(), "0");
                if tried.fetch_add(1, Ordering::SeqCst) == 0 {
                    return false;
                }
                published.fetch_add(1, Ordering::SeqCst);
                true
            });
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let _initial = events.try_recv().unwrap();
        commands
            .send(SubmissionControl::Release {
                id: report.id(),
                expected: report.version(),
                account: "dummy-a".to_owned(),
                intent: SubmissionIntent::Manual(SubmissionMode::Production),
            })
            .ok()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            bridge
                .poll(&application, None, Timestamp::from_seconds(1000))
                .unwrap();
            for _ in events.try_iter() {}
            if publications.load(Ordering::SeqCst) == 1 {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for _ in 0..10 {
            bridge
                .poll(&application, None, Timestamp::from_seconds(1000))
                .unwrap();
            for _ in events.try_iter() {}
        }
        assert_eq!(publications.load(Ordering::SeqCst), 1);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

struct SubmissionCancelled(Arc<AtomicUsize>);
impl Drop for SubmissionCancelled {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct HangingBackend {
    base: Backend,
    entered: std::sync::mpsc::SyncSender<()>,
    accounts: Arc<AtomicUsize>,
    cancelled: Arc<AtomicUsize>,
}
impl HttpBackend for HangingBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        self.base.fetch(endpoint).await
    }
    async fn fetch_terminal_priors(
        &self,
        terminals: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        self.base.fetch_terminal_priors(terminals).await
    }
    async fn validate(
        &self,
        values: CredentialValues,
    ) -> Result<ValidatedAccount, CredentialFailure> {
        self.base.validate(values).await
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
        values: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        self.base.0.fetch_add(1, Ordering::SeqCst);
        let account = match values.secret_key() {
            "dummy-key" => 1,
            "dummy-key-b" => 2,
            _ => 4,
        };
        self.accounts.fetch_or(account, Ordering::SeqCst);
        let guard = SubmissionCancelled(Arc::clone(&self.cancelled));
        let _sent = self.entered.try_send(());
        let result = std::future::pending().await;
        drop(guard);
        result
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_8_r_sub_9_r_sub_12__account_switch_and_shutdown_keep_unknown_closed_and_require_renewed_release_for_unsent_work()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{SubmissionQueueController, SubmissionQueuePreferences};
        use tallyline_game::api::model::EnvironmentMapping;
        use tallyline_reference_data::api::AcceptedPriorHistoryStore;
        use tallyline_reporting::api::model::StockSettings;
        use tallyline_reporting::api::{ManualReviewer, ReportRepository};
        use tallyline_shared_kernel::api::model::{
            CommodityId, ObservationAgeSettings, SubmissionMode, Timestamp,
        };
        use tallyline_submission::api::SubmissionJobRepository;
        use tallyline_submission::api::model::{
            CredentialCandidate, JobPhase, Persistence, SubmissionWaitReason,
        };
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let accounts = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&accounts);
        let cancelled = Arc::new(AtomicUsize::new(0));
        let cancellation = Arc::clone(&cancelled);
        let (entered, started) = std::sync::mpsc::sync_channel(1);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(HangingBackend {
                base: Backend(recorded),
                entered,
                accounts: observed,
                cancelled: cancellation,
            })
        })
        .unwrap();
        let controller_support::Fixture {
            mut controller,
            mut application,
            report,
            path,
        } = controller_support::fixture(client, SubmissionMode::Production, "account-shutdown")
            .unwrap();
        let first = controller
            .release_manual(
                report.id(),
                report.version(),
                SubmissionMode::Production,
                &application,
            )
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            controller.tick(&application, None).unwrap();
            if started.try_recv().is_ok() {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let in_flight = SubmissionJobRepository::load(&mut observer, first.job().id())
            .unwrap()
            .unwrap();
        assert_eq!(in_flight.phase(), JobPhase::Sending);
        assert_eq!(in_flight.attempts().len(), 1);
        let candidate = application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key-b".to_owned(), Some("dummy-token".to_owned()))
                .unwrap(),
            Persistence::Session,
        ));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(application.accept(
                candidate,
                Ok(ValidatedAccount::new("dummy-b".to_owned())),
                Timestamp::from_seconds(1000),
                &mut Store,
            ))
            .unwrap();
        let mut pending = observer
            .create_manual(report.key(), report.observed_at(), report.game_version())
            .unwrap();
        let row = pending.add_commodity(CommodityId(42)).unwrap();
        pending
            .set_price(row, Some("123.45".parse().unwrap()))
            .unwrap();
        pending.set_stock(row, Some("0".parse().unwrap())).unwrap();
        pending
            .set_status(row, std::num::NonZeroU8::new(3))
            .unwrap();
        observer.save_draft(&pending, 0).unwrap();
        let second = controller
            .release_test(pending.id(), pending.version(), &application)
            .unwrap();
        controller.tick(&application, None).unwrap();
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(accounts.load(Ordering::SeqCst), 1);
        assert_eq!(
            controller
                .waiting()
                .iter()
                .find(|wait| wait.job == second.job().id())
                .unwrap()
                .reason,
            SubmissionWaitReason::InFlight
        );
        assert_eq!(second.job().request().header().account, "dummy-b");
        assert_eq!(second.job().request().header().mode, SubmissionMode::DryRun);
        controller.shutdown().unwrap();
        let unknown = SubmissionJobRepository::load(&mut observer, first.job().id())
            .unwrap()
            .unwrap();
        assert_eq!(unknown.phase(), JobPhase::OutcomeUnknown);
        assert_eq!(unknown.request(), first.job().request());
        assert_eq!(unknown.result(), Some(&SubmissionResult::Unknown));
        assert!(observer.accepted_history().unwrap().records().is_empty());
        assert_eq!(
            SubmissionJobRepository::load(&mut observer, second.job().id())
                .unwrap()
                .unwrap()
                .phase(),
            JobPhase::Queued
        );
        drop(controller);
        driver.finish().unwrap();
        assert_eq!(cancelled.load(Ordering::SeqCst), 1);
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let store = tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let mut recovered = SubmissionQueueController::new(
            store,
            client,
            || {
                Ok(SubmissionQueuePreferences {
                    reviewer: ManualReviewer::new(
                        EnvironmentMapping::default(),
                        StockSettings::default(),
                        None,
                    ),
                    age: ObservationAgeSettings::default(),
                    mode: SubmissionMode::Production,
                })
            },
            None,
            support::Fixed,
        )
        .unwrap();
        for _ in 0..3 {
            recovered.tick(&application, None).unwrap();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        assert_eq!(
            recovered
                .jobs()
                .iter()
                .find(|job| job.id() == first.job().id())
                .unwrap()
                .phase(),
            JobPhase::OutcomeUnknown
        );
        assert_eq!(
            recovered
                .jobs()
                .iter()
                .find(|job| job.id() == second.job().id())
                .unwrap()
                .phase(),
            JobPhase::Returned
        );
        let renewed = observer.load_draft(pending.id()).unwrap().unwrap();
        assert_eq!(renewed.observed_at(), pending.observed_at());
        assert_eq!(renewed.rows(), second.report().rows());
        let explicit = recovered
            .release_test(renewed.id(), renewed.version(), &application)
            .unwrap();
        assert!(explicit.job().id() > second.job().id());
        assert_eq!(explicit.job().request().header().account, "dummy-b");
        assert_eq!(
            explicit.job().request().header().observed_at,
            Timestamp::from_seconds(900)
        );
        assert_eq!(
            explicit.job().request().rows(),
            second.job().request().rows()
        );
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        recovered.shutdown().unwrap();
        drop(recovered);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

struct RecordedReleaseRefresh {
    complete: Arc<std::sync::atomic::AtomicBool>,
    attempts: Arc<std::sync::Mutex<Vec<TerminalId>>>,
}
impl tallyline_app::api::ReleaseReferenceRefresh for RecordedReleaseRefresh {
    fn begin(&mut self, terminal: TerminalId) -> std::io::Result<()> {
        self.attempts
            .lock()
            .map_err(|_| std::io::Error::other("recorded attempt unavailable"))?
            .push(terminal);
        Ok(())
    }
    fn completed(&mut self) -> std::io::Result<bool> {
        Ok(self.complete.load(Ordering::SeqCst))
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_sub_2__manual_release_waits_for_an_actual_reference_attempt_before_saving_or_posting()
{
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
        use tallyline_ui::api::model::{BackgroundEvent, SubmissionControl, SubmissionIntent};
        let posts=Arc::new(AtomicUsize::new(0));
        let recorded=Arc::clone(&posts);
        let (driver,client)=HttpDriver::start(move || Ok::<_,()>(Backend(recorded))).unwrap();
        let controller_support::Fixture{controller,application,report,path}=controller_support::fixture(client,SubmissionMode::DryRun,"release-refresh").unwrap();
        let complete=Arc::new(std::sync::atomic::AtomicBool::new(false));
        let attempts=Arc::new(std::sync::Mutex::new(Vec::new()));
        let refresh=RecordedReleaseRefresh{complete:Arc::clone(&complete),attempts:Arc::clone(&attempts)};
        let (commands,incoming)=std::sync::mpsc::sync_channel(1);
        let (outgoing,events)=std::sync::mpsc::sync_channel(16);
        let mut bridge=SubmissionQueueBridge::new(controller,incoming,outgoing,||{}).with_release_refresh(refresh);
        commands.send(SubmissionControl::Release{id:report.id(),expected:report.version(),account:"dummy-a".to_owned(),intent:SubmissionIntent::OnboardingTest}).unwrap();
        for _ in 0..3 {
            bridge.poll(&application,None,Timestamp::from_seconds(1000)).unwrap();
            for event in events.try_iter() {
                if let BackgroundEvent::SubmissionUpdated(update)=event {
                    assert!(update.jobs.is_empty());
                    assert!(!update.completed);
                    assert_eq!(update.reports[0].version(),report.version());
                }
            }
        }
        assert_eq!(*attempts.lock().unwrap(),vec![report.key().terminal()]);
        assert_eq!(posts.load(Ordering::SeqCst),0);
        complete.store(true,Ordering::SeqCst);
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        loop {
            bridge.poll(&application,None,Timestamp::from_seconds(1000)).unwrap();
            if events.try_iter().any(|event| matches!(event,BackgroundEvent::SubmissionUpdated(update) if update.jobs.iter().any(|job|job.phase()==tallyline_submission::api::model::JobPhase::Succeeded))) {break;}
            assert!(std::time::Instant::now()<deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(posts.load(Ordering::SeqCst),1);
        assert_eq!(attempts.lock().unwrap().len(),1);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}

struct RecordedOfflineRefresh;
impl tallyline_app::api::ReleaseReferenceRefresh for RecordedOfflineRefresh {
    fn begin(&mut self, _: TerminalId) -> std::io::Result<()> {
        Ok(())
    }
    fn completed(&mut self) -> std::io::Result<bool> {
        Ok(true)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_4_r_val_7_r_sub_2__starting_a_candidate_during_release_refresh_closes_sending_and_preserves_the_draft()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
        use tallyline_submission::api::model::{CredentialCandidate, Persistence};
        use tallyline_ui::api::model::{
            BackgroundEvent, SubmissionControl, SubmissionFeedback, SubmissionIntent,
        };
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let controller_support::Fixture {
            controller,
            mut application,
            report,
            path,
        } = controller_support::fixture(
            client,
            SubmissionMode::DryRun,
            "release-refresh-candidate",
        )
        .unwrap();
        let complete = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let refresh = RecordedReleaseRefresh {
            complete: Arc::clone(&complete),
            attempts: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let (commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(4);
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {})
            .with_release_refresh(refresh);
        commands
            .send(SubmissionControl::Release {
                id: report.id(),
                expected: report.version(),
                account: "dummy-a".to_owned(),
                intent: SubmissionIntent::OnboardingTest,
            })
            .unwrap();
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let _initial = events.try_recv().unwrap();
        let _candidate = application.begin(CredentialCandidate::new(
            CredentialValues::new(
                "dummy-replacement".to_owned(),
                Some("dummy-token".to_owned()),
            )
            .unwrap(),
            Persistence::Session,
        ));
        complete.store(true, Ordering::SeqCst);
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let BackgroundEvent::SubmissionUpdated(update) = events.try_recv().unwrap() else {
            panic!("release acknowledgement expected")
        };
        assert!(update.completed);
        assert_eq!(update.feedback, Some(SubmissionFeedback::Changed));
        assert!(update.jobs.is_empty());
        assert_eq!(update.reports[0], report);
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_val_7_r_sub_2__missing_reference_owner_refuses_release_without_a_job_or_post() {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
        use tallyline_ui::api::model::{
            BackgroundEvent, SubmissionControl, SubmissionFeedback, SubmissionIntent,
        };
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let controller_support::Fixture {
            controller,
            application,
            report,
            path,
        } = controller_support::fixture(client, SubmissionMode::DryRun, "release-refresh-missing")
            .unwrap();
        let (commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(4);
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {});
        commands
            .send(SubmissionControl::Release {
                id: report.id(),
                expected: report.version(),
                account: "dummy-a".to_owned(),
                intent: SubmissionIntent::OnboardingTest,
            })
            .unwrap();
        bridge
            .poll(&application, None, Timestamp::from_seconds(1000))
            .unwrap();
        let BackgroundEvent::SubmissionUpdated(update) = events.try_recv().unwrap() else {
            panic!("release acknowledgement expected")
        };
        assert!(update.completed);
        assert_eq!(update.feedback, Some(SubmissionFeedback::Unavailable));
        assert!(update.jobs.is_empty());
        assert_eq!(update.reports[0], report);
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
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
fn r_sub_2_r_sub_9_r_nf_11__startup_notifications_wait_for_both_projection_channels_before_durable_acknowledgement()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
        use tallyline_submission::api::model::JobPhase;
        use tallyline_ui::api::model::BackgroundEvent;
        use tallyline_workflows::api::EventOutbox;
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(ProductionBackend {
            posts: recorded, lookups: Arc::new(AtomicUsize::new(0)),
        })).unwrap();
        let controller_support::Fixture { mut controller, application, report, path } =
            controller_support::fixture(client, SubmissionMode::Production, "outbox-startup").unwrap();
        controller.release_manual(report.id(), report.version(), SubmissionMode::Production, &application).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while controller.jobs()[0].phase() != JobPhase::Succeeded {
            controller.tick(&application, None).unwrap();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let mut observer = tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        let original = observer.pending_events().unwrap();
        assert!(original.len() >= 2);
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let allowed = Arc::clone(&ready);
        let publications = Arc::new(AtomicUsize::new(0));
        let published = Arc::clone(&publications);
        let (_commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(1);
        outgoing.send(BackgroundEvent::StorageWarning("recorded occupied channel".to_owned())).unwrap();
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {})
            .with_reference_publication(move || {
                if !allowed.load(Ordering::SeqCst) { return false; }
                published.fetch_add(1, Ordering::SeqCst);
                true
            });
        bridge.poll(&application, None, Timestamp::from_seconds(1000)).unwrap();
        assert_eq!(observer.pending_events().unwrap(), original);
        drop(events.try_recv().unwrap());
        bridge.poll(&application, None, Timestamp::from_seconds(1000)).unwrap();
        assert!(matches!(events.try_recv().unwrap(), BackgroundEvent::SubmissionUpdated(_)));
        assert_eq!(observer.pending_events().unwrap(), original);
        ready.store(true, Ordering::SeqCst);
        bridge.poll(&application, None, Timestamp::from_seconds(1000)).unwrap();
        assert!(observer.pending_events().unwrap().is_empty(), "startup notifications must be acknowledged after both handlers accept the committed projection");
        for _ in 0..3 {
            bridge.poll(&application, None, Timestamp::from_seconds(1000)).unwrap();
            for _ in events.try_iter() {}
        }
        assert_eq!(publications.load(Ordering::SeqCst), 1);
        assert_eq!(posts.load(Ordering::SeqCst), 1);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    }).unwrap().finish().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9_r_nf_11__startup_drains_multiple_notification_batches_without_new_queue_progress()
 {
    tallyline_app::api::CpuWorker::start("tallyline-queue-contract", || {
        use tallyline_app::api::{CredentialQueue, SubmissionQueueBridge};
        use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
        use tallyline_workflows::api::{EventOutbox, ManualReleaseStore, OUTBOX_BATCH_LIMIT};
        let path = std::env::temp_dir().join(format!(
            "tallyline-controller-outbox-batches-{}.db",
            std::process::id()
        ));
        for _ in 0..=OUTBOX_BATCH_LIMIT {
            let (mut store, gate) = support::stored_gate(&path).unwrap();
            store.release_manual(&gate).unwrap();
        }
        let posts = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
        let controller_support::Fixture {
            controller,
            application,
            path,
            ..
        } = controller_support::fixture(client, SubmissionMode::DryRun, "outbox-batches").unwrap();
        let mut observer =
            tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
        assert_eq!(observer.pending_events().unwrap().len(), OUTBOX_BATCH_LIMIT);
        let (_commands, incoming) = std::sync::mpsc::sync_channel(1);
        let (outgoing, events) = std::sync::mpsc::sync_channel(1);
        let mut bridge = SubmissionQueueBridge::new(controller, incoming, outgoing, || {});
        for _ in 0..10 {
            bridge
                .poll(&application, None, Timestamp::from_seconds(1000))
                .unwrap();
            for _ in events.try_iter() {}
        }
        assert!(
            observer.pending_events().unwrap().is_empty(),
            "idle control steps must drain every bounded startup batch"
        );
        assert_eq!(posts.load(Ordering::SeqCst), 0);
        bridge.shutdown(Timestamp::from_seconds(1000)).unwrap();
        drop(bridge);
        drop(observer);
        driver.finish().unwrap();
        std::fs::remove_file(path).unwrap();
    })
    .unwrap()
    .finish()
    .unwrap();
}

#[path = "submission_controller/offline_release.rs"]
mod offline_release;

#[path = "submission_controller/recovery.rs"]
mod recovery;

#[path = "submission_controller/global_holds.rs"]
mod global_holds;

#[path = "submission_controller/version_lockout.rs"]
mod version_lockout;

#[path = "submission_controller/fixable_returns.rs"]
mod fixable_returns;

#[path = "submission_controller/account_receipt.rs"]
mod account_receipt;

#[path = "submission_controller/blocked_read.rs"]
mod blocked_read;

#[path = "submission_controller/repeated_account.rs"]
mod repeated_account;

#[path = "submission_controller/permanent_refusal.rs"]
mod permanent_refusal;

#[path = "submission_controller/unknown_resolution.rs"]
mod unknown_resolution;

#[path = "submission_controller/receipt_support.rs"]
mod receipt_support;

#[path = "submission_controller/partial_resolution.rs"]
mod partial_resolution;

#[path = "submission_controller/quota.rs"]
mod quota;

#[path = "submission_controller/quota_regressions.rs"]
mod quota_regressions;

#[path = "submission_controller/quota_tokens.rs"]
mod quota_tokens;

#[path = "submission_controller/quota_overlap.rs"]
mod quota_overlap;

#[path = "submission_controller/quota_transfer.rs"]
mod quota_transfer;

#[path = "submission_controller/quota_pending_post.rs"]
mod quota_pending_post;

#[path = "submission_controller/cooldown_split.rs"]
mod cooldown_split;
#[path = "../../workflows/tests/manual_release/three_rows.rs"]
mod three_rows;

#[path = "submission_controller/rejected_copy.rs"]
mod rejected_copy;

#[path = "submission_controller/lifecycle_reads.rs"]
mod lifecycle_reads;
