// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Continuous public admission while concurrent HTTP commands transfer quota evidence.

use super::controller_support;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex, mpsc};
use std::time::Duration;
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::model::HttpAdmissionFailure;
use tallyline_app::api::{CpuWorker, HttpBackend, HttpDriver, SubmissionControlError};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp, UexReportId};
use tallyline_submission::api::CredentialValidator;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, JobId, JobPhase, PayloadHash, SubmissionJob,
    SubmissionResult, TokenQuotaObservation, ValidatedAccount, VerifiedUexReport,
};

struct TransferBackend {
    observations: Mutex<Vec<TokenQuotaObservation>>,
    known: mpsc::SyncSender<()>,
    resume: Arc<tokio::sync::Notify>,
    draining: mpsc::SyncSender<()>,
    release: Arc<Barrier>,
    transferred: AtomicBool,
}
impl HttpBackend for TransferBackend {
    async fn fetch(&self, _: Endpoint) -> Result<EndpointData, ReadError> {
        Err(ReadError::Transport(
            "unused public fixture route".to_owned(),
        ))
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
        _: SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        None
    }
    async fn verify_row_receipts(
        &self,
        _: SubmissionJob,
        values: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<VerifiedUexReport>> {
        self.observations
            .lock()
            .ok()?
            .push(TokenQuotaObservation::new(values.token_scope(), None));
        self.known.send(()).ok()?;
        self.resume.notified().await;
        None
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        SubmissionResult::Unknown
    }
    fn take_quota_observations(&self) -> Vec<TokenQuotaObservation> {
        let observations = match self.observations.lock() {
            Ok(mut observations) => std::mem::take(&mut *observations),
            Err(_) => vec![TokenQuotaObservation::new(None, None)],
        };
        if !observations.is_empty()
            && !self.transferred.swap(true, Ordering::SeqCst)
            && self.draining.send(()).is_ok()
        {
            self.release.wait();
        }
        observations
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_api_3__concurrent_quota_handoff_never_opens_a_gap_in_authenticated_admission() {
    CpuWorker::start("tallyline-quota-transfer", transfer_case)
        .unwrap()
        .finish()
        .unwrap()
        .unwrap();
}
fn transfer_case() -> Result<(), String> {
    let (known_tx, known_rx) = mpsc::sync_channel(1);
    let (draining_tx, draining_rx) = mpsc::sync_channel(1);
    let resume = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(Barrier::new(2));
    let backend = TransferBackend {
        observations: Mutex::new(vec![]),
        known: known_tx,
        resume: Arc::clone(&resume),
        draining: draining_tx,
        release: Arc::clone(&release),
        transferred: AtomicBool::new(false),
    };
    let (driver, client) =
        HttpDriver::start(move || Ok::<_, ()>(backend)).map_err(|error| format!("{error:?}"))?;
    let mut fixture = controller_support::fixture(
        client.clone(),
        SubmissionMode::Production,
        "quota-atomic-transfer",
    )
    .map_err(|error| format!("{error:?}"))?;
    fixture
        .controller
        .release_manual(
            fixture.report.id(),
            fixture.report.version(),
            SubmissionMode::Production,
            &fixture.application,
        )
        .map_err(|error| format!("{error:?}"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while fixture
        .controller
        .jobs()
        .first()
        .ok_or("missing fixture job")?
        .phase()
        != JobPhase::OutcomeUnknown
    {
        fixture
            .controller
            .tick(&fixture.application, None)
            .map_err(|error| format!("{error:?}"))?;
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    let original = fixture
        .controller
        .jobs()
        .first()
        .ok_or("missing fixture job")?
        .clone();
    let accepted = SubmissionJob::queued(JobId(999), original.request().clone())
        .map_err(|error| format!("{error:?}"))?
        .begin_prepared(
            Timestamp::from_seconds(1000),
            PayloadHash::new([7; 32]),
            Timestamp::from_seconds(900),
        )
        .map_err(|error| format!("{error:?}"))?
        .complete(
            SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(81, Timestamp::from_seconds(900))
                    .ok_or("invalid fixture receipt identity")?,
            ]),
            Timestamp::from_seconds(1001),
        )
        .map_err(|error| format!("{error:?}"))?;
    fixture
        .controller
        .check_receipts(
            original.id(),
            original.revision(),
            &fixture.application,
            None,
        )
        .map_err(|error| format!("{error:?}"))?;
    known_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| format!("{error:?}"))?;
    let values = fixture
        .application
        .credentials_for("dummy-a")
        .ok_or("missing fixture account")?
        .clone();
    let validating_client = client.clone();
    let validating_values = values.clone();
    let validating = CpuWorker::start(
        "tallyline-quota-concurrent",
        move || -> Result<_, String> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("{error:?}"))?;
            Ok(runtime.block_on(validating_client.validate(&validating_values)))
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    draining_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| format!("{error:?}"))?;
    let (probing_tx, probing_rx) = mpsc::sync_channel(1);
    let (admission_tx, admission_rx) = mpsc::sync_channel(1);
    let probe = CpuWorker::start(
        "tallyline-quota-admission",
        move || -> Result<(), String> {
            probing_tx.send(()).map_err(|error| format!("{error:?}"))?;
            let result = client
                .begin_receipt_verification(&accepted, &values, None)
                .map(|_| ());
            admission_tx
                .send(result)
                .map_err(|error| format!("{error:?}"))?;
            Ok(())
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    probing_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| format!("{error:?}"))?;
    let early = admission_rx.recv_timeout(Duration::from_millis(500)).ok();
    release.wait();
    let admission = match early {
        Some(admission) => admission,
        None => admission_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| format!("{error:?}"))?,
    };
    probe.finish().map_err(|error| format!("{error:?}"))??;
    assert!(
        validating
            .finish()
            .map_err(|error| format!("{error:?}"))??
            .is_ok()
    );
    resume.notify_one();
    while fixture
        .controller
        .authenticated_quotas()
        .map_err(|error| format!("{error:?}"))?
        .is_empty()
    {
        let result = fixture.controller.tick(&fixture.application, None);
        assert!(matches!(
            result,
            Ok(_) | Err(SubmissionControlError::Unavailable)
        ));
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(
        fixture
            .controller
            .jobs()
            .first()
            .ok_or("missing fixture job")?,
        &original
    );
    fixture
        .controller
        .shutdown()
        .map_err(|error| format!("{error:?}"))?;
    drop(fixture.controller);
    driver.finish().map_err(|error| format!("{error:?}"))?;
    std::fs::remove_file(fixture.path).map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        admission,
        Err(HttpAdmissionFailure::Busy),
        "a known drained quota cannot admit an authenticated receipt read before outer publication"
    );
    Ok(())
}
