// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fresh full send checks and write-ahead persistence at the real workflow boundary.

mod support;

use tallyline_reference_data::api::model::{CachedReference, ReferenceSnapshot, UexEnvironment};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_submission::api::model::{SavedCredentials, StorageFailure};
use tallyline_submission::api::{CredentialApplication, SecretStore};

struct SnapshotCache(Vec<CachedReference>);
impl ReferenceDataCache for SnapshotCache {
    type Error = std::convert::Infallible;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}
#[derive(Default)]
struct Store(Option<SavedCredentials>);
impl SecretStore for Store {
    async fn load(&mut self) -> Result<Option<SavedCredentials>, StorageFailure> {
        Ok(self.0.clone())
    }
    async fn save(&mut self, value: &SavedCredentials) -> Result<(), StorageFailure> {
        self.0 = Some(value.clone());
        Ok(())
    }
    async fn save_passive(&mut self, value: &SavedCredentials) -> Result<(), StorageFailure> {
        self.save(value).await
    }
    async fn remove(&mut self) -> Result<(), StorageFailure> {
        self.0 = None;
        Ok(())
    }
}
fn snapshot(entries: Vec<CachedReference>) -> ReferenceSnapshot {
    ReferenceApplication::load(SnapshotCache(entries), support::Fixed)
        .unwrap()
        .snapshot(UexEnvironment::Live)
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_5_r_sub_8__write_ahead_requires_fresh_session_authority_and_rechecks_exact_reference_history()
 {
    use tallyline_game::api::model::EnvironmentMapping;
    use tallyline_reference_data::api::model::{
        AcceptanceSwitch, CommodityConstraints, EndpointData, ReferenceParameters,
        SubmissionAcceptance,
    };
    use tallyline_reporting::api::ManualReviewer;
    use tallyline_reporting::api::model::{
        ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        StockSettings,
    };
    use tallyline_shared_kernel::api::model::{ObservationAgeSettings, SubmissionMode, Timestamp};
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        CredentialCandidate, CredentialValues, JobPhase, PayloadHash, Persistence, SendHoldReason,
        ValidatedAccount,
    };
    use tallyline_workflows::api::{
        ManualDispatch, ManualDispatchError, ManualDispatchStore, ManualReleaseStore,
        SubmissionQueueStore, assess_release_observation_history,
    };
    let path = std::env::temp_dir().join(format!(
        "tallyline-manual-dispatch-{}.db",
        std::process::id()
    ));
    let (mut database, draft) = support::stored_gate(&path).unwrap();
    let parameters = CachedReference::new(
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
    );
    database.replace(&parameters).unwrap();
    let references = snapshot(ReferenceDataCache::load(&mut database).unwrap());
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let context = ReleaseContext::new(
        ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
        SubmissionMode::DryRun,
        ReleaseScreenshotPolicy::Unknown,
        ReleaseWorkingCopy::Absent,
    );
    let release_gate = reviewer.submission_gate(
        draft.source().clone(),
        &references,
        context.clone(),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let released = database.release_manual(&release_gate).unwrap();
    let send_gate = reviewer.submission_gate(
        released.report().clone(),
        &references,
        context,
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let send_gate =
        assess_release_observation_history(&send_gate, Some(&database.queue_jobs().unwrap()));
    let mut application = CredentialApplication::default();
    let mut store = Store::default();
    let candidate = CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        Persistence::Durable,
    );
    let request = application.begin(candidate);
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-a".to_owned())),
            Timestamp::from_seconds(999),
            &mut store,
        )
        .await
        .unwrap();
    let mut restored = CredentialApplication::load(&mut store).await.unwrap();
    assert!(
        matches!(
            ManualDispatch::prepare(
                released.job(),
                &send_gate,
                &restored,
                PayloadHash::new([7; 32]),
                Timestamp::from_seconds(900)
            ),
            Err(ManualDispatchError::Held(
                SendHoldReason::AccountUnavailable
            ))
        ),
        "Remembered accepted identity does not authorise a write in this session"
    );
    let request = restored.begin_revalidation().unwrap();
    restored
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-a".to_owned())),
            Timestamp::from_seconds(1000),
            &mut store,
        )
        .await
        .unwrap();
    let plan = ManualDispatch::prepare(
        released.job(),
        &send_gate,
        &restored,
        PayloadHash::new([7; 32]),
        Timestamp::from_seconds(900),
    )
    .unwrap();
    let mut refreshed = parameters.clone();
    refreshed = CachedReference::new(Timestamp::from_seconds(1000), refreshed.data().clone());
    database.replace(&refreshed).unwrap();
    assert!(matches!(
        database.begin_manual_dispatch(&plan, Timestamp::from_seconds(1000)),
        Err(ManualDispatchError::Conflict)
    ));
    assert_eq!(
        SubmissionJobRepository::load(&mut database, released.job().id())
            .unwrap()
            .unwrap()
            .phase(),
        JobPhase::Queued
    );
    let references = snapshot(ReferenceDataCache::load(&mut database).unwrap());
    let send_gate = reviewer.submission_gate(
        released.report().clone(),
        &references,
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::DryRun,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let send_gate =
        assess_release_observation_history(&send_gate, Some(&database.queue_jobs().unwrap()));
    let plan = ManualDispatch::prepare(
        released.job(),
        &send_gate,
        &restored,
        PayloadHash::new([7; 32]),
        Timestamp::from_seconds(900),
    )
    .unwrap();
    let sending = database
        .begin_manual_dispatch(&plan, Timestamp::from_seconds(1000))
        .unwrap();
    assert_eq!(sending.phase(), JobPhase::Sending);
    assert_eq!(sending.attempts().len(), 1);
    assert_eq!(sending.request(), released.job().request());
    assert_eq!(
        sending.attempts()[0].payload_hash(),
        PayloadHash::new([7; 32])
    );
    assert!(matches!(
        database.begin_manual_dispatch(&plan, Timestamp::from_seconds(1000)),
        Err(ManualDispatchError::Conflict)
    ));
    drop(database);
    std::fs::remove_file(path).unwrap();
}
