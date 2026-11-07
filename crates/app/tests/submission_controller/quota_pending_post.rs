// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! POST admission while another public HTTP command retains a known quota answer.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{CpuWorker, HttpBackend, HttpClient, HttpDriver};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    CredentialCandidate, CredentialFailure, CredentialValues, Persistence, SavedCredentials,
    StorageFailure, SubmissionHeader, SubmissionJob, SubmissionRequest, SubmissionResult,
    SubmissionReturnReason, SubmissionRow, TokenAdmission, TokenQuotaObservation,
    TokenQuotaObservations, TokenScope, ValidatedAccount, VerifiedUexReport,
};
use tallyline_submission::api::{
    CredentialApplication, CredentialValidator, SecretStore, SubmissionGateway,
};
use tokio::sync::{Notify, oneshot};

struct PendingBackend {
    observations: Mutex<TokenQuotaObservations>,
    known: Mutex<Option<oneshot::Sender<()>>>,
    resume: Arc<Notify>,
    posts: Arc<AtomicUsize>,
    scope: Option<TokenScope>,
}
impl HttpBackend for PendingBackend {
    async fn fetch(&self, _: Endpoint) -> Result<EndpointData, ReadError> {
        Err(ReadError::Transport("unused fixture route".to_owned()))
    }
    async fn fetch_terminal_priors(
        &self,
        _: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Ok(vec![])
    }
    async fn validate(&self, _: CredentialValues) -> Result<ValidatedAccount, CredentialFailure> {
        self.observations
            .lock()
            .map_err(|_| CredentialFailure::Unavailable)?
            .record(TokenQuotaObservation::new(self.scope, None));
        self.known
            .lock()
            .map_err(|_| CredentialFailure::Unavailable)?
            .take()
            .ok_or(CredentialFailure::Unavailable)?
            .send(())
            .map_err(|()| CredentialFailure::Unavailable)?;
        self.resume.notified().await;
        Err(CredentialFailure::Unavailable)
    }
    async fn verify_receipts(
        &self,
        _: SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        None
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        self.posts.fetch_add(1, Ordering::SeqCst);
        SubmissionResult::DryRunSucceeded
    }
    fn take_quota_observations(&self) -> Vec<TokenQuotaObservation> {
        self.observations.lock().map_or_else(
            |_| vec![TokenQuotaObservation::new(None, None)],
            |mut observations| observations.take(),
        )
    }
}
struct ScopeStore;
impl SecretStore for ScopeStore {
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
    async fn token_admission(&mut self, token: &str) -> Result<TokenAdmission, StorageFailure> {
        let a = TokenScope::new([1; 32]).ok_or(StorageFailure::Corrupt)?;
        Ok(match token {
            "dummy-a" => TokenAdmission::from_scope(Some(a)),
            "dummy-b" => TokenAdmission::distinct_from(vec![a]).ok_or(StorageFailure::Corrupt)?,
            _ => TokenAdmission::default(),
        })
    }
}
async fn values(token: &str) -> Result<CredentialValues, String> {
    let mut application = CredentialApplication::default();
    let mut request = application.begin(CredentialCandidate::new(
        CredentialValues::new("dummy-key".to_owned(), Some(token.to_owned()))
            .map_err(|error| format!("{error:?}"))?,
        Persistence::Session,
    ));
    request
        .lookup_token_admission(&mut ScopeStore, None)
        .await
        .map_err(|error| format!("{error:?}"))?;
    application
        .accept(
            request,
            Ok(ValidatedAccount::new("dummy-account".to_owned())),
            Timestamp::from_seconds(1000),
            &mut ScopeStore,
        )
        .await
        .map_err(|error| format!("{error:?}"))?;
    application
        .credentials_for("dummy-account")
        .cloned()
        .ok_or_else(|| "missing fixture account".to_owned())
}
fn request() -> Result<SubmissionRequest, String> {
    SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(1),
            report_version: 1,
            account: "dummy-account".to_owned(),
            mode: SubmissionMode::DryRun,
            terminal: TerminalId(89),
            side: TradeSide::Sell,
            environment: UexEnvironment::Live,
            game_version: "4.10.2".to_owned(),
            faction_affinity: None,
            observed_at: Timestamp::from_seconds(100),
        },
        vec![
            SubmissionRow::new(
                RowId(1),
                CommodityId(7),
                "120".parse().map_err(|_| "invalid fixture price")?,
                "0".parse().map_err(|_| "invalid fixture stock")?,
                1,
            )
            .map_err(|error| format!("{error:?}"))?,
        ],
    )
    .map_err(|error| format!("{error:?}"))
}
#[derive(Clone, Copy)]
enum Route {
    Reserved,
    Ordinary,
}
async fn dispatch(
    client: &HttpClient,
    values: &CredentialValues,
    route: Route,
) -> Result<SubmissionResult, String> {
    let prepared = client
        .prepare(&request()?)
        .map_err(|error| format!("{error:?}"))?
        .0;
    match route {
        Route::Ordinary => Ok(client.submit(prepared, values, None).await),
        Route::Reserved => {
            let mut reservation = client
                .begin_submission_reservation()
                .map_err(|error| format!("{error:?}"))?;
            let slot = loop {
                if let Some(slot) = reservation
                    .poll_slot()
                    .map_err(|error| format!("{error:?}"))?
                {
                    break slot;
                }
                tokio::task::yield_now().await;
            };
            let mut pending = slot.dispatch(prepared, values, None);
            loop {
                if let Some(result) = pending.poll_result() {
                    break Ok(result);
                }
                tokio::task::yield_now().await;
            }
        }
    }
}
async fn pending_case(route: Route, scope: Option<TokenScope>) -> Result<(), String> {
    let (known_tx, known_rx) = oneshot::channel();
    let resume = Arc::new(Notify::new());
    let posts = Arc::new(AtomicUsize::new(0));
    let backend = PendingBackend {
        observations: Default::default(),
        known: Mutex::new(Some(known_tx)),
        resume: Arc::clone(&resume),
        posts: Arc::clone(&posts),
        scope,
    };
    let (driver, client) =
        HttpDriver::start(move || Ok::<_, ()>(backend)).map_err(|error| format!("{error:?}"))?;
    let read_client = client.clone();
    let a = values("dummy-a").await?;
    let b = values("dummy-b").await?;
    let unknown = values("dummy-unknown").await?;
    let read_values = a.clone();
    let read = tokio::spawn(async move { read_client.validate(&read_values).await });
    known_rx.await.map_err(|error| format!("{error:?}"))?;
    let same = dispatch(&client, &a, route).await?;
    let distinct = dispatch(&client, &b, route).await?;
    let unproven = dispatch(&client, &unknown, route).await?;
    let dispatched = posts.load(Ordering::SeqCst);
    resume.notify_one();
    assert_eq!(
        read.await.map_err(|error| format!("{error:?}"))?,
        Err(CredentialFailure::Unavailable)
    );
    driver.finish().map_err(|error| format!("{error:?}"))?;
    let refused = SubmissionResult::Returned(SubmissionReturnReason::PreflightChanged);
    assert_eq!(
        same, refused,
        "known pending GET quota must close same-token POST before its command returns"
    );
    assert_eq!(
        distinct,
        if scope.is_some() {
            SubmissionResult::DryRunSucceeded
        } else {
            refused.clone()
        }
    );
    assert_eq!(unproven, refused);
    assert_eq!(dispatched, usize::from(scope.is_some()));
    Ok(())
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_11_r_api_3__pending_get_quota_closes_reserved_and_ordinary_posts_before_read_return()
 {
    CpuWorker::start("tallyline-pending-post", || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            for route in [Route::Reserved, Route::Ordinary] {
                for scope in [TokenScope::new([1; 32]), None] {
                    tokio::time::timeout(Duration::from_secs(10), pending_case(route, scope))
                        .await
                        .unwrap()
                        .unwrap();
                }
            }
        });
    })
    .unwrap()
    .finish()
    .unwrap();
}
