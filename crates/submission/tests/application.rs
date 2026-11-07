// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Save-before-publish submission application contracts.

use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    JobId, JobPhase, PayloadHash, SubmissionApplicationError, SubmissionChange, SubmissionCommand,
    SubmissionEvent, SubmissionHeader, SubmissionJob, SubmissionRepositoryError, SubmissionRequest,
    SubmissionRow,
};
use tallyline_submission::api::{SubmissionApplication, SubmissionJobRepository};

fn queued() -> Option<SubmissionJob> {
    SubmissionJob::queued(
        JobId(1),
        SubmissionRequest::new(
            SubmissionHeader {
                report: ReportId(4),
                report_version: 3,
                account: "test-account".to_owned(),
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
                    RowId(2),
                    CommodityId(7),
                    "120".parse().ok()?,
                    "0".parse().ok()?,
                    1,
                )
                .ok()?,
            ],
        )
        .ok()?,
    )
    .ok()
}
struct Repository {
    job: SubmissionJob,
    events: Vec<SubmissionEvent>,
    fail: bool,
}
impl SubmissionJobRepository for Repository {
    fn load(&mut self, id: JobId) -> Result<Option<SubmissionJob>, SubmissionRepositoryError> {
        Ok((id == self.job.id()).then(|| self.job.clone()))
    }
    fn save(&mut self, change: &SubmissionChange) -> Result<(), SubmissionRepositoryError> {
        if self.fail {
            return Err(SubmissionRepositoryError::Unavailable);
        }
        if self.job.revision() != change.expected_revision() {
            return Err(SubmissionRepositoryError::Conflict);
        }
        self.job = change.state().clone();
        self.events.extend(change.event().cloned());
        Ok(())
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9__failed_persistence_publishes_nothing_and_recovery_is_idempotent() {
    let initial = queued()
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([6; 32]))
        .unwrap();
    let mut app = SubmissionApplication::new(Repository {
        job: initial.clone(),
        events: vec![],
        fail: true,
    });
    let command = SubmissionCommand::Recover {
        at: Timestamp::from_seconds(120),
    };
    assert_eq!(
        app.apply(JobId(1), 2, command.clone()),
        Err(SubmissionApplicationError::Repository(
            SubmissionRepositoryError::Unavailable
        ))
    );
    let mut repo = app.into_repository();
    assert_eq!(repo.job, initial);
    assert!(repo.events.is_empty());
    repo.fail = false;
    let mut app = SubmissionApplication::new(repo);
    let recovered = app.apply(JobId(1), 2, command.clone()).unwrap();
    assert_eq!(recovered.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(
        app.apply(JobId(1), 2, SubmissionCommand::Cancel),
        Err(SubmissionApplicationError::Repository(
            SubmissionRepositoryError::Conflict
        ))
    );
    assert_eq!(app.apply(JobId(1), 3, command).unwrap(), recovered);
    let repo = app.into_repository();
    assert_eq!(repo.events.len(), 1);
    assert_eq!(repo.events[0].id().version(), 3);
    assert_eq!(repo.job.attempts().len(), 1);
}
