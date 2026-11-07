// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! First-ever explicit token validation and durable/session dispatch boundaries.

use super::quota::QuotaBackend;
use super::*;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::model::JobPhase;

struct FirstTokenStore {
    registered_b: bool,
    lost: bool,
}
impl tallyline_submission::api::SecretStore for FirstTokenStore {
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
        persistence: tallyline_submission::api::model::Persistence,
    ) -> Result<
        Option<tallyline_submission::api::model::TokenScope>,
        tallyline_submission::api::model::StorageFailure,
    > {
        let mut bytes = [1; 32];
        if token == "dummy-first-b" {
            if !self.registered_b
                && persistence == tallyline_submission::api::model::Persistence::Session
            {
                return Ok(None);
            }
            self.registered_b = true;
            bytes.get_mut(16..).ok_or(StorageFailure::Corrupt)?.fill(2);
        }
        Ok(tallyline_submission::api::model::TokenScope::new(bytes))
    }
    async fn token_admission(
        &mut self,
        token: &str,
    ) -> Result<
        tallyline_submission::api::model::TokenAdmission,
        tallyline_submission::api::model::StorageFailure,
    > {
        use tallyline_submission::api::model::{Persistence, TokenAdmission, TokenScope};
        if self.lost {
            return Ok(TokenAdmission::default());
        }
        if token == "dummy-first-b" && !self.registered_b {
            return TokenAdmission::distinct_from(vec![
                TokenScope::new([1; 32]).ok_or(StorageFailure::Corrupt)?,
            ])
            .ok_or(StorageFailure::Corrupt);
        }
        Ok(TokenAdmission::from_scope(
            self.token_scope(token, Persistence::Session).await?,
        ))
    }
}
fn first_token(persistence: tallyline_submission::api::model::Persistence) -> Result<(), String> {
    use tallyline_submission::api::CredentialApplication;
    use tallyline_submission::api::model::{CredentialCandidate, Persistence};
    tallyline_app::api::CpuWorker::start("tallyline-first-token-b", move || {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(QuotaBackend(backend_posts)))
            .map_err(|error| format!("{error:?}"))?;
        let mut fixture = controller_support::fixture(
            client.clone(),
            SubmissionMode::DryRun,
            match persistence {
                Persistence::Durable => "quota-first-token-b",
                Persistence::Session => "quota-first-session-b",
            },
        )
        .map_err(|error| format!("{error:?}"))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("{error:?}"))?;
        let mut store = FirstTokenStore {
            registered_b: false,
            lost: false,
        };
        fixture.application = CredentialApplication::default();
        let mut a = fixture.application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-a".to_owned()))
                .map_err(|error| format!("{error:?}"))?,
            Persistence::Durable,
        ));
        runtime
            .block_on(a.lookup_token_admission(&mut store, None))
            .map_err(|error| format!("{error:?}"))?;
        let validated = runtime.block_on(a.validate(&client));
        runtime
            .block_on(fixture.application.accept(
                a,
                validated,
                Timestamp::from_seconds(1000),
                &mut store,
            ))
            .map_err(|error| format!("{error:?}"))?;
        runtime
            .block_on(fixture.application.establish_token_scope(
                &mut store,
                None,
                Persistence::Durable,
            ))
            .map_err(|error| format!("{error:?}"))?;
        fixture
            .controller
            .release_manual(
                fixture.report.id(),
                fixture.report.version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .map_err(|error| format!("{error:?}"))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while fixture
            .controller
            .jobs()
            .first()
            .ok_or("missing job")?
            .phase()
            != JobPhase::Held
        {
            fixture
                .controller
                .tick(&fixture.application, None)
                .map_err(|error| format!("{error:?}"))?;
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let fence = *fixture
            .controller
            .service_fences()
            .map_err(|error| format!("{error:?}"))?
            .first()
            .ok_or("missing fence")?;
        let source = fixture
            .controller
            .jobs()
            .first()
            .ok_or("missing job")?
            .clone();
        let mut same = fixture.application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-a".to_owned()))
                .map_err(|error| format!("{error:?}"))?,
            Persistence::Durable,
        ));
        runtime
            .block_on(same.lookup_token_admission(&mut store, None))
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(
            runtime.block_on(same.validate(&client)),
            Err(CredentialFailure::Unavailable)
        );
        let mut b = fixture.application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-first-b".to_owned()))
                .map_err(|error| format!("{error:?}"))?,
            persistence,
        ));
        runtime
            .block_on(b.lookup_token_admission(&mut store, None))
            .map_err(|error| format!("{error:?}"))?;
        let validated = runtime.block_on(b.validate(&client));
        assert!(validated.is_ok());
        assert!(!store.registered_b);
        runtime
            .block_on(fixture.application.accept(
                b,
                validated,
                Timestamp::from_seconds(1000),
                &mut store,
            ))
            .map_err(|error| format!("{error:?}"))?;
        runtime
            .block_on(
                fixture
                    .application
                    .establish_token_scope(&mut store, None, persistence),
            )
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(store.registered_b, persistence == Persistence::Durable);
        fixture
            .controller
            .cancel(source.id(), source.revision())
            .map_err(|error| format!("{error:?}"))?;
        let (database, next) =
            super::support::stored_gate(&fixture.path).map_err(|error| format!("{error:?}"))?;
        drop(database);
        let released = fixture
            .controller
            .release_manual(
                next.source().id(),
                next.source().version(),
                SubmissionMode::DryRun,
                &fixture.application,
            )
            .map_err(|error| format!("{error:?}"))?;
        while !fixture
            .controller
            .jobs()
            .iter()
            .any(|job| job.id() == released.job().id() && job.phase() == JobPhase::Succeeded)
        {
            fixture
                .controller
                .tick(&fixture.application, None)
                .map_err(|error| format!("{error:?}"))?;
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(posts.load(Ordering::SeqCst), 2);
        assert_eq!(
            *fixture
                .controller
                .service_fences()
                .map_err(|error| format!("{error:?}"))?
                .first()
                .ok_or("missing fence")?,
            fence
        );
        store.lost = true;
        let mut lost = fixture.application.begin(CredentialCandidate::new(
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-first-b".to_owned()))
                .map_err(|error| format!("{error:?}"))?,
            Persistence::Durable,
        ));
        runtime
            .block_on(lost.lookup_token_admission(&mut store, None))
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(
            runtime.block_on(lost.validate(&client)),
            Err(CredentialFailure::Unavailable)
        );
        fixture
            .controller
            .shutdown()
            .map_err(|error| format!("{error:?}"))?;
        drop(fixture.controller);
        driver.finish().map_err(|error| format!("{error:?}"))?;
        std::fs::remove_file(fixture.path).map_err(|error| format!("{error:?}"))?;
        Ok::<(), String>(())
    })
    .map_err(|error| format!("{error:?}"))?
    .finish()
    .map_err(|error| format!("{error:?}"))??;
    Ok(())
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__first_ever_token_b_validates_and_dispatches_while_token_a_and_lost_identity_stay_held()
 {
    first_token(tallyline_submission::api::model::Persistence::Durable).unwrap();
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__first_ever_session_token_b_dispatches_without_persisting_its_association()
 {
    first_token(tallyline_submission::api::model::Persistence::Session).unwrap();
}
