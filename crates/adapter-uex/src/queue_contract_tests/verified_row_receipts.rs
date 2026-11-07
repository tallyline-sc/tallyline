// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Positive-only receipt association through isolated HTTP.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::Value;
use serde_json::json;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::SubmissionRowReceiptVerifier;
use tallyline_submission::api::model::VerifiedUexReport;
use tallyline_submission::api::model::{
    CredentialValues, JobPhase, SubmissionRequest, SubmissionResult,
};
use wiremock::matchers;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__unknown_receipt_read_uses_exact_attempted_date_and_never_posts() {
    let server = MockServer::start().await;
    Mock::given(matchers::method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/2.0/data_info/"))
        .and(matchers::query_param("type", "commodity"))
        .and(matchers::query_param("id_terminal", "89"))
        .and(matchers::query_param("username", "dummy-a"))
        .and(matchers::query_param("limit", "100"))
        .and(matchers::header("authorization", "Bearer dummy-token"))
        .and(matchers::header("secret-key", "dummy-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[
                {"id":81,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":100},
                {"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}
            ]})),
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
    let reader = UexReader::new(&settings).unwrap();
    let base = super::released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let job = super::accepted_job(
        &request,
        SubmissionResult::Unknown,
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let receipts = reader.verify_row_receipts(&job, &credentials, None).await;
    assert!(
        receipts.is_some(),
        "one positive exact-date lookup must be retained: {receipts:?}"
    );
    let receipts = receipts.unwrap();
    assert_eq!(
        receipts
            .iter()
            .map(|receipt| (
                receipt.row().0,
                receipt.id(),
                receipt.date_added().seconds()
            ))
            .collect::<Vec<_>>(),
        vec![(1, 82, 95)]
    );
    assert_eq!(job.phase(), JobPhase::OutcomeUnknown);
    assert_eq!(job.result(), Some(&SubmissionResult::Unknown));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method.as_str(), "GET");
    assert!(requests[0].url.query_pairs().all(|(key, _)| key != "id"));
    server.verify().await;
}

async fn read_window(data: Value) -> Option<Vec<VerifiedUexReport>> {
    read_result_window(data, SubmissionResult::Unknown).await
}
async fn read_result_window(
    data: Value,
    result: SubmissionResult,
) -> Option<Vec<VerifiedUexReport>> {
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/2.0/data_info/"))
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
    let base = super::released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let job = super::accepted_job(&request, result, Timestamp::from_seconds(95)).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader.verify_row_receipts(&job, &credentials, None).await;
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    server.verify().await;
    result
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__ambiguous_unknown_receipt_window_never_selects_first_matching_identity() {
    let duplicated =
        json!({"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95});
    assert!(
        read_window(json!([duplicated.clone(), duplicated]))
            .await
            .is_none(),
        "repeated matching records are ambiguous, not two accepted rows"
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__malformed_unknown_window_cannot_supply_receipt_authority() {
    let data = json!([
        {"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95},
        {"id":83,"type":"commodity","id_terminal":89,"id_commodity":8}
    ]);
    assert!(
        read_window(data).await.is_none(),
        "malformed windows remain unavailable even beside a matching record"
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__empty_full_and_oversized_unknown_windows_never_prove_non_receipt() {
    let empty = read_window(json!([])).await.unwrap();
    assert!(empty.is_empty());
    let records: Vec<_> = (1..=100).map(|id| json!({"id":id,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":94})).collect();
    assert!(read_window(json!(records)).await.unwrap().is_empty());
    let mut positive = records.clone();
    positive[99] =
        json!({"id":100,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95});
    let proof = read_window(json!(positive)).await.unwrap();
    assert_eq!(
        proof.iter().map(|receipt| receipt.id()).collect::<Vec<_>>(),
        vec![100]
    );
    let mut oversized = records;
    oversized.push(
        json!({"id":101,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}),
    );
    assert!(read_window(json!(oversized)).await.is_none());
    assert!(
        read_window(json!([
            {"id":1,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95},
            {"id":2,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}
        ]))
        .await
        .is_none()
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__partial_receipt_read_maps_reordered_rows_and_retains_unresolved_rows() {
    use tallyline_shared_kernel::api::model::{CommodityId, RowId, UexReportId};
    use tallyline_submission::api::model::SubmissionRow;
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET"))
        .and(matchers::path("/2.0/data_info/"))
        .and(matchers::query_param("username", "dummy-a"))
        .and(matchers::query_param("id_terminal", "89"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[
                {"id":82,"type":"commodity","id_terminal":89,"id_commodity":9,"date_added":95},
                {"id":81,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}
            ]})),
        )
        .expect(2)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let base = super::released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let rows = [(31, 7), (42, 8), (53, 9)]
        .into_iter()
        .map(|(row, commodity)| {
            SubmissionRow::new(
                RowId(row),
                CommodityId(commodity),
                "120".parse().unwrap(),
                "0".parse().unwrap(),
                1,
            )
            .unwrap()
        })
        .collect();
    let request = SubmissionRequest::new(header, rows).unwrap();
    let id = UexReportId::new(82, Timestamp::from_seconds(95)).unwrap();
    let job = super::accepted_job(
        &request,
        SubmissionResult::Partial(vec![id]),
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let receipts = reader.verify_row_receipts(&job, &credentials, None).await;
    assert!(
        receipts.is_some(),
        "partial production must retain individual positive row proofs: {receipts:?}"
    );
    let receipts = receipts.unwrap();
    assert_eq!(
        receipts
            .iter()
            .map(|proof| (proof.row().0, proof.id(), proof.date_added().seconds()))
            .collect::<Vec<_>>(),
        vec![(53, 82, 95), (31, 81, 95)]
    );
    let resolved = job
        .resolve_delivery(
            tallyline_submission::api::model::SubmissionResolutionAction::Verified {
                receipts,
                at: Timestamp::from_seconds(130),
            },
        )
        .unwrap();
    assert!(
        matches!(
            resolved.row_result(RowId(42)),
            Some(tallyline_submission::api::model::SubmissionRowResult::Unmapped)
        ),
        "unmatched rows cannot become not accepted by absence"
    );
    assert_eq!(job.phase(), JobPhase::PartiallyAccepted);
    assert_eq!(job.attempts().len(), 1);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request
                .url
                .query_pairs()
                .any(|(key, value)| key == "id" && value == "82"))
            .count(),
        1,
        "each surviving returned ID must receive its own identity-filtered lookup"
    );
    server.verify().await;
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__partial_id_lookup_cannot_hide_a_conflicting_identity_in_the_window() {
    use tallyline_shared_kernel::api::model::UexReportId;
    let server = MockServer::start().await;
    let first =
        json!({"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95});
    Mock::given(matchers::method("GET"))
        .and(matchers::query_param("id", "82"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[first.clone()]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(matchers::method("GET")).and(matchers::query_param_is_missing("id")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[first,{"id":83,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}]}))).expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let base = super::released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let job = super::accepted_job(
        &request,
        SubmissionResult::Partial(vec![
            UexReportId::new(82, Timestamp::from_seconds(95)).unwrap(),
        ]),
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert!(
        reader
            .verify_row_receipts(&job, &credentials, None)
            .await
            .is_none(),
        "an individually matched ID cannot choose between contradictory same-row identities from the account window"
    );
    server.verify().await;
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__full_hundred_record_window_retains_positive_partial_proof_without_negative_inference()
 {
    let mut records:Vec<Value>=(1000..1099).map(|id|json!({"id":id,"type":"commodity","id_terminal":89,"id_commodity":8,"date_added":95})).collect();
    records.push(
        json!({"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}),
    );
    let proofs = read_result_window(Value::Array(records), SubmissionResult::Partial(vec![]))
        .await
        .unwrap();
    assert_eq!(
        proofs
            .iter()
            .map(|proof| (proof.row().0, proof.id(), proof.date_added().seconds()))
            .collect::<Vec<_>>(),
        vec![(1, 82, 95)]
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__mismatched_response_date_keeps_partial_history_but_still_checks_each_candidate_id()
 {
    use tallyline_shared_kernel::api::model::UexReportId;
    let server = MockServer::start().await;
    Mock::given(matchers::method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}]}))).expect(2).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let base = super::released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let response_id = UexReportId::new(82, Timestamp::from_seconds(100)).unwrap();
    let job = super::accepted_job(
        &request,
        SubmissionResult::Partial(vec![response_id]),
        Timestamp::from_seconds(95),
    )
    .unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let proofs = reader
        .verify_row_receipts(&job, &credentials, None)
        .await
        .unwrap();
    assert_eq!(proofs[0].date_added(), Timestamp::from_seconds(95));
    assert_eq!(
        job.result(),
        Some(&SubmissionResult::Partial(vec![response_id]))
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request
                .url
                .query_pairs()
                .any(|(key, value)| key == "id" && value == "82"))
            .count(),
        1,
        "every positive returned candidate must be checked, even when its original date is inconsistent"
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__ambiguous_individual_partial_probe_cannot_be_overridden_by_prior_or_later_proof()
{
    use tallyline_shared_kernel::api::model::{CommodityId, RowId, UexReportId};
    use tallyline_submission::api::model::SubmissionRow;
    let mut accepted_cases = Vec::new();
    for prior in [false, true] {
        for repeated in [false, true] {
            let server = MockServer::start().await;
            let first = json!({"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95});
            let second = if repeated {
                first.clone()
            } else {
                json!({"id":83,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95})
            };
            Mock::given(matchers::method("GET"))
                .and(matchers::path("/2.0/data_info/"))
                .and(matchers::query_param("id", "82"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"status":"ok","data":[first.clone(),second]})),
                )
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(matchers::method("GET"))
                .and(matchers::path("/2.0/data_info/"))
                .and(matchers::query_param_is_missing("id"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[first]})),
                )
                .expect(0..=1)
                .mount(&server)
                .await;
            if prior {
                Mock::given(matchers::method("GET"))
                    .and(matchers::path("/2.0/data_info/"))
                    .and(matchers::query_param("id", "81"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":81,"type":"commodity","id_terminal":89,"id_commodity":8,"date_added":95}]})))
                    .expect(1).mount(&server).await;
            }
            let settings = ReaderSettings::for_test_server(
                &format!("{}/2.0/", server.uri()),
                ClientIdentity::new("tallyline-test", "1.2.3"),
            )
            .unwrap()
            .with_expert_host_confirmation();
            let reader = UexReader::new(&settings).unwrap();
            let base = super::released_request().unwrap();
            let mut header = base.header().clone();
            header.mode = SubmissionMode::Production;
            let rows = [(31, 7), (42, 8)]
                .into_iter()
                .map(|(row, commodity)| {
                    SubmissionRow::new(
                        RowId(row),
                        CommodityId(commodity),
                        "120".parse().unwrap(),
                        "0".parse().unwrap(),
                        1,
                    )
                    .unwrap()
                })
                .collect();
            let request = SubmissionRequest::new(header, rows).unwrap();
            let mut ids = Vec::new();
            if prior {
                ids.push(UexReportId::new(81, Timestamp::from_seconds(95)).unwrap());
            }
            ids.push(UexReportId::new(82, Timestamp::from_seconds(95)).unwrap());
            let job = super::accepted_job(
                &request,
                SubmissionResult::Partial(ids),
                Timestamp::from_seconds(95),
            )
            .unwrap();
            let credentials =
                CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))
                    .unwrap();
            let evidence = reader.verify_row_receipts(&job, &credentials, None).await;
            server.verify().await;
            assert!(
                server
                    .received_requests()
                    .await
                    .unwrap()
                    .iter()
                    .all(|request| request.method == "GET")
            );
            if evidence.is_some() {
                accepted_cases.push((prior, repeated));
            }
        }
    }
    assert!(
        accepted_cases.is_empty(),
        "an ambiguous individual probe must fail the entire proof read despite earlier or later clean evidence; accepted (prior,repeated) cases: {accepted_cases:?}"
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__unavailable_or_nonmatching_individual_partial_probe_allows_independent_general_proof()
 {
    use tallyline_shared_kernel::api::model::UexReportId;
    for individual in [ResponseTemplate::new(503),ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[]})),ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":94}]}))] {
        let server = MockServer::start().await;
        Mock::given(matchers::method("GET")).and(matchers::path("/2.0/data_info/")).and(matchers::query_param("id","82")).respond_with(individual).expect(1).mount(&server).await;
        Mock::given(matchers::method("GET")).and(matchers::path("/2.0/data_info/")).and(matchers::query_param_is_missing("id")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":82,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}]}))).expect(1).mount(&server).await;
        let settings = ReaderSettings::for_test_server(&format!("{}/2.0/", server.uri()),ClientIdentity::new("tallyline-test", "1.2.3")).unwrap().with_expert_host_confirmation();
        let reader = UexReader::new(&settings).unwrap();
        let base = super::released_request().unwrap();
        let mut header = base.header().clone();header.mode = SubmissionMode::Production;
        let request = SubmissionRequest::new(header,base.rows().to_vec()).unwrap();
        let job = super::accepted_job(&request,SubmissionResult::Partial(vec![UexReportId::new(82,Timestamp::from_seconds(95)).unwrap()]),Timestamp::from_seconds(95)).unwrap();
        let credentials = CredentialValues::new("dummy-key".to_owned(),Some("dummy-token".to_owned())).unwrap();
        let evidence = reader.verify_row_receipts(&job,&credentials,None).await.unwrap();
        assert_eq!(evidence.len(),1);assert_eq!(evidence[0].id(),82);assert_eq!(evidence[0].date_added(),Timestamp::from_seconds(95));
        server.verify().await;
        assert!(server.received_requests().await.unwrap().iter().all(|request| request.method == "GET"));
    }
}
