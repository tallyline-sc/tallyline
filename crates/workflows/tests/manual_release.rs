// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Actual numeric-gated manual release coordination without technology in the workflow.

mod support;

use tallyline_game::api::model::{EnvironmentMapping, GameEnvironment};
use tallyline_reporting::api::ManualReviewer;
use tallyline_reporting::api::model::{
    ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy, Report,
    ReportKey, ReportState, StockSettings,
};
use tallyline_shared_kernel::api::model::{
    CommodityId, ObservationAgeSettings, ReportId, SubmissionJobId, SubmissionMode, TerminalId,
    Timestamp, TradeSide, UexEnvironment,
};
use tallyline_workflows::api::ManualRelease;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_3_r_sub_8__release_plan_keeps_exact_gate_source_and_final_queued_request() {
    let mut report = Report::manual(
        ReportId(1),
        ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
        Timestamp::from_seconds(900),
        Some("4.10.2"),
    )
    .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
    let context = ReleaseContext::new(
        ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
        SubmissionMode::DryRun,
        ReleaseScreenshotPolicy::Unknown,
        ReleaseWorkingCopy::Absent,
    );
    let gate = reviewer.submission_gate(
        report.clone(),
        &snapshot,
        context,
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let gate = tallyline_workflows::api::assess_release_observation_history(&gate, Some(&[]));
    let plan = ManualRelease::prepare(&gate, SubmissionJobId(7)).unwrap();
    assert_eq!(plan.source(), &report);
    assert_eq!(plan.report().state(), ReportState::Queued);
    assert_eq!(
        plan.report().release_binding().unwrap().job(),
        Some(SubmissionJobId(7))
    );
    assert_eq!(
        plan.job().request().header().report_version,
        plan.report().version()
    );
    assert_eq!(plan.job().request().header().account, "dummy-a");
    assert_eq!(plan.job().request().header().mode, SubmissionMode::DryRun);
    assert_eq!(
        plan.job().request().header().observed_at,
        report.observed_at()
    );
    assert_eq!(
        plan.job().request().rows()[0].stock(),
        report.rows()[0].stock().unwrap()
    );
    assert_eq!(plan.events().len(), 1);
    assert!(ManualRelease::prepare(&gate, SubmissionJobId(0)).is_err());
    let blocked = reviewer.submission_gate(
        report,
        &snapshot,
        ReleaseContext::new(
            ReleaseAccountStanding::Unavailable,
            SubmissionMode::Production,
            ReleaseScreenshotPolicy::Required,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    assert!(ManualRelease::prepare(&blocked, SubmissionJobId(8)).is_err());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_13_r_nf_11__manual_release_commits_report_job_and_outbox_atomically_and_reopens() {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_reference_data::api::ReferenceDataCache;
    use tallyline_reporting::api::ReportRepository;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-manual-release-{}.db",
        std::process::id()
    ));
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    for record in support::records().unwrap() {
        store.replace(&record).unwrap();
    }
    let mut report = store
        .create_manual(
            ReportKey::new(TerminalId(11), TradeSide::Buy, GameEnvironment::Live).unwrap(),
            Timestamp::from_seconds(900),
            Some("4.10.2"),
        )
        .unwrap();
    let row = report.add_commodity(CommodityId(42)).unwrap();
    report
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    report.set_stock(row, Some("0".parse().unwrap())).unwrap();
    report.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    report = report
        .set_faction_affinity(Some("0".parse().unwrap()))
        .unwrap();
    store.save_draft(&report, 0).unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        report.clone(),
        &support::snapshot(UexEnvironment::Live).unwrap(),
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::DryRun,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let release = store.release_manual(&gate).unwrap();
    assert_eq!(
        release
            .job()
            .request()
            .header()
            .faction_affinity
            .unwrap()
            .value(),
        0
    );
    assert!(store.load_draft(report.id()).unwrap().is_none());
    assert!(store.release_manual(&gate).is_err());
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, release.job().id())
            .unwrap()
            .as_ref(),
        Some(release.job())
    );
    assert!(reopened.drafts().unwrap().is_empty());
    use tallyline_submission::api::model::{PayloadHash, SubmissionCommand, SubmissionResult};
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([7; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut reopened, &sending).unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, release.job().id())
            .unwrap()
            .as_ref(),
        Some(sending.state())
    );
    let accepted = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::DryRunSucceeded,
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut reopened, &accepted).unwrap();
    SubmissionJobRepository::save(&mut reopened, &accepted).unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, release.job().id())
            .unwrap()
            .as_ref(),
        Some(accepted.state())
    );

    let database = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        database
            .query_row("SELECT COUNT(*) FROM domain_outbox", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        database
            .query_row(
                "SELECT phase FROM reports WHERE id=?1",
                [i64::try_from(report.id().0).unwrap()],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "submitted"
    );
    assert_eq!(
        database
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        17
    );
    drop(database);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_3_r_sub_8_r_nf_11__persisted_job_must_match_its_exact_committed_release_snapshot() {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::SubmissionRepositoryError;
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-release-identity-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let release = store.release_manual(&gate).unwrap();
    drop(store);
    let database = rusqlite::Connection::open(&path).unwrap();
    let original: String = database
        .query_row("SELECT payload FROM submission_jobs", [], |row| row.get(0))
        .unwrap();
    let corrupted = original.replace("dummy-a", "dummy-b");
    assert_ne!(corrupted, original);
    database
        .execute("UPDATE submission_jobs SET payload=?1", [corrupted])
        .unwrap();
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, release.job().id()),
        Err(SubmissionRepositoryError::Invalid)
    );
    database
        .execute("UPDATE submission_jobs SET payload=?1", [original])
        .unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, release.job().id())
            .unwrap()
            .as_ref(),
        Some(release.job())
    );
    drop(database);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_2__screenshot_required_dry_run_returns_draft_and_remembers_only_attempt_account_atomically()
 {
    use tallyline_reporting::api::ReportRepository;
    use tallyline_reporting::api::model::ReportFinding;
    use tallyline_submission::api::model::{
        PayloadHash, SubmissionCommand, SubmissionResult, SubmissionReturnReason,
    };
    use tallyline_submission::api::{ScreenshotPolicyStore, SubmissionJobRepository};
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-dry-screenshot-required-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let release = store.release_manual(&gate).unwrap();
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let returned = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::Returned(SubmissionReturnReason::ScreenshotRequired),
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &returned).unwrap();
    assert_eq!(
        store.required_since("dummy-a").unwrap(),
        Some(Timestamp::from_seconds(1001))
    );
    assert_eq!(store.required_since("dummy-b").unwrap(), None);
    let report = store.load_draft(gate.source().id()).unwrap().unwrap();
    assert_eq!(
        report.finding(),
        Some(ReportFinding::ReturnedForFix(
            tallyline_reporting::api::model::FixableReason::ScreenshotRequired
        ))
    );
    assert_eq!(report.rows(), release.report().rows());
    drop(store);
    let mut reopened = tallyline_adapter_storage::api::SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        reopened.required_since("dummy-a").unwrap(),
        Some(Timestamp::from_seconds(1001))
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, release.job().id())
            .unwrap()
            .as_ref(),
        Some(returned.state())
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_nf_11__failed_outbox_insert_rolls_back_both_aggregates_and_release_identity() {
    use tallyline_reporting::api::ReportRepository;
    use tallyline_workflows::api::{ManualReleaseError, ManualReleaseStore};
    let path = std::env::temp_dir().join(format!(
        "tallyline-release-rollback-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let database = rusqlite::Connection::open(&path).unwrap();
    database.execute_batch("CREATE TRIGGER reject_release_event BEFORE INSERT ON domain_outbox BEGIN SELECT RAISE(ABORT,'test transaction failure'); END;").unwrap();
    assert_eq!(
        store.release_manual(&gate),
        Err(ManualReleaseError::Unavailable)
    );
    assert_eq!(
        store.load_draft(gate.source().id()).unwrap().as_ref(),
        Some(gate.source())
    );
    for table in ["submission_jobs", "domain_outbox"] {
        let count: u32 = database
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
    database
        .execute_batch("DROP TRIGGER reject_release_event")
        .unwrap();
    let committed = store.release_manual(&gate).unwrap();
    assert_eq!(committed.job().id(), SubmissionJobId(1));
    drop(database);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_val_7__atomic_release_refuses_successful_reference_refresh_even_without_value_change()
{
    use tallyline_reference_data::api::ReferenceDataCache;
    use tallyline_reference_data::api::model::{CachedReference, EndpointData, GameVersions};
    use tallyline_workflows::api::{ManualReleaseError, ManualReleaseStore};
    let path = std::env::temp_dir().join(format!(
        "tallyline-release-reference-cas-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    store
        .replace(&CachedReference::new(
            Timestamp::from_seconds(951),
            EndpointData::GameVersions(GameVersions {
                live: "4.10.2".to_owned(),
                ptu: None,
            }),
        ))
        .unwrap();
    assert_eq!(
        store.release_manual(&gate),
        Err(ManualReleaseError::Conflict)
    );
    let database = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        database
            .query_row("SELECT COUNT(*) FROM submission_jobs", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        database
            .query_row("SELECT COUNT(*) FROM domain_outbox", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    drop(database);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_2__failed_account_policy_save_preserves_sending_and_can_commit_without_another_attempt()
 {
    use tallyline_reporting::api::ReportRepository;
    use tallyline_submission::api::model::{
        PayloadHash, SubmissionCommand, SubmissionRepositoryError, SubmissionResult,
        SubmissionReturnReason,
    };
    use tallyline_submission::api::{ScreenshotPolicyStore, SubmissionJobRepository};
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-policy-rollback-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let release = store.release_manual(&gate).unwrap();
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let returned = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::Returned(SubmissionReturnReason::ScreenshotRequired),
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    let database = rusqlite::Connection::open(&path).unwrap();
    database.execute_batch("CREATE TRIGGER reject_policy BEFORE INSERT ON screenshot_requirements BEGIN SELECT RAISE(ABORT,'test policy failure'); END;").unwrap();
    assert_eq!(
        SubmissionJobRepository::save(&mut store, &returned),
        Err(SubmissionRepositoryError::Unavailable)
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut store, release.job().id())
            .unwrap()
            .as_ref(),
        Some(sending.state())
    );
    assert!(store.load_draft(gate.source().id()).unwrap().is_none());
    assert_eq!(store.required_since("dummy-a").unwrap(), None);
    assert_eq!(
        database
            .query_row("SELECT COUNT(*) FROM domain_outbox", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
    database
        .execute_batch("DROP TRIGGER reject_policy")
        .unwrap();
    SubmissionJobRepository::save(&mut store, &returned).unwrap();
    assert_eq!(
        store.required_since("dummy-a").unwrap(),
        Some(Timestamp::from_seconds(1001))
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut store, release.job().id())
            .unwrap()
            .unwrap()
            .attempts()
            .len(),
        1
    );
    drop(database);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9__restart_returns_unsent_to_draft_and_closes_sending_unknown_without_replay() {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_reporting::api::ReportRepository;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        JobPhase, PayloadHash, SubmissionCommand, SubmissionRepositoryError,
    };
    use tallyline_workflows::api::{ManualReleaseStore, SubmissionQueueStore};
    let path =
        std::env::temp_dir().join(format!("tallyline-queue-restart-{}.db", std::process::id()));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let first = store.release_manual(&gate).unwrap();
    let mut second_source = store
        .create_manual(
            gate.source().key(),
            Timestamp::from_seconds(901),
            Some("4.10.2"),
        )
        .unwrap();
    let row = second_source.add_commodity(CommodityId(42)).unwrap();
    second_source
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    second_source
        .set_stock(row, Some("0".parse().unwrap()))
        .unwrap();
    second_source
        .set_status(row, std::num::NonZeroU8::new(3))
        .unwrap();
    store.save_draft(&second_source, 0).unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let second_gate = reviewer.submission_gate(
        second_source,
        &support::snapshot(UexEnvironment::Live).unwrap(),
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::DryRun,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let second = store.release_manual(&second_gate).unwrap();
    let sending = first
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([9; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    assert!(sending.state().apply(SubmissionCommand::Cancel).is_err());
    let simultaneous = second
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([10; 32]),
            date_added: Timestamp::from_seconds(901),
        })
        .unwrap();
    assert_eq!(
        SubmissionJobRepository::save(&mut store, &simultaneous),
        Err(SubmissionRepositoryError::Conflict)
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let recovered = reopened
        .recover_unfinished(Timestamp::from_seconds(1001))
        .unwrap();
    assert_eq!(recovered.len(), 2);
    let unknown = SubmissionJobRepository::load(&mut reopened, first.job().id())
        .unwrap()
        .unwrap();
    let returned = SubmissionJobRepository::load(&mut reopened, second.job().id())
        .unwrap()
        .unwrap();
    assert_eq!(unknown.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(unknown.attempts().len(), 1);
    assert_eq!(returned.phase(), JobPhase::Returned);
    assert!(returned.attempts().is_empty());
    assert_eq!(
        reopened.released_report(first.job().id()).unwrap().as_ref(),
        Some(first.report())
    );
    assert_eq!(reopened.queue_jobs().unwrap().len(), 2);
    assert!(reopened.load_draft(first.report().id()).unwrap().is_none());
    let draft = reopened.load_draft(second.report().id()).unwrap().unwrap();
    assert_eq!(draft.state(), ReportState::Draft);
    assert_eq!(draft.rows(), second.report().rows());
    assert!(
        reopened
            .recover_unfinished(Timestamp::from_seconds(1002))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, first.job().id()).unwrap(),
        Some(unknown)
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_sub_9_r_nf_11__full_production_acceptance_persists_local_prior_until_next_successful_refresh()
 {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_reference_data::api::model::{
        CachedReference, EndpointData, GameVersions, LocalPriorProjection, PriorReferenceSource,
    };
    use tallyline_reference_data::api::{AcceptedPriorHistoryStore, ReferenceDataCache};
    use tallyline_reporting::api::ReportRepository;
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{PayloadHash, SubmissionCommand, SubmissionResult};
    use tallyline_workflows::api::ManualReleaseStore;
    let path = std::env::temp_dir().join(format!(
        "tallyline-accepted-ledger-{}.db",
        std::process::id()
    ));
    let (mut store, original_gate) = support::stored_gate(&path).unwrap();
    let mut report = original_gate.source().clone();
    let row = report.rows()[0].id();
    report.set_price(row, Some("124".parse().unwrap())).unwrap();
    store
        .save_draft(&report, original_gate.source().version())
        .unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        report,
        &support::snapshot(UexEnvironment::Live).unwrap(),
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::Production,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let release = store.release_manual(&gate).unwrap();
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([10; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let accepted = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(900)).unwrap(),
            ]),
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    let failing = rusqlite::Connection::open(&path).unwrap();
    failing.execute_batch("CREATE TRIGGER reject_accepted BEFORE INSERT ON accepted_priors BEGIN SELECT RAISE(ABORT,'test accepted history failure'); END;").unwrap();
    assert!(SubmissionJobRepository::save(&mut store, &accepted).is_err());
    assert_eq!(
        SubmissionJobRepository::load(&mut store, release.job().id())
            .unwrap()
            .as_ref(),
        Some(sending.state())
    );
    assert!(store.accepted_history().unwrap().records().is_empty());
    failing
        .execute_batch("DROP TRIGGER reject_accepted")
        .unwrap();
    drop(failing);
    SubmissionJobRepository::save(&mut store, &accepted).unwrap();
    let history = store.accepted_history().unwrap();
    assert_eq!(history.records().len(), 1);
    assert_eq!(
        history.records()[0].prior().context().submission,
        release.job().id()
    );
    assert_eq!(
        history.records()[0].prior().values().price.to_string(),
        "124"
    );
    assert_eq!(
        history.records()[0].projection(),
        LocalPriorProjection::Active
    );
    let database = rusqlite::Connection::open(&path).unwrap();
    let original: String = database
        .query_row("SELECT payload FROM accepted_priors", [], |row| row.get(0))
        .unwrap();
    let corrupted = original.replace("\"price\":\"124\"", "\"price\":\"200\"");
    assert_ne!(corrupted, original);
    database
        .execute("UPDATE accepted_priors SET payload=?1", [corrupted])
        .unwrap();
    assert!(matches!(
        store.accepted_history(),
        Err(tallyline_adapter_storage::api::StorageError::InvalidCache)
    ));
    database
        .execute("UPDATE accepted_priors SET payload=?1", [original])
        .unwrap();
    drop(database);
    let projected = support::snapshot(UexEnvironment::Live)
        .unwrap()
        .with_local_priors(&history.active(UexEnvironment::Live))
        .unwrap();
    assert_eq!(
        projected
            .prior_reference(TerminalId(11), CommodityId(42), TradeSide::Buy)
            .unwrap()
            .source(),
        PriorReferenceSource::AcceptedLocal
    );
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.accepted_history().unwrap(), history);
    reopened
        .replace(&CachedReference::new(
            Timestamp::from_seconds(1002),
            EndpointData::GameVersions(GameVersions {
                live: "4.10.2".to_owned(),
                ptu: None,
            }),
        ))
        .unwrap();
    let refreshed = reopened.accepted_history().unwrap();
    assert_eq!(refreshed.records().len(), 1);
    assert_eq!(
        refreshed.records()[0].projection(),
        LocalPriorProjection::Refreshed
    );
    assert!(refreshed.active(UexEnvironment::Live).is_empty());
    SubmissionJobRepository::save(&mut reopened, &accepted).unwrap();
    assert_eq!(reopened.accepted_history().unwrap(), refreshed);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_val_7__release_refuses_local_comparison_evidence_not_present_in_committed_history() {
    use tallyline_reference_data::api::model::{
        AcceptedLocalPrior, AcceptedPriorContext, PriorValues,
    };
    use tallyline_reporting::api::ReportRepository;
    use tallyline_workflows::api::{ManualReleaseError, ManualReleaseStore};
    let path = std::env::temp_dir().join(format!(
        "tallyline-local-release-cas-{}.db",
        std::process::id()
    ));
    let (mut store, original) = support::stored_gate(&path).unwrap();
    let mut report = original.source().clone();
    let row = report.rows()[0].id();
    report.set_price(row, Some("200".parse().unwrap())).unwrap();
    store
        .save_draft(&report, original.source().version())
        .unwrap();
    let local = AcceptedLocalPrior::new(
        std::sync::Arc::new(AcceptedPriorContext {
            report: ReportId(9),
            report_version: 10,
            submission: SubmissionJobId(77),
            environment: UexEnvironment::Live,
            terminal: report.key().terminal(),
            side: report.key().side(),
            game_version: "4.10.2".to_owned(),
            observed_at: Timestamp::from_seconds(850),
            accepted_at: Timestamp::from_seconds(860),
        }),
        CommodityId(42),
        PriorValues {
            price: "200".parse().unwrap(),
            stock: "0".parse().unwrap(),
            status: std::num::NonZeroU8::new(3),
            container_sizes: vec![],
        },
    )
    .unwrap();
    let snapshot = support::snapshot(UexEnvironment::Live)
        .unwrap()
        .with_local_priors(&[local])
        .unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        report,
        &snapshot,
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::DryRun,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let gate = tallyline_workflows::api::assess_release_observation_history(&gate, Some(&[]));
    assert!(gate.blocks(gate.report()).is_empty());
    assert_eq!(
        store.release_manual(&gate),
        Err(ManualReleaseError::Conflict)
    );
    assert!(store.load_draft(gate.source().id()).unwrap().is_some());
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_5_r_sub_2__release_rechecks_committed_account_screenshot_policy_before_enqueuing() {
    use tallyline_reporting::api::model::ReleaseBlockReason;
    use tallyline_submission::api::ScreenshotPolicyStore;
    use tallyline_workflows::api::{ManualReleaseError, ManualReleaseStore};
    let path = std::env::temp_dir().join(format!(
        "tallyline-release-policy-cas-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let gate = tallyline_workflows::api::assess_release_observation_history(&gate, Some(&[]));
    assert!(gate.blocks(gate.report()).is_empty());
    store
        .remember("dummy-a", Timestamp::from_seconds(1000))
        .unwrap();
    assert_eq!(
        store.release_manual(&gate),
        Err(ManualReleaseError::Blocked(vec![
            ReleaseBlockReason::ImageRequired
        ]))
    );
    let database = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        database
            .query_row("SELECT COUNT(*) FROM submission_jobs", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        database
            .query_row("SELECT COUNT(*) FROM domain_outbox", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    drop(database);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_8_r_sub_2__older_release_requires_persisted_exact_scope_decision_and_still_waits_for_cooldown()
 {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_reference_data::api::AcceptedPriorHistoryStore;
    use tallyline_reporting::api::ReportRepository;
    use tallyline_reporting::api::model::{NewerObservationEvidence, ReleaseBlockReason};
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::model::{
        PayloadHash, ScheduleDecision, SubmissionCommand, SubmissionResult,
    };
    use tallyline_submission::api::{SubmissionJobRepository, SubmissionSchedule};
    use tallyline_workflows::api::{ManualReleaseError, ManualReleaseStore, SubmissionQueueStore};
    let path = std::env::temp_dir().join(format!(
        "tallyline-newer-history-release-{}.db",
        std::process::id()
    ));
    let (mut store, original) = support::stored_gate(&path).unwrap();
    let mut newer = store
        .create_manual(
            original.source().key(),
            Timestamp::from_seconds(1100),
            Some("4.10.2"),
        )
        .unwrap();
    let row = newer.add_commodity(CommodityId(42)).unwrap();
    newer
        .set_price(row, Some("123.45".parse().unwrap()))
        .unwrap();
    newer.set_stock(row, Some("0".parse().unwrap())).unwrap();
    newer.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
    store.save_draft(&newer, 0).unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let context = ReleaseContext::new(
        ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
        SubmissionMode::Production,
        ReleaseScreenshotPolicy::Unknown,
        ReleaseWorkingCopy::Absent,
    );
    let base = support::snapshot(UexEnvironment::Live).unwrap();
    let newer_gate = reviewer.submission_gate(
        newer,
        &base,
        context.clone(),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1200),
    );
    let released = store.release_manual(&newer_gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1200),
            payload_hash: PayloadHash::new([11; 32]),
            date_added: Timestamp::from_seconds(1100),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let accepted = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(42, Timestamp::from_seconds(1100)).unwrap(),
            ]),
            at: Timestamp::from_seconds(1201),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &accepted).unwrap();
    let current = base
        .with_local_priors(
            &store
                .accepted_history()
                .unwrap()
                .active(UexEnvironment::Live),
        )
        .unwrap();
    let older_gate = reviewer.submission_gate(
        original.source().clone(),
        &current,
        context.clone(),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1202),
    );
    assert_eq!(
        store.release_manual(&older_gate),
        Err(ManualReleaseError::Blocked(vec![
            ReleaseBlockReason::NewerObservationSent {
                row: original.source().rows()[0].id(),
                observed_at: Timestamp::from_seconds(1100)
            }
        ]))
    );
    let decision = original
        .source()
        .confirm_newer_observation(
            original.source().rows()[0].id(),
            NewerObservationEvidence::new(
                "dummy-a",
                SubmissionMode::Production,
                UexEnvironment::Live,
                Timestamp::from_seconds(1100),
            )
            .unwrap(),
            Timestamp::from_seconds(1202),
        )
        .unwrap();
    store
        .save_draft(decision.state(), original.source().version())
        .unwrap();
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let approved = reopened
        .load_draft(original.source().id())
        .unwrap()
        .unwrap();
    assert_eq!(&approved, decision.state());
    for (account, mode, environment) in [
        ("dummy-b", SubmissionMode::Production, UexEnvironment::Live),
        ("dummy-a", SubmissionMode::DryRun, UexEnvironment::Live),
        ("dummy-a", SubmissionMode::Production, UexEnvironment::Ptu),
    ] {
        assert!(
            tallyline_workflows::api::manual_submission_request(
                &approved,
                environment,
                account,
                mode
            )
            .unwrap()
            .newer_observation_confirmations()
            .is_empty()
        );
    }
    let gate = reviewer.submission_gate(
        approved,
        &current,
        context,
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1202),
    );
    let older = reopened.release_manual(&gate).unwrap();
    assert_eq!(
        older
            .job()
            .request()
            .newer_observation_confirmations()
            .len(),
        1
    );
    assert_eq!(
        SubmissionSchedule::assess(
            older.job(),
            &reopened.queue_jobs().unwrap(),
            Timestamp::from_seconds(1202)
        ),
        ScheduleDecision::WaitingUntil(Timestamp::from_seconds(1400))
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_8_r_sub_9__newer_unknown_or_partial_history_cannot_be_bypassed_by_a_row_decision() {
    use tallyline_reporting::api::ReportRepository;
    use tallyline_reporting::api::model::{NewerObservationEvidence, ReleaseBlockReason};
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{PayloadHash, SubmissionCommand, SubmissionResult};
    use tallyline_workflows::api::{ManualReleaseError, ManualReleaseStore};
    for (label, result) in [
        ("unknown", SubmissionResult::Unknown),
        ("partial", SubmissionResult::Partial(Vec::new())),
    ] {
        let path = std::env::temp_dir().join(format!(
            "tallyline-unresolved-newer-{label}-{}.db",
            std::process::id()
        ));
        let (mut store, original) = support::stored_gate(&path).unwrap();
        let mut newer = store
            .create_manual(
                original.source().key(),
                Timestamp::from_seconds(1100),
                Some("4.10.2"),
            )
            .unwrap();
        let row = newer.add_commodity(CommodityId(42)).unwrap();
        newer
            .set_price(row, Some("123.45".parse().unwrap()))
            .unwrap();
        newer.set_stock(row, Some("0".parse().unwrap())).unwrap();
        newer.set_status(row, std::num::NonZeroU8::new(3)).unwrap();
        store.save_draft(&newer, 0).unwrap();
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
        let snapshot = support::snapshot(UexEnvironment::Live).unwrap();
        let gate = reviewer.submission_gate(
            newer,
            &snapshot,
            context.clone(),
            ObservationAgeSettings::default(),
            Timestamp::from_seconds(1200),
        );
        let release = store.release_manual(&gate).unwrap();
        let sending = release
            .job()
            .apply(SubmissionCommand::Begin {
                at: Timestamp::from_seconds(1200),
                payload_hash: PayloadHash::new([12; 32]),
                date_added: Timestamp::from_seconds(1100),
            })
            .unwrap();
        SubmissionJobRepository::save(&mut store, &sending).unwrap();
        let outcome = sending
            .state()
            .apply(SubmissionCommand::Complete {
                result,
                at: Timestamp::from_seconds(1201),
            })
            .unwrap();
        SubmissionJobRepository::save(&mut store, &outcome).unwrap();
        let decision = original
            .source()
            .confirm_newer_observation(
                original.source().rows()[0].id(),
                NewerObservationEvidence::new(
                    "dummy-a",
                    SubmissionMode::DryRun,
                    UexEnvironment::Live,
                    Timestamp::from_seconds(1100),
                )
                .unwrap(),
                Timestamp::from_seconds(1202),
            )
            .unwrap();
        store
            .save_draft(decision.state(), original.source().version())
            .unwrap();
        let gate = reviewer.submission_gate(
            decision.into_state(),
            &snapshot,
            context,
            ObservationAgeSettings::default(),
            Timestamp::from_seconds(1202),
        );
        assert!(
            matches!(store.release_manual(&gate), Err(ManualReleaseError::Blocked(blocks)) if blocks.iter().any(|block| matches!(block, ReleaseBlockReason::UnresolvedNewerObservation { submission, .. } if *submission == release.job().id())))
        );
        assert!(store.load_draft(original.source().id()).unwrap().is_some());
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_sub_9_r_nf_11__outbox_delivery_replays_after_handler_or_ack_failure_without_releasing_a_second_job()
 {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_workflows::api::{
        EventOutbox, ManualReleaseStore, OutboxError, dispatch_pending_events,
    };
    let path = std::env::temp_dir().join(format!(
        "tallyline-outbox-delivery-{}.db",
        std::process::id()
    ));
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let release = store.release_manual(&gate).unwrap();
    let original = store.pending_events().unwrap();
    assert_eq!(original.len(), 1);
    assert_eq!(original[0].id(), release.events()[0].id());
    let mut handled = Vec::new();
    assert_eq!(
        dispatch_pending_events(&mut store, |event| {
            handled.push(event.id());
            Err(OutboxError::Unavailable)
        }),
        Err(OutboxError::Unavailable)
    );
    assert_eq!(store.pending_events().unwrap(), original);
    let database = rusqlite::Connection::open(&path).unwrap();
    database.execute_batch("CREATE TRIGGER refuse_delivery BEFORE UPDATE ON domain_outbox BEGIN SELECT RAISE(ABORT,'recorded acknowledgement failure'); END;").unwrap();
    assert_eq!(
        dispatch_pending_events(&mut store, |event| {
            handled.push(event.id());
            Ok(())
        }),
        Err(OutboxError::Unavailable)
    );
    assert_eq!(store.pending_events().unwrap(), original);
    drop(store);
    database
        .execute_batch("DROP TRIGGER refuse_delivery;")
        .unwrap();
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        dispatch_pending_events(&mut reopened, |event| {
            handled.push(event.id());
            Ok(())
        }),
        Ok(1)
    );
    assert!(reopened.pending_events().unwrap().is_empty());
    reopened.mark_delivered(original[0]).unwrap();
    assert_eq!(handled, vec![original[0].id(); 3]);
    assert_eq!(
        tallyline_workflows::api::SubmissionQueueStore::queue_jobs(&mut reopened)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        dispatch_pending_events(&mut reopened, |_| panic!(
            "delivered notifications must not replay"
        )),
        Ok(0)
    );
    drop(reopened);
    drop(database);
    std::fs::remove_file(path).unwrap();
}

#[path = "manual_release/unknown_resolution.rs"]
mod unknown_resolution;

#[path = "manual_release/partial_resolution.rs"]
mod partial_resolution;

#[path = "manual_release/three_rows.rs"]
mod three_rows;

#[path = "manual_release/cooldown_split.rs"]
mod cooldown_split;

#[path = "manual_release/rejected_copy.rs"]
mod rejected_copy;
