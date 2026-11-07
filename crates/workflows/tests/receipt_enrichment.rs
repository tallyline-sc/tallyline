// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Optional receipt enrichment is durable, bounded once and cannot regress acceptance.

mod support;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_nf_11__receipt_enrichment_is_claimed_once_and_survives_a_failed_mapping_save_and_restart()
 {
    use tallyline_adapter_storage::api::SqliteReferenceCache;
    use tallyline_game::api::model::EnvironmentMapping;
    use tallyline_reporting::api::ManualReviewer;
    use tallyline_reporting::api::model::{
        ReleaseAccountStanding, ReleaseContext, ReleaseScreenshotPolicy, ReleaseWorkingCopy,
        StockSettings,
    };
    use tallyline_shared_kernel::api::model::{
        ObservationAgeSettings, SubmissionMode, Timestamp, UexReportId,
    };
    use tallyline_submission::api::SubmissionJobRepository;
    use tallyline_submission::api::model::{
        PayloadHash, SubmissionCommand, SubmissionRepositoryError, SubmissionResult,
        VerifiedReceiptMapping, VerifiedUexReport,
    };
    use tallyline_workflows::api::{ManualReleaseStore, ReceiptEnrichmentStore};
    let path = std::env::temp_dir().join(format!(
        "tallyline-receipt-enrichment-{}.db",
        std::process::id()
    ));
    let (mut store, draft) = support::stored_gate(&path).unwrap();
    let reviewer = ManualReviewer::new(
        EnvironmentMapping::default(),
        StockSettings::default(),
        None,
    );
    let gate = reviewer.submission_gate(
        draft.source().clone(),
        draft.snapshot(),
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
    assert_eq!(
        store.claim_receipt_enrichment(release.job()),
        Err(SubmissionRepositoryError::Invalid)
    );
    let sending = release
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([8; 32]),
            date_added: Timestamp::from_seconds(900),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let accepted = sending
        .state()
        .apply(SubmissionCommand::Complete {
            at: Timestamp::from_seconds(1001),
            result: SubmissionResult::ProductionAccepted(vec![
                UexReportId::new(123, Timestamp::from_seconds(900)).unwrap(),
            ]),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &accepted).unwrap();
    assert!(store.claim_receipt_enrichment(accepted.state()).unwrap());
    drop(store);
    let mut store = SqliteReferenceCache::open(&path).unwrap();
    assert!(
        !store.claim_receipt_enrichment(accepted.state()).unwrap(),
        "An interrupted enrichment never starts another lookup on restart"
    );
    let request = accepted.state().request();
    let receipt = VerifiedUexReport::from_lookup_at(
        123,
        request.header().terminal,
        request.rows()[0].commodity(),
        Timestamp::from_seconds(900),
        request,
        Timestamp::from_seconds(900),
    )
    .unwrap();
    let mapping = VerifiedReceiptMapping::new(accepted.state(), vec![receipt]).unwrap();
    let database = rusqlite::Connection::open(&path).unwrap();
    database.execute_batch("CREATE TRIGGER reject_receipt BEFORE UPDATE ON submission_receipt_enrichment BEGIN SELECT RAISE(ABORT,'test receipt save'); END;").unwrap();
    assert_eq!(
        store.save_receipt_mapping(&mapping),
        Err(SubmissionRepositoryError::Unavailable)
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut store, accepted.state().id())
            .unwrap()
            .as_ref(),
        Some(accepted.state())
    );
    assert_eq!(store.receipt_mapping(accepted.state().id()).unwrap(), None);
    database
        .execute_batch("DROP TRIGGER reject_receipt")
        .unwrap();
    database.execute_batch("CREATE TRIGGER ignore_receipt BEFORE UPDATE ON submission_receipt_enrichment BEGIN SELECT RAISE(IGNORE); END;").unwrap();
    assert_eq!(
        store.save_receipt_mapping(&mapping),
        Err(SubmissionRepositoryError::Conflict),
        "A silently ignored proof write must never be published as committed"
    );
    assert_eq!(store.receipt_mapping(accepted.state().id()).unwrap(), None);
    database
        .execute_batch("DROP TRIGGER ignore_receipt")
        .unwrap();
    store.save_receipt_mapping(&mapping).unwrap();
    store.save_receipt_mapping(&mapping).unwrap();
    drop(database);
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    assert_eq!(
        reopened.receipt_mapping(accepted.state().id()).unwrap(),
        Some(mapping)
    );
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, accepted.state().id())
            .unwrap()
            .as_ref(),
        Some(accepted.state())
    );
    assert!(!reopened.claim_receipt_enrichment(accepted.state()).unwrap());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
