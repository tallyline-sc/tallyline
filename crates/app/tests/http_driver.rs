// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned shared HTTP runtime contracts without remote services or credential providers.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{HttpBackend, HttpDriver};
use tallyline_reference_data::api::ReferenceDataFetcher;
use tallyline_reference_data::api::model::{Endpoint, EndpointData, GameVersions, TerminalPriors};
use tallyline_shared_kernel::api::model::TerminalId;
use tallyline_submission::api::CredentialValidator;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SubmissionResult, ValidatedAccount,
};

struct Backend {
    workers: Arc<AtomicUsize>,
}
impl HttpBackend for Backend {
    async fn fetch(&self, _: Endpoint) -> Result<EndpointData, ReadError> {
        self.workers.store(
            tokio::runtime::Handle::current().metrics().num_workers(),
            Ordering::SeqCst,
        );
        Ok(EndpointData::GameVersions(GameVersions {
            live: "4.10.2".to_owned(),
            ptu: None,
        }))
    }
    async fn fetch_terminal_priors(
        &self,
        _: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Ok(Vec::new())
    }
    async fn validate(&self, _: CredentialValues) -> Result<ValidatedAccount, CredentialFailure> {
        let workers = Arc::clone(&self.workers);
        tokio::task::spawn(async move {
            workers.store(
                tokio::runtime::Handle::current().metrics().num_workers(),
                Ordering::SeqCst,
            );
        })
        .await
        .map_err(|_| CredentialFailure::Unavailable)?;
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
        SubmissionResult::DryRunSucceeded
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_api_7__shared_http_driver_owns_exactly_two_io_threads_beyond_caller_runtime_lifetimes()
{
    let workers = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&workers);
    let (driver, client) =
        HttpDriver::start(move || Ok::<_, ()>(Backend { workers: observed })).unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    {
        let caller = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert_eq!(
            caller.block_on(client.validate(&values)).unwrap(),
            ValidatedAccount::new("dummy-a".to_owned())
        );
    }
    assert_eq!(workers.load(Ordering::SeqCst), 2);
    workers.store(0, Ordering::SeqCst);
    let caller = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert!(matches!(
        caller
            .block_on(client.fetch(Endpoint::GameVersions))
            .unwrap(),
        EndpointData::GameVersions(_)
    ));
    assert_eq!(workers.load(Ordering::SeqCst), 2);
    drop(client);
    drop(caller);
    driver.finish().unwrap();
}

use std::sync::mpsc;
use std::time::Duration;
use tallyline_app::api::HttpClient;
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::SubmissionGateway;
use tallyline_submission::api::model::{SubmissionHeader, SubmissionRequest, SubmissionRow};

struct BlockingBackend {
    base: Backend,
    entered: mpsc::SyncSender<()>,
    release: Arc<tokio::sync::Notify>,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
}
impl HttpBackend for BlockingBackend {
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
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        if self.entered.try_send(()).is_err() {
            self.active.fetch_sub(1, Ordering::SeqCst);
            return SubmissionResult::Unknown;
        }
        self.release.notified().await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        SubmissionResult::DryRunSucceeded
    }
}
fn request() -> Option<SubmissionRequest> {
    SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(1),
            report_version: 3,
            account: "dummy-a".to_owned(),
            mode: SubmissionMode::DryRun,
            terminal: TerminalId(89),
            side: TradeSide::Buy,
            environment: UexEnvironment::Live,
            game_version: "4.10.2".to_owned(),
            faction_affinity: None,
            observed_at: Timestamp::from_seconds(100),
        },
        vec![
            SubmissionRow::new(
                RowId(1),
                CommodityId(7),
                "2".parse().ok()?,
                "5".parse().ok()?,
                2,
            )
            .ok()?,
        ],
    )
    .ok()
}
fn caller(
    client: HttpClient,
) -> std::io::Result<std::thread::JoinHandle<std::io::Result<SubmissionResult>>> {
    std::thread::Builder::new().spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let request = request().ok_or_else(|| std::io::Error::other("invalid request fixture"))?;
        let (prepared, _) = client
            .prepare(&request)
            .map_err(|_| std::io::Error::other("request preparation failed"))?;
        let values = CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))
            .map_err(|_| std::io::Error::other("invalid credential fixture"))?;
        Ok(runtime.block_on(client.submit(prepared, &values, None)))
    })
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2__shared_driver_never_runs_two_submissions_in_flight_even_with_independent_callers() {
    let (entered, events) = mpsc::sync_channel(2);
    let release = Arc::new(tokio::sync::Notify::new());
    let signal = Arc::clone(&release);
    let maximum = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&maximum);
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(BlockingBackend {
            base: Backend {
                workers: Arc::new(AtomicUsize::new(0)),
            },
            entered,
            release: signal,
            active: Arc::new(AtomicUsize::new(0)),
            maximum: observed,
        })
    })
    .unwrap();
    let first = caller(client.clone()).unwrap();
    let second = caller(client).unwrap();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    release.notify_one();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    release.notify_one();
    assert_eq!(
        first.join().unwrap().unwrap(),
        SubmissionResult::DryRunSucceeded
    );
    assert_eq!(
        second.join().unwrap().unwrap(),
        SubmissionResult::DryRunSucceeded
    );
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
    driver.finish().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__stopping_the_owned_driver_closes_inflight_submission_as_unknown_without_replay() {
    let (entered, events) = mpsc::sync_channel(2);
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(BlockingBackend {
            base: Backend {
                workers: Arc::new(AtomicUsize::new(0)),
            },
            entered,
            release: Arc::new(tokio::sync::Notify::new()),
            active: Arc::new(AtomicUsize::new(0)),
            maximum: Arc::new(AtomicUsize::new(0)),
        })
    })
    .unwrap();
    let sending = caller(client).unwrap();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    driver.finish().unwrap();
    assert_eq!(sending.join().unwrap().unwrap(), SubmissionResult::Unknown);
    assert!(matches!(
        events.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

struct ReceiptBackend {
    base: Backend,
    entered: mpsc::SyncSender<()>,
    submitted: mpsc::SyncSender<()>,
    release: Arc<tokio::sync::Notify>,
    receipts: Option<Vec<tallyline_submission::api::model::VerifiedUexReport>>,
}
impl HttpBackend for ReceiptBackend {
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
    async fn submit(
        &self,
        prepared: PreparedUexSubmission,
        values: CredentialValues,
        token: Option<String>,
    ) -> SubmissionResult {
        if self.submitted.try_send(()).is_err() {
            return SubmissionResult::Unknown;
        }
        self.base.submit(prepared, values, token).await
    }
    async fn verify_receipts(
        &self,
        _: tallyline_submission::api::model::SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        self.entered.try_send(()).ok()?;
        self.release.notified().await;
        self.receipts.clone()
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9__receipt_reads_do_not_hold_the_submission_permit_or_change_known_success() {
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::SubmissionReceiptVerifier;
    use tallyline_submission::api::model::{JobId, PayloadHash, SubmissionJob};
    let (entered, events) = mpsc::sync_channel(1);
    let (submitted, submissions) = mpsc::sync_channel(1);
    let release = Arc::new(tokio::sync::Notify::new());
    let signal = Arc::clone(&release);
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(ReceiptBackend {
            base: Backend {
                workers: Arc::new(AtomicUsize::new(0)),
            },
            entered,
            submitted,
            release: signal,
            receipts: None,
        })
    })
    .unwrap();
    let base = request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let accepted = SubmissionJob::queued(
        JobId(9),
        SubmissionRequest::new(header, base.rows().to_vec()).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
    .unwrap()
    .complete(
        SubmissionResult::ProductionAccepted(vec![
            UexReportId::new(42, Timestamp::from_seconds(100)).unwrap(),
        ]),
        Timestamp::from_seconds(115),
    )
    .unwrap();
    let retained = accepted.clone();
    let reader = client.clone();
    let verification = std::thread::Builder::new()
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let values =
                CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))
                    .unwrap();
            runtime.block_on(reader.verify_receipts(&accepted, &values, None))
        })
        .unwrap();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    let next = caller(client).unwrap();
    let dispatched_before_lookup_finished =
        submissions.recv_timeout(Duration::from_secs(2)).is_ok();
    release.notify_one();
    assert!(verification.join().unwrap().is_none());
    assert_eq!(
        next.join().unwrap().unwrap(),
        SubmissionResult::DryRunSucceeded
    );
    driver.finish().unwrap();
    assert!(dispatched_before_lookup_finished);
    assert_eq!(
        retained.recover(Timestamp::from_seconds(120)).unwrap(),
        retained
    );
    assert_eq!(retained.attempts().len(), 1);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9__optional_receipt_reply_never_blocks_account_work_and_closes_without_replay() {
    use tallyline_app::api::model::ReceiptLookupOutcome;
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::model::{JobId, PayloadHash, SubmissionJob};
    let (entered, events) = mpsc::sync_channel(1);
    let (submitted, _submissions) = mpsc::sync_channel(1);
    let release = Arc::new(tokio::sync::Notify::new());
    let signal = Arc::clone(&release);
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(ReceiptBackend {
            base: Backend {
                workers: Arc::new(AtomicUsize::new(0)),
            },
            entered,
            submitted,
            release: signal,
            receipts: None,
        })
    })
    .unwrap();
    let base = request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let accepted = SubmissionJob::queued(
        JobId(10),
        SubmissionRequest::new(header, base.rows().to_vec()).unwrap(),
    )
    .unwrap()
    .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
    .unwrap()
    .complete(
        SubmissionResult::ProductionAccepted(vec![
            UexReportId::new(42, Timestamp::from_seconds(100)).unwrap(),
        ]),
        Timestamp::from_seconds(115),
    )
    .unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let queued = SubmissionJob::queued(JobId(12), accepted.request().clone()).unwrap();
    assert!(matches!(
        client.begin_receipt_verification(&queued, &values, None),
        Err(tallyline_app::api::model::HttpAdmissionFailure::InvalidJob)
    ));
    let mut pending = client
        .begin_receipt_verification(&accepted, &values, None)
        .unwrap();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(pending.poll_result().is_none());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert_eq!(
        runtime.block_on(client.validate(&values)).unwrap(),
        ValidatedAccount::new("dummy-a".to_owned())
    );
    driver.finish().unwrap();
    assert!(matches!(
        pending.poll_result(),
        Some(ReceiptLookupOutcome::Unavailable)
    ));
    assert!(pending.poll_result().is_none());
    assert_eq!(
        accepted.recover(Timestamp::from_seconds(120)).unwrap(),
        accepted
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__optional_receipt_completion_rejects_an_identity_outside_the_accepted_response() {
    use tallyline_app::api::model::ReceiptLookupOutcome;
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::model::{JobId, PayloadHash, SubmissionJob, VerifiedUexReport};
    for id in [42, 43] {
        let base = request().unwrap();
        let mut header = base.header().clone();
        header.mode = SubmissionMode::Production;
        let accepted = SubmissionJob::queued(
            JobId(11),
            SubmissionRequest::new(header, base.rows().to_vec()).unwrap(),
        )
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(100)).unwrap(),
            ]),
            Timestamp::from_seconds(115),
        )
        .unwrap();
        let receipt = VerifiedUexReport::from_lookup(
            id,
            TerminalId(89),
            CommodityId(7),
            Timestamp::from_seconds(100),
            accepted.request(),
        )
        .unwrap();
        let (entered, events) = mpsc::sync_channel(1);
        let (submitted, _submissions) = mpsc::sync_channel(1);
        let release = Arc::new(tokio::sync::Notify::new());
        let signal = Arc::clone(&release);
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(ReceiptBackend {
                base: Backend {
                    workers: Arc::new(AtomicUsize::new(0)),
                },
                entered,
                submitted,
                release: signal,
                receipts: Some(vec![receipt]),
            })
        })
        .unwrap();
        let values =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        let mut pending = client
            .begin_receipt_verification(&accepted, &values, None)
            .unwrap();
        events.recv_timeout(Duration::from_secs(10)).unwrap();
        release.notify_one();
        let mut outcome = None;
        for _ in 0..100 {
            outcome = pending.poll_result();
            if outcome.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        match outcome {
            Some(ReceiptLookupOutcome::Verified(mapping)) => {
                assert_eq!(id, 42);
                assert_eq!(mapping.receipts()[0].id(), 42);
                assert!(mapping.matches_job(&accepted));
            }
            Some(ReceiptLookupOutcome::Unavailable) => assert_eq!(id, 43),
            None => panic!("bounded local receipt fixture did not complete"),
        }
        assert!(pending.poll_result().is_none());
        assert_eq!(accepted.attempts().len(), 1);
        driver.finish().unwrap();
    }
}

struct BlockingPoolBackend {
    base: Backend,
    entered: mpsc::SyncSender<()>,
    gate: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
}
impl HttpBackend for BlockingPoolBackend {
    async fn fetch(&self, endpoint: Endpoint) -> Result<EndpointData, ReadError> {
        let mut tasks = Vec::new();
        for _ in 0..3 {
            let entered = self.entered.clone();
            let gate = Arc::clone(&self.gate);
            let active = Arc::clone(&self.active);
            let maximum = Arc::clone(&self.maximum);
            tasks.push(tokio::task::spawn_blocking(move || {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(count, Ordering::SeqCst);
                let result = (|| {
                    entered.try_send(()).map_err(|_| ())?;
                    let stopped = gate.0.lock().map_err(|_| ())?;
                    let _finished = gate
                        .1
                        .wait_while(stopped, |stopped| !*stopped)
                        .map_err(|_| ())?;
                    Ok::<_, ()>(())
                })();
                active.fetch_sub(1, Ordering::SeqCst);
                result
            }));
        }
        for task in tasks {
            task.await
                .map_err(|_| ReadError::Transport("fake blocking task stopped".to_owned()))?
                .map_err(|()| ReadError::Transport("fake blocking task failed".to_owned()))?;
        }
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
    async fn submit(
        &self,
        prepared: PreparedUexSubmission,
        values: CredentialValues,
        token: Option<String>,
    ) -> SubmissionResult {
        self.base.submit(prepared, values, token).await
    }
    async fn verify_receipts(
        &self,
        job: tallyline_submission::api::model::SubmissionJob,
        values: CredentialValues,
        token: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        self.base.verify_receipts(job, values, token).await
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_3_r_sub_2__shared_runtime_bounds_existing_blocking_resolver_work_and_joins_every_task() {
    let (entered, events) = mpsc::sync_channel(3);
    let gate = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let control = Arc::clone(&gate);
    let active = Arc::new(AtomicUsize::new(0));
    let observed_active = Arc::clone(&active);
    let maximum = Arc::new(AtomicUsize::new(0));
    let observed_maximum = Arc::clone(&maximum);
    let (driver, client) = HttpDriver::start(move || {
        Ok::<_, ()>(BlockingPoolBackend {
            base: Backend {
                workers: Arc::new(AtomicUsize::new(0)),
            },
            entered,
            gate: control,
            active: observed_active,
            maximum: observed_maximum,
        })
    })
    .unwrap();
    let call = std::thread::Builder::new()
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(client.fetch(Endpoint::GameVersions))
        })
        .unwrap();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    events.recv_timeout(Duration::from_secs(10)).unwrap();
    let third_started_before_capacity = events.recv_timeout(Duration::from_secs(2)).is_ok();
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    call.join().unwrap().unwrap();
    driver.finish().unwrap();
    assert!(!third_started_before_capacity);
    assert_eq!(maximum.load(Ordering::SeqCst), 2);
    assert_eq!(active.load(Ordering::SeqCst), 0);
}
