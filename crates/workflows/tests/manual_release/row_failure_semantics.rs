// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Adapter-supplied row meaning survives real SQLite restart independently of spelling.

use super::{TestDirectory, support};
use rusqlite::Connection;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::SubmissionJobRepository;
use tallyline_submission::api::model::{
    PayloadHash, RowFailureKind, RowFailureSource, SubmissionCommand, SubmissionResult,
    SubmissionRowResult, UexErrorCode,
};
use tallyline_workflows::api::ManualReleaseStore;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_9_r_nf_11__explicit_duplicate_cause_and_cooldown_survive_reopening_without_wire_inference()
{
    let directory = TestDirectory::new().unwrap();
    let path = directory.0.join("semantic-code.db");
    let (mut store, gate) =
        support::stored_gate_with_mode(&path, SubmissionMode::Production).unwrap();
    let released = store.release_manual(&gate).unwrap();
    let sending = released
        .job()
        .apply(SubmissionCommand::Begin {
            at: Timestamp::from_seconds(1000),
            payload_hash: PayloadHash::new([9; 32]),
            date_added: Timestamp::from_seconds(895),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &sending).unwrap();
    let partial = sending
        .state()
        .apply(SubmissionCommand::Complete {
            result: SubmissionResult::PartialError {
                code: UexErrorCode::classified(
                    "adapter_duplicate".to_owned(),
                    RowFailureKind::Duplicate,
                )
                .unwrap(),
                row: Some(0),
                source: RowFailureSource::Index,
                receipts: vec![],
            },
            at: Timestamp::from_seconds(1001),
        })
        .unwrap();
    SubmissionJobRepository::save(&mut store, &partial).unwrap();
    drop(store);
    let mut reopened = SqliteReferenceCache::open(&path).unwrap();
    let restored = SubmissionJobRepository::load(&mut reopened, released.job().id())
        .unwrap()
        .unwrap();
    let row = restored.request().rows()[0].id();
    let result = restored.row_result(row).unwrap();
    let SubmissionRowResult::Rejected(code) = result else {
        panic!("the named row must retain its refusal");
    };
    assert_eq!(code.as_str(), "adapter_duplicate");
    assert_eq!(
        code.row_failure(),
        RowFailureKind::Duplicate,
        "restart must preserve the explicit cause independently of identifier spelling"
    );
    assert!(
        result.retains_cooldown(),
        "duplicate cooldown must survive restart"
    );
    assert_eq!(&restored, partial.state());
    let fault = Connection::open(&path).unwrap();
    let identity = i64::try_from(restored.id().0).unwrap();
    let original: String = fault
        .query_row(
            "SELECT payload FROM submission_jobs WHERE id=?1",
            [identity],
            |row| row.get(0),
        )
        .unwrap();
    for expression in [
        "json_set(?1,'$.result.partial_evidence[0].row_failure','future_cause')",
        "json_set(?1,'$.result.partial_evidence[0].unexpected',1)",
        "json_set(?1,'$.result.partial_evidence[0].value','')",
        "json_set(?1,'$.result.partial_evidence[0].value','unsafe code')",
        "json_remove(?1,'$.result.partial_evidence[0].row_failure')",
    ] {
        let query = format!("UPDATE submission_jobs SET payload={expression} WHERE id=?2");
        fault.execute(&query, (&original, identity)).unwrap();
        assert!(
            SubmissionJobRepository::load(&mut reopened, restored.id()).is_err(),
            "an invalid semantic code document must fail closed: {expression}"
        );
    }
    fault
        .execute(
            "UPDATE submission_jobs SET payload=?1 WHERE id=?2",
            (&original, identity),
        )
        .unwrap();
    assert_eq!(
        SubmissionJobRepository::load(&mut reopened, restored.id()).unwrap(),
        Some(restored)
    );
}
