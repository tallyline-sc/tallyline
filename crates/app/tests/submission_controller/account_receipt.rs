// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Independent account causes through the public queue owner and real SQLite.

use super::controller_support;
use super::recovery::RecoveryBackend;
use super::support;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_app::api::{CpuWorker, HttpDriver, SubmissionControlError};
use tallyline_reference_data::api::ReferenceDataCache;
use tallyline_reference_data::api::model::{
    CachedReference, EndpointData, Terminal, TerminalPriors, TerminalText,
};
use tallyline_reporting::api::ReportRepository;
use tallyline_reporting::api::model::{Report, ReportKey};
use tallyline_shared_kernel::api::model::{SubmissionMode, TerminalId, Timestamp};
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    JobId, JobPhase, PayloadHash, SubmissionCommand, SubmissionJob, SubmissionPauseReason,
    SubmissionResult, SubmissionServiceCause, UexErrorCode,
};

type TestResult<T = ()> = std::io::Result<T>;
fn error(value: impl std::fmt::Debug) -> std::io::Error {
    std::io::Error::other(format!("{value:?}"))
}
fn code(value: &str) -> TestResult<UexErrorCode> {
    UexErrorCode::new(value.to_owned()).ok_or_else(|| error("invalid fixture code"))
}
struct Clock(Arc<AtomicU64>);
impl tallyline_shared_kernel::api::Clock for Clock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(self.0.load(Ordering::SeqCst))
    }
}
struct Scenario {
    fixture: controller_support::Fixture,
    driver: HttpDriver,
    posts: Arc<AtomicUsize>,
}
impl Scenario {
    fn new(name: &str, outcome: SubmissionResult, code: Option<UexErrorCode>) -> TestResult<Self> {
        let posts = Arc::new(AtomicUsize::new(0));
        let backend_posts = Arc::clone(&posts);
        let result = SubmissionResult::AccountRefusal {
            check: tallyline_submission::api::model::SubmissionAccountCheck::NotRequired,
            code,
            http_status: 403,
            reason: SubmissionPauseReason::AppToken,
            outcome: Box::new(outcome),
        };
        let (driver, client) = HttpDriver::start(move || {
            Ok::<_, ()>(RecoveryBackend {
                posts: backend_posts,
                result,
            })
        })?;
        let seconds = Arc::new(AtomicU64::new(1000));
        let fixture = controller_support::fixture_with_clock(
            client,
            SubmissionMode::Production,
            name,
            Clock(Arc::clone(&seconds)),
        )
        .map_err(error)?;
        Ok(Self {
            fixture,
            driver,
            posts,
        })
    }
    fn release(&mut self, report: &Report) -> TestResult<JobId> {
        self.fixture
            .controller
            .release_manual(
                report.id(),
                report.version(),
                SubmissionMode::Production,
                &self.fixture.application,
            )
            .map(|release| release.job().id())
            .map_err(error)
    }
    fn tick(&mut self) -> TestResult {
        self.fixture
            .controller
            .tick(&self.fixture.application, None)
            .map(|_| ())
            .map_err(error)
    }
    fn job(&self, id: JobId) -> TestResult<&SubmissionJob> {
        self.fixture
            .controller
            .jobs()
            .iter()
            .find(|job| job.id() == id)
            .ok_or_else(|| error("missing fixture job"))
    }
    fn until(&mut self, id: JobId, phase: JobPhase) -> TestResult {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while self.job(id)?.phase() != phase {
            self.tick()?;
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        Ok(())
    }
    fn finish(mut self) -> TestResult {
        self.fixture.controller.shutdown().map_err(error)?;
        drop(self.fixture.controller);
        self.driver.finish()?;
        std::fs::remove_file(self.fixture.path)
    }
}
enum PendingChange {
    Acknowledge,
    NewAttempt,
}
fn pending(change: PendingChange) -> TestResult {
    let mut scenario = Scenario::new(
        "account-receipt",
        SubmissionResult::NeedsOutcomeCheck {
            code: code("no_api_found")?,
            http_status: 403,
        },
        Some(code("no_api_found")?),
    )?;
    let first = scenario.release(&scenario.fixture.report.clone())?;
    scenario.until(first, JobPhase::Held)?;
    let fences = scenario
        .fixture
        .controller
        .service_fences()
        .map_err(error)?;
    assert_eq!(fences.len(), 1);
    assert_eq!(
        fences
            .first()
            .ok_or_else(|| error("missing account fence"))?
            .cause(),
        SubmissionServiceCause::AccountPause
    );

    scenario.tick()?;
    match change {
        PendingChange::Acknowledge => {
            let revision = scenario.job(first)?.revision();
            let acknowledged = scenario
                .fixture
                .controller
                .retry_now(first, revision)
                .map_err(error)?;
            assert_eq!(acknowledged.phase(), JobPhase::Held);
            scenario.until(first, JobPhase::Rejected)?;
            assert_eq!(scenario.posts.load(Ordering::SeqCst), 1);
            let mut store = SqliteReferenceCache::open(&scenario.fixture.path).map_err(error)?;
            let restored = SubmissionJobRepository::load(&mut store, first)
                .map_err(error)?
                .ok_or_else(|| error("missing restored job"))?;
            assert_eq!(restored.phase(), JobPhase::Rejected);
            assert_eq!(restored.attempts().len(), 1);
            assert!(restored.service_pause_acknowledged());
            assert!(
                tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(
                    &mut store
                )
                .map_err(error)?
                .is_empty()
            );
            drop(store);
        }
        PendingChange::NewAttempt => reject_new_attempt(&mut scenario, first)?,
    }
    scenario.finish()
}
fn reject_new_attempt(scenario: &mut Scenario, first: JobId) -> TestResult {
    let mut store = SqliteReferenceCache::open(&scenario.fixture.path).map_err(error)?;
    let current = SubmissionJobRepository::load(&mut store, first)
        .map_err(error)?
        .ok_or_else(|| error("missing held job"))?;
    let resolved = current
        .apply(SubmissionCommand::ResolveOutcomeCheck {
            result: SubmissionResult::Paused(SubmissionPauseReason::AppToken),
        })
        .map_err(error)?;
    SubmissionJobRepository::save(&mut store, &resolved).map_err(error)?;
    let queued = scenario
        .fixture
        .controller
        .retry_now(first, resolved.state().revision())
        .map_err(error)?;
    let sending = queued
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1901),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .map_err(error)?;
    SubmissionJobRepository::save(&mut store, &sending).map_err(error)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Err(error) = scenario
            .fixture
            .controller
            .tick(&scenario.fixture.application, None)
        {
            assert_eq!(error, SubmissionControlError::Conflict);
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "a stale GET must refuse a new attempt"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        SubmissionJobRepository::load(&mut store, first)
            .map_err(error)?
            .as_ref(),
        Some(sending.state())
    );
    assert_eq!(scenario.posts.load(Ordering::SeqCst), 1);
    Ok(())
}
fn unrelated_draft(path: &std::path::Path) -> TestResult<Report> {
    let (mut store, gate) = support::stored_gate(path).map_err(error)?;
    let mut terminals = support::records()
        .map_err(error)?
        .iter()
        .find_map(|record| match record.data() {
            EndpointData::Terminals(terminals) => Some(terminals.clone()),
            EndpointData::Commodities(_)
            | EndpointData::GameVersions(_)
            | EndpointData::StatusLevels(_)
            | EndpointData::Parameters(_)
            | EndpointData::TerminalPriors(_) => None,
        })
        .ok_or_else(|| error("missing fixture terminals"))?;
    terminals.push(Terminal::new(
        TerminalId(12),
        TerminalText {
            name: "Other depot".to_owned(),
            nickname: "OD".to_owned(),
            display_name: "Other shop".to_owned(),
            location: "Pyro".to_owned(),
        },
        true,
    ));
    store
        .replace(&CachedReference::new(
            Timestamp::from_seconds(950),
            EndpointData::Terminals(terminals),
        ))
        .map_err(error)?;
    store
        .replace(&CachedReference::new(
            Timestamp::from_seconds(950),
            EndpointData::TerminalPriors(
                TerminalPriors::new(TerminalId(12), vec![]).map_err(error)?,
            ),
        ))
        .map_err(error)?;
    let mut report = gate.source().clone();
    report
        .change_key(
            ReportKey::new(
                TerminalId(12),
                report.key().side(),
                report.key().environment(),
            )
            .map_err(error)?,
        )
        .map_err(error)?;
    store
        .save_draft(&report, gate.source().version())
        .map_err(error)?;
    Ok(report)
}
fn receipt_fence(
    outcome: SubmissionResult,
    phase: JobPhase,
    code: Option<UexErrorCode>,
    name: &str,
) -> TestResult {
    let mut scenario = Scenario::new(name, outcome, code)?;
    let first = scenario.release(&scenario.fixture.report.clone())?;
    let other = unrelated_draft(&scenario.fixture.path)?;
    let second = scenario.release(&other)?;
    scenario.until(first, phase)?;
    for _ in 0..50 {
        scenario.tick()?;
    }
    assert_eq!(scenario.posts.load(Ordering::SeqCst), 1);
    assert_eq!(scenario.job(second)?.attempts().len(), 0);
    let fences = scenario
        .fixture
        .controller
        .service_fences()
        .map_err(error)?;
    assert_eq!(fences.len(), 1);
    let mut store = SqliteReferenceCache::open(&scenario.fixture.path).map_err(error)?;
    assert_eq!(
        tallyline_submission::api::SubmissionServiceFenceRepository::service_fences(&mut store)
            .map_err(error)?,
        fences
    );
    drop(store);
    let original = scenario.job(first)?.clone();
    let acknowledged = scenario
        .fixture
        .controller
        .retry_now(first, original.revision())
        .map_err(error)?;
    assert_eq!(acknowledged.phase(), phase);
    assert_eq!(acknowledged.attempts(), original.attempts());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while scenario.posts.load(Ordering::SeqCst) < 2 {
        scenario.tick()?;
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert_eq!(scenario.job(first)?.attempts().len(), 1);
    scenario.finish()
}
pub(super) fn verify() -> TestResult {
    CpuWorker::start("tallyline-account-contracts", || {
        pending(PendingChange::Acknowledge)?;
        pending(PendingChange::NewAttempt)?;
        receipt_fence(
            SubmissionResult::PartialError {
                code: code("no_api_found")?,
                row: None,
                source: tallyline_submission::api::model::RowFailureSource::Unmapped,
                receipts: Vec::new(),
            },
            JobPhase::PartiallyAccepted,
            Some(code("no_api_found")?),
            "partial-account",
        )?;
        receipt_fence(
            SubmissionResult::Unknown,
            JobPhase::OutcomeUnknown,
            None,
            "unknown-account",
        )?;
        receipt_fence(
            SubmissionResult::NeedsOutcomeCheck {
                code: code("Future_Code")?,
                http_status: 403,
            },
            JobPhase::Held,
            Some(code("Future_Code")?),
            "pending-account",
        )
    })?
    .finish()?
}
