// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Account batches preserve exact row proofs through isolated HTTP and completion timing.

use super::*;
use tallyline_shared_kernel::api::Clock;
use tallyline_submission::api::SubmissionLifecycleReader;
use tallyline_submission::api::model::{
    JobId, LifecycleReadBatch, LifecycleRefresh, PayloadHash, RowEditability, SubmissionJob,
    SubmissionLifecycle, UexReportStatus, VerifiedUexReport,
};
use wiremock::matchers::query_param;

struct CompletionClock(std::cell::Cell<usize>);
impl Clock for CompletionClock {
    fn now(&self) -> Timestamp {
        self.0.set(self.0.get() + 1);
        Timestamp::from_seconds(125)
    }
}

fn admitted_batch() -> LifecycleReadBatch {
    let claims = [(1, 81, 89, 7), (2, 82, 90, 8)]
        .into_iter()
        .map(|(job_id, id, terminal, commodity)| {
            let base = released_request().unwrap();
            let mut header = base.header().clone();
            header.mode = SubmissionMode::Production;
            header.report = ReportId(job_id);
            header.terminal = TerminalId(terminal);
            let request = SubmissionRequest::new(
                header,
                vec![
                    SubmissionRow::new(
                        RowId(1),
                        CommodityId(commodity),
                        "120".parse().unwrap(),
                        "0".parse().unwrap(),
                        1,
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
            let proof = VerifiedUexReport::from_lookup_at(
                id,
                request.header().terminal,
                CommodityId(commodity),
                Timestamp::from_seconds(95),
                &request,
                Timestamp::from_seconds(95),
            )
            .unwrap();
            let job = SubmissionJob::queued(JobId(job_id), request)
                .unwrap()
                .begin_prepared(
                    Timestamp::from_seconds(110),
                    PayloadHash::new([1; 32]),
                    Timestamp::from_seconds(95),
                )
                .unwrap()
                .complete(
                    SubmissionResult::ProductionSucceeded(vec![proof]),
                    Timestamp::from_seconds(115),
                )
                .unwrap();
            SubmissionLifecycle::try_from_job(&job)
                .unwrap()
                .claim(LifecycleRefresh::Manual, Timestamp::from_seconds(120))
                .unwrap()
                .unwrap()
        })
        .collect();
    LifecycleReadBatch::try_new(claims).unwrap()
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_4_r_sub_9_r_api_7__one_username_batch_matches_exact_rows_across_terminals_and_seals_one_check_time()
 {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/data_info/"))
        .and(query_param("type", "commodity")).and(query_param("username", "dummy-a"))
        .and(query_param("limit", "100"))
        .and(header("authorization", "Bearer dummy-token")).and(header("secret-key", "dummy-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[
            {"id":82,"type":"commodity","id_terminal":90,"id_commodity":8,"date_added":95,"status":"declined","is_editable":"0"},
            {"id":81,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":94,"status":"expired","is_editable":0},
            {"id":81,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95,"status":"pending","is_editable":1}
        ]}))).expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let response = reader
        .read_lifecycle(admitted_batch(), &credentials, None)
        .await
        .unwrap();
    let clock = CompletionClock(std::cell::Cell::new(0));
    let completed = response.complete(&clock).unwrap();
    assert_eq!(clock.0.get(), 1);
    let observed = completed
        .claims()
        .iter()
        .map(|result| {
            assert_eq!(result.observations().len(), 1);
            let observation = &result.observations()[0];
            (
                result.claim().lifecycle().job(),
                observation.receipt().id(),
                observation.status(),
                observation.editability(),
                observation.checked_at(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            (
                JobId(1),
                81,
                UexReportStatus::Pending,
                RowEditability::Editable,
                Timestamp::from_seconds(125)
            ),
            (
                JobId(2),
                82,
                UexReportStatus::Declined,
                RowEditability::NotEditable,
                Timestamp::from_seconds(125)
            ),
        ]
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .url
            .query_pairs()
            .all(|(key, _)| key != "id_terminal" && key != "id")
    );
    server.verify().await;
}
async fn read_rows(
    data: serde_json::Value,
) -> Result<
    tallyline_submission::api::model::CompletedLifecycleRead,
    tallyline_submission::api::model::LifecycleReadFailure,
> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":data})))
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let response = reader
        .read_lifecycle(admitted_batch(), &values, None)
        .await?;
    let completed = response
        .complete(&CompletionClock(std::cell::Cell::new(0)))
        .unwrap();
    server.verify().await;
    Ok(completed)
}
fn known_row() -> serde_json::Value {
    json!({"id":81,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95,"status":"pending","is_editable":1})
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_4_r_sub_9__only_documented_statuses_and_exact_binary_editability_supply_positive_evidence()
 {
    use tallyline_submission::api::model::LifecycleReadFailure;
    for (wire, expected) in [
        ("pending", UexReportStatus::Pending),
        ("under_review", UexReportStatus::UnderReview),
        ("queued", UexReportStatus::Queued),
        ("approved", UexReportStatus::Approved),
        ("consolidated", UexReportStatus::Consolidated),
        ("declined", UexReportStatus::Declined),
        ("expired", UexReportStatus::Expired),
    ] {
        let mut row = known_row();
        row["status"] = json!(wire);
        let completed = read_rows(json!([row])).await.unwrap();
        assert_eq!(completed.claims()[0].observations()[0].status(), expected);
        assert!(completed.claims()[1].observations().is_empty());
    }
    for (field, invalid) in [
        ("status", json!("deleted")),
        ("status", json!("APPROVED")),
        ("status", json!(1)),
        ("status", serde_json::Value::Null),
        ("is_editable", json!(2)),
        ("is_editable", json!(true)),
        ("is_editable", json!("yes")),
        ("is_editable", serde_json::Value::Null),
    ] {
        let mut row = known_row();
        row[field] = invalid;
        assert_eq!(
            read_rows(json!([row])).await,
            Err(LifecycleReadFailure::InvalidResponse)
        );
    }
    for field in ["status", "is_editable"] {
        let mut row = known_row();
        row.as_object_mut().unwrap().remove(field);
        assert_eq!(
            read_rows(json!([row])).await,
            Err(LifecycleReadFailure::InvalidResponse)
        );
    }
    assert_eq!(
        read_rows(json!([known_row(), known_row()])).await,
        Err(LifecycleReadFailure::InvalidResponse)
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_4_r_sub_9__missing_full_windows_and_wrong_metadata_never_establish_negative_evidence()
 {
    for rows in [
        Vec::new(),
        (1000..1100)
            .map(|id| {
                let mut row = known_row();
                row["id"] = json!(id);
                row
            })
            .collect(),
    ] {
        let completed = read_rows(json!(rows)).await.unwrap();
        assert!(
            completed
                .claims()
                .iter()
                .all(|claim| claim.observations().is_empty())
        );
    }
    for (field, value) in [
        ("id", json!(999)),
        ("date_added", json!(94)),
        ("id_terminal", json!(90)),
        ("id_commodity", json!(8)),
    ] {
        let mut row = known_row();
        row[field] = value;
        assert!(
            read_rows(json!([row]))
                .await
                .unwrap()
                .claims()
                .iter()
                .all(|claim| claim.observations().is_empty())
        );
    }
    let mut full = (1000..1099)
        .map(|id| {
            let mut row = known_row();
            row["id"] = json!(id);
            row
        })
        .collect::<Vec<_>>();
    full.push(known_row());
    let completed = read_rows(json!(full)).await.unwrap();
    assert_eq!(completed.claims()[0].observations().len(), 1);
    assert!(completed.claims()[1].observations().is_empty());
    assert_eq!(
        read_rows(json!(
            (1000..1101)
                .map(|id| {
                    let mut row = known_row();
                    row["id"] = json!(id);
                    row
                })
                .collect::<Vec<_>>()
        ))
        .await,
        Err(tallyline_submission::api::model::LifecycleReadFailure::InvalidResponse)
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_4_r_sub_11_r_api_3__shared_and_newly_observed_quota_prevent_another_lifecycle_get() {
    use crate::api::AuthenticatedReadGate;
    use tallyline_submission::api::model::LifecycleReadFailure;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({"status":"exceeded_daily_quota","data":{"reset":1000}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let shared = UexReader::new(&settings)
        .unwrap()
        .with_authenticated_read_gate(AuthenticatedReadGate::new(|_| false));
    assert_eq!(
        shared.read_lifecycle(admitted_batch(), &values, None).await,
        Err(LifecycleReadFailure::Unavailable)
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    let reader = UexReader::new(&settings).unwrap();
    assert_eq!(
        reader.read_lifecycle(admitted_batch(), &values, None).await,
        Err(LifecycleReadFailure::Unavailable)
    );
    assert_eq!(
        reader.read_lifecycle(admitted_batch(), &values, None).await,
        Err(LifecycleReadFailure::Unavailable)
    );
    assert_eq!(reader.take_quota_observations().len(), 1);
    server.verify().await;
}
