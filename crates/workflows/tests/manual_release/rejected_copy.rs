// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Permanent-refusal copying through public workflow and real SQLite contracts.

use super::support;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_reporting::api::model::ReportState;
use tallyline_reporting::api::{ReportHistoryRepository, ReportRepository};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    JobPhase, PayloadHash, SubmissionCommand, SubmissionCorrection, SubmissionRejection,
    SubmissionResolutionAction, SubmissionResult, UexErrorCode,
};
use tallyline_workflows::api::{ManualReleaseStore, ResolutionDraftStore, SubmissionQueueStore};
use tempfile::Builder;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_sub_10_r_val_7__rejected_origin_copies_to_a_distinct_unreviewed_draft_and_reopens() {
    let fixture = Builder::new()
        .prefix("rejected-copy-")
        .tempdir()
        .expect("an exclusive SQLite fixture directory must be created");
    let path = fixture.path().join("application.db");
    let (mut store, gate) = support::stored_gate(&path).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let rejection = SubmissionResult::Rejected(SubmissionRejection::new(
        UexErrorCode::new("invalid_input".to_owned()).unwrap(),
        SubmissionCorrection::ReviewClient,
    ));
    let rejected = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: rejection.clone(),
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &rejected).unwrap();
    let origin_report = store
        .retained_report(released.report().id())
        .unwrap()
        .unwrap();
    assert_eq!(origin_report.state(), ReportState::Rejected);
    assert!(store.drafts().unwrap().is_empty());

    let duplicated = store.duplicate_resolved(
        rejected.state().id(),
        rejected.state().revision(),
        Timestamp::from_seconds(1002),
    );
    assert!(
        duplicated.is_ok(),
        "a ReviewClient refusal must allow an explicit new Draft: {duplicated:?}"
    );
    let duplicated = duplicated.unwrap();
    assert_ne!(duplicated.draft.id(), origin_report.id());
    assert_eq!(duplicated.draft.state(), ReportState::Draft);
    assert!(duplicated.draft.release_binding().is_none());
    assert_eq!(duplicated.draft.key(), origin_report.key());
    assert_eq!(duplicated.draft.observed_at(), Timestamp::from_seconds(900));
    assert_eq!(duplicated.draft.game_version(), Some("4.10.2"));
    assert_eq!(
        duplicated.draft.faction_affinity(),
        origin_report.faction_affinity()
    );
    assert_eq!(duplicated.draft.rows().len(), 1);
    let row = &duplicated.draft.rows()[0];
    assert_eq!(row.commodity(), origin_report.rows()[0].commodity());
    assert_eq!(row.price().unwrap().to_string(), "123.45");
    assert_eq!(row.stock().unwrap().to_string(), "0");
    assert_eq!(row.status().unwrap().get(), 3);
    assert!(row.price_confirmation().is_none());
    assert!(row.stock_confirmation().is_none());
    assert!(row.status_confirmation().is_none());
    assert!(duplicated.draft.age_confirmation().is_none());
    assert!(
        duplicated
            .draft
            .newer_observation_confirmations()
            .is_empty()
    );
    assert_eq!(duplicated.origin.phase(), JobPhase::Rejected);
    assert_eq!(duplicated.origin.result(), Some(&rejection));
    assert_eq!(duplicated.origin.attempts(), rejected.state().attempts());
    assert_eq!(duplicated.origin.request(), rejected.state().request());
    assert_eq!(
        duplicated.origin.revision(),
        rejected.state().revision() + 1
    );
    assert_eq!(
        duplicated.origin.resolution_actions(),
        &[SubmissionResolutionAction::Duplicated {
            report: duplicated.draft.id(),
            at: Timestamp::from_seconds(1002),
        }]
    );
    assert!(!duplicated.origin.can_retry());
    assert_eq!(
        store.retained_report(origin_report.id()).unwrap().unwrap(),
        origin_report
    );
    assert_eq!(store.queue_jobs().unwrap(), vec![duplicated.origin.clone()]);
    drop(store);

    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(reopened.drafts().unwrap(), vec![duplicated.draft]);
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, duplicated.origin.id()).unwrap(),
        Some(duplicated.origin)
    );
    assert_eq!(
        reopened.retained_report(origin_report.id()).unwrap(),
        Some(origin_report)
    );
    drop(reopened);
    fixture
        .close()
        .expect("the complete SQLite fixture directory must be removed");
}

fn stored_rejection(
    path: &std::path::Path,
    correction: SubmissionCorrection,
) -> Result<
    (
        SqliteReferenceCache,
        tallyline_submission::api::model::SubmissionJob,
    ),
    String,
> {
    let (mut store, gate) = support::stored_gate(path).map_err(fixture_error)?;
    let released = store.release_manual(&gate).map_err(fixture_error)?;
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .map_err(fixture_error)?;
    SubmissionJobRepository::save(&mut store, &sending).map_err(fixture_error)?;
    let rejected = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::Rejected(SubmissionRejection::new(
                UexErrorCode::new("opaque_refusal".to_owned()).ok_or("invalid fixture refusal")?,
                correction,
            )),
            at: Timestamp::from_seconds(1001),
        })
        .map_err(fixture_error)?;
    SubmissionJobRepository::save(&mut store, &rejected).map_err(fixture_error)?;
    Ok((store, rejected.into_state()))
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_nf_11__copy_conflicts_and_outbox_failure_leave_no_partial_draft_or_history() {
    use tallyline_submission::api::model::SubmissionRepositoryError;
    use tallyline_workflows::api::EventOutbox;
    let fixture = Builder::new()
        .prefix("rejected-rollback-")
        .tempdir()
        .unwrap();
    let path = fixture.path().join("application.db");
    let (mut store, rejected) =
        stored_rejection(&path, SubmissionCorrection::ReviewClient).unwrap();
    let report = store
        .retained_report(rejected.request().header().report)
        .unwrap()
        .unwrap();
    let events = store.pending_events().unwrap();
    assert_eq!(
        store.duplicate_resolved(
            rejected.id(),
            rejected.revision() - 1,
            Timestamp::from_seconds(1002)
        ),
        Err(SubmissionRepositoryError::Conflict)
    );
    let fault = rusqlite::Connection::open(&path).unwrap();
    fault.execute_batch("CREATE TRIGGER fail_rejected_copy BEFORE INSERT ON domain_outbox WHEN NEW.event_source='submission' BEGIN SELECT RAISE(ABORT,'injected copy rollback'); END;").unwrap();
    assert_eq!(
        store.duplicate_resolved(
            rejected.id(),
            rejected.revision(),
            Timestamp::from_seconds(1002)
        ),
        Err(SubmissionRepositoryError::Unavailable)
    );
    drop(store);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert!(store.drafts().unwrap().is_empty());
    assert_eq!(store.queue_jobs().unwrap(), vec![rejected.clone()]);
    assert_eq!(
        store.retained_report(report.id()).unwrap(),
        Some(report.clone())
    );
    assert_eq!(store.pending_events().unwrap(), events);
    fault
        .execute_batch("DROP TRIGGER fail_rejected_copy")
        .unwrap();
    drop(fault);
    let copied = store
        .duplicate_resolved(
            rejected.id(),
            rejected.revision(),
            Timestamp::from_seconds(1002),
        )
        .unwrap();
    assert_eq!(store.pending_events().unwrap().len(), events.len() + 1);
    assert_eq!(store.retained_report(report.id()).unwrap(), Some(report));
    assert_eq!(
        store.duplicate_resolved(
            copied.origin.id(),
            copied.origin.revision(),
            Timestamp::from_seconds(1003)
        ),
        Err(SubmissionRepositoryError::Invalid)
    );
    assert_eq!(store.drafts().unwrap(), vec![copied.draft]);
    drop(store);
    fixture.close().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11__new_capture_refusal_never_creates_a_copy() {
    let fixture = Builder::new()
        .prefix("rejected-new-capture-")
        .tempdir()
        .unwrap();
    let (mut store, rejected) = stored_rejection(
        &fixture.path().join("application.db"),
        SubmissionCorrection::NewCapture,
    )
    .unwrap();
    assert!(
        store
            .duplicate_resolved(
                rejected.id(),
                rejected.revision(),
                Timestamp::from_seconds(1002)
            )
            .is_err()
    );
    assert!(store.drafts().unwrap().is_empty());
    assert_eq!(store.queue_jobs().unwrap(), vec![rejected]);
    drop(store);
    fixture.close().unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_11_r_val_7_r_man_5__copy_drops_actual_price_age_and_attachment_approvals_and_ownership() {
    use tallyline_reporting::api::model::AttachmentBlock;
    use tallyline_shared_kernel::api::model::CommodityId;
    let fixture = Builder::new()
        .prefix("rejected-approvals-")
        .tempdir()
        .unwrap();
    let path = fixture.path().join("application.db");
    use tallyline_reference_data::api::{AcceptedPriorHistoryStore, ReferenceDataCache};
    use tallyline_reporting::api::model::{
        NewerObservationEvidence, ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy,
        ReleaseWorkingCopy, StockSettings,
    };
    use tallyline_reporting::api::{ManualReviewer, ReportingApplication};
    use tallyline_shared_kernel::api::model::{
        ObservationAgeSettings, SubmissionMode, UexEnvironment,
    };
    let (mut store, initial) = super::three_rows::configured_gate(
        &path,
        Some("0".parse().unwrap()),
        Some(CommodityId(45)),
    )
    .unwrap();
    let snapshot = support::snapshot_records(
        ReferenceDataCache::load(&mut store).unwrap(),
        UexEnvironment::Live,
    )
    .unwrap();
    let source = initial.source();
    let row = source.rows()[0].id();
    let mut application = ReportingApplication::new(store, support::Fixed);
    let stock = application
        .confirm_stock(source.id(), source.version(), row, &snapshot)
        .unwrap();
    let status = application
        .confirm_status(stock.id(), stock.version(), row, &snapshot)
        .unwrap();
    drop(application);
    let mut history_store = SqliteReferenceCache::open(&path).unwrap();
    record_newer_acceptance(&mut history_store, &snapshot, status.key()).unwrap();
    drop(history_store);
    let approved = status
        .confirm_newer_observation(
            row,
            NewerObservationEvidence::new(
                "dummy-a",
                SubmissionMode::Production,
                UexEnvironment::Live,
                Timestamp::from_seconds(950),
            )
            .unwrap(),
            Timestamp::from_seconds(1000),
        )
        .unwrap()
        .into_state();
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    store.save_draft(&approved, status.version()).unwrap();
    let snapshot = snapshot
        .with_local_priors(
            &store
                .accepted_history()
                .unwrap()
                .active(UexEnvironment::Live),
        )
        .unwrap();
    let attachment = approved.attachment().unwrap();
    let reviewer = ManualReviewer::new(
        tallyline_game::api::model::EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        approved,
        &snapshot,
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::Production,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Verified {
                binding: attachment.binding(),
                digest: attachment.working_copy().unwrap(),
            },
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let gate = tallyline_workflows::api::assess_release_observation_history(
        &gate,
        Some(&store.queue_jobs().unwrap()),
    );
    let source = gate.report();
    assert!(source.rows()[0].stock_confirmation().is_some());
    assert!(source.rows()[0].status_confirmation().is_some());
    assert_eq!(source.newer_observation_confirmations().len(), 1);
    assert!(source.rows()[0].price_confirmation().is_some());
    assert!(source.age_confirmation().is_some());
    let attachment = source.attachment().unwrap();
    assert!(attachment.preview_confirmed());
    assert_eq!(attachment.working_copy_owner(), Some(source.id()));
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(4000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let rejected = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::Rejected(SubmissionRejection::new(
                UexErrorCode::new("opaque_refusal".to_owned()).unwrap(),
                SubmissionCorrection::ReviewClient,
            )),
            at: Timestamp::from_seconds(4001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &rejected).unwrap();
    let copy = store
        .duplicate_resolved(
            rejected.state().id(),
            rejected.state().revision(),
            Timestamp::from_seconds(4002),
        )
        .unwrap();
    assert_eq!(copy.draft.faction_affinity().unwrap().value(), 0);
    assert_eq!(copy.draft.rows()[0].price(), source.rows()[0].price());
    assert!(copy.draft.rows()[0].price_confirmation().is_none());
    assert!(copy.draft.rows()[0].stock_confirmation().is_none());
    assert!(copy.draft.rows()[0].status_confirmation().is_none());
    assert!(copy.draft.age_confirmation().is_none());
    assert!(copy.draft.newer_observation_confirmations().is_empty());
    let copied_attachment = copy.draft.attachment().unwrap();
    assert_eq!(copied_attachment.binding(), attachment.binding());
    assert!(!copied_attachment.preview_confirmed());
    assert_eq!(copied_attachment.working_copy(), None);
    assert_eq!(copied_attachment.working_copy_owner(), None);
    assert_eq!(
        copy.draft.attachment_gate(),
        Err(AttachmentBlock::PreviewUnconfirmed)
    );
    assert_eq!(copy.draft.observed_at(), source.observed_at());
    assert_eq!(copy.draft.game_version(), source.game_version());
    drop(store);
    fixture.close().unwrap();
}

fn record_newer_acceptance(
    store: &mut SqliteReferenceCache,
    snapshot: &tallyline_reference_data::api::model::ReferenceSnapshot,
    key: tallyline_reporting::api::model::ReportKey,
) -> Result<(), String> {
    use tallyline_reporting::api::model::{
        ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        StockSettings,
    };
    use tallyline_reporting::api::{ManualReviewer, ReportRepository};
    use tallyline_shared_kernel::api::model::{
        CommodityId, ObservationAgeSettings, SubmissionMode, UexReportId,
    };
    let mut report = store
        .create_manual(key, Timestamp::from_seconds(950), Some("4.10.2"))
        .map_err(fixture_error)?;
    let row = report
        .add_commodity(CommodityId(42))
        .map_err(fixture_error)?;
    report
        .set_price(row, Some("123.45".parse().map_err(fixture_error)?))
        .map_err(fixture_error)?;
    report
        .set_stock(row, Some("0".parse().map_err(fixture_error)?))
        .map_err(fixture_error)?;
    report
        .set_status(row, std::num::NonZeroU8::new(3))
        .map_err(fixture_error)?;
    store.save_draft(&report, 0).map_err(fixture_error)?;
    let reviewer = ManualReviewer::new(
        tallyline_game::api::model::EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        report,
        snapshot,
        ReleaseContext::new(
            ReleaseAccountStanding::Accepted("dummy-a".to_owned()),
            SubmissionMode::Production,
            ReleaseScreenshotPolicy::Unknown,
            ReleaseWorkingCopy::Absent,
        ),
        ObservationAgeSettings::default(),
        Timestamp::from_seconds(1000),
    );
    let gate = tallyline_workflows::api::assess_release_observation_history(
        &gate,
        Some(&store.queue_jobs().map_err(fixture_error)?),
    );
    let released = store.release_manual(&gate).map_err(fixture_error)?;
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([9; 32]),
            date_added: Timestamp::from_seconds(950),
        })
        .map_err(fixture_error)?;
    SubmissionJobRepository::save(store, &sending).map_err(fixture_error)?;
    let accepted = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(100, Timestamp::from_seconds(950))
                    .ok_or("invalid fixture receipt")?,
            ]),
            at: Timestamp::from_seconds(1001),
        })
        .map_err(fixture_error)?;
    SubmissionJobRepository::save(store, &accepted).map_err(fixture_error)?;
    Ok(())
}

fn fixture_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
