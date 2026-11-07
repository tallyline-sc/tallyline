// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sole HTTP submission slot is reserved before write-ahead preparation.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{HttpBackend, HttpDriver};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SubmissionHeader, SubmissionRequest, SubmissionResult,
    SubmissionRow, ValidatedAccount,
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
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9__reserved_post_slot_waits_for_committed_preparation_and_drop_never_dispatches() {
    let posts = Arc::new(AtomicUsize::new(0));
    let recorded = Arc::clone(&posts);
    let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(Backend(recorded))).unwrap();
    let mut reservation = client.begin_submission_reservation().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let slot = loop {
        if let Some(slot) = reservation.poll_slot().unwrap() {
            break slot;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(
        posts.load(Ordering::SeqCst),
        0,
        "An owned ready POST slot is not an HTTP request"
    );
    drop(slot);
    let mut reservation = client.begin_submission_reservation().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let slot = loop {
        if let Some(slot) = reservation.poll_slot().unwrap() {
            break slot;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(posts.load(Ordering::SeqCst), 0);
    let request = SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(1),
            report_version: 3,
            account: "dummy-a".to_owned(),
            mode: SubmissionMode::DryRun,
            terminal: TerminalId(11),
            side: TradeSide::Buy,
            environment: UexEnvironment::Live,
            game_version: "4.10.2".to_owned(),
            faction_affinity: None,
            observed_at: Timestamp::from_seconds(100),
        },
        vec![
            SubmissionRow::new(
                RowId(1),
                CommodityId(42),
                "1".parse().unwrap(),
                "0".parse().unwrap(),
                3,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let prepared = PreparedUexSubmission::new_at(&request, Timestamp::from_seconds(100))
        .unwrap()
        .0;
    let mut pending = slot.dispatch(
        prepared,
        &CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        None,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let result = loop {
        if let Some(result) = pending.poll_result() {
            break result;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(result, SubmissionResult::DryRunSucceeded);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    assert_eq!(pending.poll_result(), None);
    let mut held = client.begin_submission_reservation().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let slot = loop {
        if let Some(slot) = held.poll_slot().unwrap() {
            break slot;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let mut stopped = client.begin_submission_reservation().unwrap();
    driver.finish().unwrap();
    assert!(stopped.poll_slot().is_err());
    let prepared = PreparedUexSubmission::new_at(&request, Timestamp::from_seconds(100))
        .unwrap()
        .0;
    let mut aborted = slot.dispatch(
        prepared,
        &CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap(),
        None,
    );
    assert_eq!(
        aborted.poll_result(),
        Some(SubmissionResult::Returned(
            tallyline_submission::api::model::SubmissionReturnReason::PreflightChanged
        )),
        "A closed payload channel proves that HTTP dispatch never started"
    );
    assert_eq!(posts.load(Ordering::SeqCst), 1);
}
