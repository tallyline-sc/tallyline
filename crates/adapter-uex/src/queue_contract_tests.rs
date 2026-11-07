// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Queue gateway contracts using an isolated local HTTP server.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::json;
use tallyline_shared_kernel::api::model::{
    CommodityId, ReportId, RowId, SubmissionMode, TerminalId, Timestamp, TradeSide, UexEnvironment,
};
use tallyline_submission::api::model::{
    CredentialValues, JobPhase, SubmissionCorrection, SubmissionHeader, SubmissionPauseReason,
    SubmissionRequest, SubmissionResult, SubmissionRetrySettings, SubmissionRow,
};
use tallyline_submission::api::{SubmissionGateway, SubmissionReceiptVerifier};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_1_r_sub_8_r_api_7__queue_dry_run_sends_exact_one_sided_values_and_omits_zero_scu() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/2.0/data_submit/"))
        .and(header("user-agent", "tallyline-test/1.2.3"))
        .and(header("x-client-version", "1.2.3"))
        .and(header("secret-key", "dummy-key"))
        .and(header("authorization", "Bearer dummy-token"))
        .and(body_json(json!({"id_terminal":89,"type":"commodity","is_production":0,"game_version":"4.10.2","date_added":100,"prices":[{"id_commodity":7,"price_sell":120.125,"status_sell":1}]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"ids_reports":null,"date_added":100}})))
        .expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let request = SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(1),
            report_version: 3,
            account: "dummy-a".to_owned(),
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
                RowId(1),
                CommodityId(7),
                "120.125".parse().unwrap(),
                "0".parse().unwrap(),
                1,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert_eq!(
        reader
            .submit(reader.prepare(&request).unwrap().0, &credentials, None)
            .await,
        SubmissionResult::DryRunSucceeded
    );
    server.verify().await;
}

pub(super) fn released_request() -> Option<SubmissionRequest> {
    SubmissionRequest::new(
        SubmissionHeader {
            report: ReportId(1),
            report_version: 3,
            account: "dummy-a".to_owned(),
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
                RowId(1),
                CommodityId(7),
                "120.125".parse().ok()?,
                "0".parse().ok()?,
                1,
            )
            .ok()?,
        ],
    )
    .ok()
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__refused_connection_is_not_processed_but_bad_response_stays_unknown() {
    use tallyline_submission::api::model::NotProcessed;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let settings = ReaderSettings::for_test_server(
        &format!("http://{address}/2.0/"),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert_eq!(
        reader
            .submit(
                reader.prepare(&released_request().unwrap()).unwrap().0,
                &credentials,
                None
            )
            .await,
        SubmissionResult::NotProcessed(NotProcessed::ConnectFailure)
    );
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("broken reply"))
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
    assert_eq!(
        reader
            .submit(
                reader.prepare(&released_request().unwrap()).unwrap().0,
                &credentials,
                None
            )
            .await,
        SubmissionResult::Unknown
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_man_5_r_sub_9__dry_run_screenshot_requirement_survives_transport_translation() {
    use tallyline_submission::api::model::SubmissionReturnReason;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"status":"screenshot_required","data":null})),
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
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert_eq!(
        reader
            .submit(
                reader.prepare(&released_request().unwrap()).unwrap().0,
                &credentials,
                None
            )
            .await,
        SubmissionResult::Returned(SubmissionReturnReason::ScreenshotRequired)
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__production_ids_are_verified_by_identity_and_never_mapped_by_position() {
    use wiremock::matchers::query_param;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"status":"ok","data":{"ids_reports":["11","12"],"date_added":100}}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    for (id, commodity) in [(11, 8), (12, 7)] {
        Mock::given(method("GET")).and(path("/2.0/data_info/")).and(query_param("id",id.to_string())).and(query_param("username","dummy-a"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":id,"type":"commodity","id_terminal":89,"id_commodity":commodity,"date_added":100}]})))
            .expect(1).mount(&server).await;
    }
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let base = released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let mut rows = base.rows().to_vec();
    rows.push(
        SubmissionRow::new(
            RowId(2),
            CommodityId(8),
            "2".parse().unwrap(),
            "10".parse().unwrap(),
            2,
        )
        .unwrap(),
    );
    let request = SubmissionRequest::new(header, rows).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await;
    let accepted = accepted_job(&request, result, Timestamp::from_seconds(100)).unwrap();
    let receipts = reader
        .verify_receipts(&accepted, &credentials, None)
        .await
        .unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].row(), RowId(2));
    assert_eq!(receipts[0].id(), 11);
    assert_eq!(receipts[1].row(), RowId(1));
    assert_eq!(receipts[1].id(), 12);
    assert_eq!(receipts[0].date_added(), Timestamp::from_seconds(100));
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__partial_ids_remain_unmapped_with_their_returned_partition_time() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"status":"ok","data":{"ids_reports":["11"],"date_added":100}}),
            ),
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
    let base = released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let mut rows = base.rows().to_vec();
    rows.push(
        SubmissionRow::new(
            RowId(2),
            CommodityId(8),
            "2".parse().unwrap(),
            "10".parse().unwrap(),
            2,
        )
        .unwrap(),
    );
    let request = SubmissionRequest::new(header, rows).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let SubmissionResult::Partial(unverified) = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await
    else {
        panic!("partial receipt count must fail closed");
    };
    assert_eq!(unverified.len(), 1);
    assert_eq!(unverified[0].id(), 11);
    assert_eq!(unverified[0].date_added(), Timestamp::from_seconds(100));
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.method.as_str() == "POST")
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2__payload_is_serialised_and_hashed_before_the_async_attempt_begins() {
    let settings = ReaderSettings::new(
        "https://api.uexcorp.uk/2.0/",
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap();
    let reader = UexReader::new(&settings).unwrap();
    let (_, hash) = reader.prepare(&released_request().unwrap()).unwrap();
    let actual: String = hash
        .bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        actual,
        "05f8a6292d79e9a33b014a01c7d13059aac8d124a238f9d7e59ee0bfbcac9e0c"
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_8_r_sub_9__malformed_or_positive_dry_run_receipts_cannot_be_success() {
    for data in [
        json!({"ids_reports":["0"],"date_added":"broken"}),
        json!({"ids_reports":["42"],"date_added":100}),
        json!({"ids_reports":["+0"],"date_added":100}),
        json!({"ids_reports":["0","0"],"date_added":100}),
        json!({"ids_reports":["0"],"date_added":101}),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":data})),
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
        let request = released_request().unwrap();
        let (prepared, _) = reader.prepare(&request).unwrap();
        let values =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        assert_eq!(
            reader.submit(prepared, &values, None).await,
            SubmissionResult::Unknown
        );
        server.verify().await;
    }
}

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
struct ServerClock {
    tick: AtomicU64,
}
impl tallyline_shared_kernel::api::Clock for ServerClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(784111900)
    }
}
impl tallyline_submission::api::SubmissionClock for ServerClock {
    fn monotonic(&self) -> Duration {
        Duration::from_millis(self.tick.load(Ordering::SeqCst))
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_val_6__every_supported_http_date_format_clamps_the_prepared_wire_time_with_monotonic_expiry()
 {
    use crate::api::model::ServerTimeTracker;
    for date in [
        "Sun, 06 Nov 1994 08:49:37 GMT",
        "Sunday, 06-Nov-94 08:49:37 GMT",
        "Sun Nov  6 08:49:37 1994",
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/2.0/game_versions/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("date", date)
                    .set_body_json(json!({"status":"ok","data":{"live":"4.10.2","ptu":null}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST")).and(body_json(json!({"id_terminal":89,"type":"commodity","is_production":0,"game_version":"4.10.2","date_added":784111779,"prices":[{"id_commodity":7,"price_sell":120.125,"status_sell":1}]}))).respond_with(ResponseTemplate::new(200).insert_header("date",date).set_body_json(json!({"status":"ok","data":{"ids_reports":["0"],"date_added":784111779}}))).expect(1).mount(&server).await;
        let clock = Arc::new(ServerClock {
            tick: AtomicU64::new(50000),
        });
        let settings = ReaderSettings::for_test_server(
            &format!("{}/2.0/", server.uri()),
            ClientIdentity::new("tallyline-test", "1.2.3"),
        )
        .unwrap()
        .with_expert_host_confirmation();
        let reader =
            UexReader::new_with_server_time(&settings, ServerTimeTracker::new(clock.clone()))
                .unwrap();
        reader.game_versions().await.unwrap();
        clock.tick.store(52250, Ordering::SeqCst);
        let base = released_request().unwrap();
        let mut header = base.header().clone();
        header.observed_at = Timestamp::from_seconds(784111900);
        let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
        let (prepared, _) = reader.prepare(&request).unwrap();
        assert_eq!(prepared.date_added(), Timestamp::from_seconds(784111779));
        let values =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        assert_eq!(
            reader.submit(prepared, &values, None).await,
            SubmissionResult::DryRunSucceeded
        );
        clock.tick.store(1000001, Ordering::SeqCst);
        assert_eq!(
            reader.prepare(&request).unwrap().0.date_added(),
            Timestamp::from_seconds(784111900)
        );
        assert_eq!(
            request.header().observed_at,
            Timestamp::from_seconds(784111900)
        );
        server.verify().await;
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_val_6_r_sub_9__production_lookup_matches_the_actual_clamped_wire_date() {
    use crate::api::model::ServerTimeTracker;
    use wiremock::matchers::query_param;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/game_versions/"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("date", "Thu, 01 Jan 1970 00:01:35 GMT")
                .set_body_json(json!({"status":"ok","data":{"live":"4.10.2","ptu":null}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST")).and(body_json(json!({"id_terminal":89,"type":"commodity","is_production":1,"game_version":"4.10.2","date_added":95,"prices":[{"id_commodity":7,"price_sell":120.125,"status_sell":1}]}))).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"ids_reports":["42"],"date_added":95}}))).expect(1).mount(&server).await;
    Mock::given(method("GET")).and(path("/2.0/data_info/")).and(query_param("id","42")).and(query_param("username","dummy-a")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":42,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":95}]}))).expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new_with_server_time(
        &settings,
        ServerTimeTracker::new(Arc::new(ServerClock {
            tick: AtomicU64::new(50000),
        })),
    )
    .unwrap();
    reader.game_versions().await.unwrap();
    let base = released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let (prepared, _) = reader.prepare(&request).unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader.submit(prepared, &values, None).await;
    let accepted = accepted_job(&request, result, Timestamp::from_seconds(95)).unwrap();
    let receipts = reader
        .verify_receipts(&accepted, &values, None)
        .await
        .unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].id(), 42);
    assert_eq!(receipts[0].date_added(), Timestamp::from_seconds(95));
    assert_eq!(request.header().observed_at, Timestamp::from_seconds(100));
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__full_count_success_survives_unavailable_identity_lookup_without_positional_mapping()
 {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"status":"ok","data":{"ids_reports":["42"],"date_added":100}}),
            ),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
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
    let base = released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await;
    let accepted = accepted_job(&request, result.clone(), Timestamp::from_seconds(100)).unwrap();
    assert!(
        reader
            .verify_receipts(&accepted, &credentials, None)
            .await
            .is_none()
    );
    assert_eq!(
        accepted.recover(Timestamp::from_seconds(120)).unwrap(),
        accepted
    );
    let SubmissionResult::ProductionAccepted(ids) = result else {
        panic!("full-count acceptance must remain successful while verification is unavailable");
    };
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].id(), 42);
    assert_eq!(ids[0].date_added(), Timestamp::from_seconds(100));
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_5_r_sub_8__legacy_single_numeric_string_and_null_test_receipts_remain_supported() {
    for ids in [json!("0"), serde_json::Value::Null] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"status":"ok","data":{"ids_reports":ids,"date_added":"100"}}),
            ))
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
        let request = released_request().unwrap();
        let credentials =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        assert_eq!(
            reader
                .submit(reader.prepare(&request).unwrap().0, &credentials, None)
                .await,
            SubmissionResult::DryRunSucceeded
        );
        server.verify().await;
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_7_r_nf_4__prepared_screenshot_is_bounded_plain_base64_and_part_of_the_exact_attempt_digest()
 {
    use crate::api::model::PreparedUexSubmission;
    use tallyline_submission::api::model::PreparationFailure;
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(body_json(json!({"id_terminal":89,"type":"commodity","is_production":0,"game_version":"4.10.2","date_added":100,"prices":[{"id_commodity":7,"price_sell":120.125,"status_sell":1}],"screenshot":"AP8="}))).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"ids_reports":null,"date_added":100}}))).expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let request = released_request().unwrap();
    let (_, no_image) = reader.prepare(&request).unwrap();
    let (prepared, image_hash) = PreparedUexSubmission::new_with_screenshot(
        &request,
        Timestamp::from_seconds(100),
        Some("AP8="),
    )
    .unwrap();
    assert_ne!(image_hash, no_image);
    for invalid in [
        "",
        "data:image/png;base64,AP8=",
        "AP8=\n",
        "A===",
        "AP9=",
        "AP8",
    ] {
        assert!(matches!(
            PreparedUexSubmission::new_with_screenshot(
                &request,
                Timestamp::from_seconds(100),
                Some(invalid)
            ),
            Err(PreparationFailure::InvalidPayload)
        ));
    }
    let limit = "A".repeat(10_000_000);
    assert!(
        PreparedUexSubmission::new_with_screenshot(
            &request,
            Timestamp::from_seconds(100),
            Some(&limit)
        )
        .is_ok()
    );
    let excessive = "A".repeat(10_000_004);
    assert!(
        PreparedUexSubmission::new_with_screenshot(
            &request,
            Timestamp::from_seconds(100),
            Some(&excessive)
        )
        .is_err()
    );
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert_eq!(
        reader.submit(prepared, &credentials, None).await,
        SubmissionResult::DryRunSucceeded
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_val_6_r_api_5__public_metadata_observes_server_time_before_payload_or_transport_refusal()
{
    use crate::api::model::{PublicContractEndpoint, ServerTimeTracker};
    for status in [200, 302, 503] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/2.0/game_versions/"))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("date", "Sun, 06 Nov 1994 08:49:37 GMT")
                    .set_body_string("invalid private response"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let tracker = ServerTimeTracker::new(Arc::new(ServerClock {
            tick: AtomicU64::new(0),
        }));
        let settings = ReaderSettings::for_test_server(
            &format!("{}/2.0/", server.uri()),
            ClientIdentity::new("tallyline-test", "1.2.3"),
        )
        .unwrap();
        let reader = UexReader::new_with_server_time(&settings, tracker.clone()).unwrap();
        match reader
            .public_contract_sample(PublicContractEndpoint::GameVersions)
            .await
        {
            Ok(payload) => assert!(payload.into_schema().is_err()),
            Err(_) => assert_eq!(status, 302),
        }
        assert_eq!(
            tracker.clamp(Timestamp::from_seconds(784111900)),
            Timestamp::from_seconds(784111777)
        );
        server.verify().await;
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_val_6_r_sub_11__invalid_date_returns_for_review_without_changing_the_observation() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/data_submit/"))
        .respond_with(ResponseTemplate::new(400).set_body_json(
            json!({"status":"invalid_date","message":"Check observation time","data":null}),
        ))
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
    let request = released_request().unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await;
    assert!(matches!(result, SubmissionResult::Returned(_)));
    assert_eq!(request.header().observed_at, Timestamp::from_seconds(100));
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_9__full_count_acceptance_returns_before_any_identity_lookup() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/data_submit/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"status":"ok","data":{"ids_reports":["42"],"date_added":100}}),
            ),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .expect(0)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let base = released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await;
    assert!(matches!(result, SubmissionResult::ProductionAccepted(_)));
    server.verify().await;
}

fn accepted_job(
    request: &SubmissionRequest,
    result: SubmissionResult,
    date_added: Timestamp,
) -> Option<tallyline_submission::api::model::SubmissionJob> {
    use tallyline_submission::api::model::{JobId, PayloadHash, SubmissionJob};
    SubmissionJob::queued(JobId(1), request.clone())
        .ok()?
        .begin_prepared(
            Timestamp::from_seconds(110),
            PayloadHash::new([1; 32]),
            date_added,
        )
        .ok()?
        .complete(result, Timestamp::from_seconds(115))
        .ok()
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_1_r_man_2__zero_scu_above_status_one_is_omitted_without_rounding_the_entered_price()
{
    let server = MockServer::start().await;
    let expected: serde_json::Value = serde_json::from_str(r#"{"id_terminal":89,"type":"commodity","is_production":0,"game_version":"4.10.2","date_added":100,"prices":[{"id_commodity":7,"price_sell":12.123456789012345678901234567,"status_sell":3}]}"#).unwrap();
    Mock::given(method("POST"))
        .and(path("/2.0/data_submit/"))
        .and(body_json(expected))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status":"ok","data":{"ids_reports":null,"date_added":100}})),
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
    let base = released_request().unwrap();
    let request = SubmissionRequest::new(
        base.header().clone(),
        vec![
            SubmissionRow::new(
                RowId(1),
                CommodityId(7),
                "12.123456789012345678901234567".parse().unwrap(),
                "0".parse().unwrap(),
                3,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert_eq!(
        reader
            .submit(reader.prepare(&request).unwrap().0, &values, None)
            .await,
        SubmissionResult::DryRunSucceeded
    );
    let requests = server.received_requests().await.unwrap();
    let body = std::str::from_utf8(&requests[0].body).unwrap();
    assert!(body.contains("12.123456789012345678901234567"));
    assert!(!body.contains("scu_sell"));
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9_r_sub_11__verified_pre_storage_service_failure_is_held_without_replaying() {
    use tallyline_submission::api::model::{JobId, JobPhase};
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"status":"service_unavailable","data":null})),
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
    let request = released_request().unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let (prepared, hash) = reader.prepare(&request).unwrap();
    let sending = tallyline_submission::api::model::SubmissionJob::queued(JobId(1), request)
        .unwrap()
        .begin(Timestamp::from_seconds(101), hash)
        .unwrap();
    let result = reader.submit(prepared, &credentials, None).await;
    let completed = sending
        .complete(result, Timestamp::from_seconds(102))
        .unwrap();
    assert_eq!(completed.phase(), JobPhase::Held);
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_6_r_sub_11__invalid_token_pauses_instead_of_losing_the_action() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({"status":"access_denied","data":null})),
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
    let request = released_request().unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let (prepared, hash) = reader.prepare(&request).unwrap();
    let sending = tallyline_submission::api::model::SubmissionJob::queued(
        tallyline_submission::api::model::JobId(1),
        request,
    )
    .unwrap()
    .begin(Timestamp::from_seconds(101), hash)
    .unwrap();
    let completed = sending
        .complete(
            reader.submit(prepared, &credentials, None).await,
            Timestamp::from_seconds(102),
        )
        .unwrap();
    assert_eq!(completed.phase(), JobPhase::Paused);
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_11__acceptance_closed_and_removed_endpoint_are_held_without_replaying() {
    for response in [
        ResponseTemplate::new(400)
            .set_body_json(json!({"status":"ptu_reports_not_allowed","data":null})),
        ResponseTemplate::new(404).set_body_string("<!doctype html><title>Not found</title>"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(response)
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
        let request = released_request().unwrap();
        let credentials =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        let (prepared, hash) = reader.prepare(&request).unwrap();
        let sending = tallyline_submission::api::model::SubmissionJob::queued(
            tallyline_submission::api::model::JobId(1),
            request,
        )
        .unwrap()
        .begin(Timestamp::from_seconds(101), hash)
        .unwrap();
        let completed = sending
            .complete(
                reader.submit(prepared, &credentials, None).await,
                Timestamp::from_seconds(102),
            )
            .unwrap();
        assert_eq!(
            completed.phase(),
            tallyline_submission::api::model::JobPhase::Held
        );
        server.verify().await;
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_11__spurious_key_error_checks_only_user_and_preserves_a_retry_action() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"status":"invalid_secret_key","data":null})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path("/2.0/user/")).and(header("secret-key", "dummy-key")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"username":"dummy-a","is_datarunner":1,"is_datarunner_banned":0,"is_staff":0}}))).expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let request = released_request().unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await;
    assert_eq!(
        result.final_result(),
        &SubmissionResult::Paused(SubmissionPauseReason::KeyCheckUnavailable)
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    let (_, hash) = reader.prepare(&request).unwrap();
    let job = tallyline_submission::api::model::SubmissionJob::queued(
        tallyline_submission::api::model::JobId(1),
        request,
    )
    .unwrap()
    .begin(Timestamp::from_seconds(101), hash)
    .unwrap()
    .complete(result, Timestamp::from_seconds(102))
    .unwrap();
    let result = reader
        .recheck_submission_refusal(&job, &credentials, None)
        .await
        .unwrap();
    assert_eq!(
        result.final_result(),
        &SubmissionResult::Paused(SubmissionPauseReason::KeyChecked)
    );
    assert!(!job.attempts()[0].receipt_checked());
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        !requests
            .iter()
            .find(|request| request.method == "GET")
            .unwrap()
            .headers
            .contains_key("authorization")
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_6_r_sub_11__report_fixable_refusals_return_and_permanent_codes_remain_visible() {
    for (code, expected) in [
        ("max_rows_exceeded", JobPhase::Returned),
        ("terminal_not_found", JobPhase::Returned),
        ("invalid_screenshot", JobPhase::Returned),
        ("screenshot_length_exceeds_limit", JobPhase::Returned),
        ("missing_client_version", JobPhase::Rejected),
        ("invalid_game_version", JobPhase::Rejected),
        ("dummy_unknown_error", JobPhase::Rejected),
        ("Future_Code", JobPhase::Rejected),
        ("future-code", JobPhase::Rejected),
        ("", JobPhase::OutcomeUnknown),
        ("future.code", JobPhase::OutcomeUnknown),
        ("future\ncode", JobPhase::OutcomeUnknown),
        ("\u{e9}tat", JobPhase::OutcomeUnknown),
        ("dummy-key", JobPhase::OutcomeUnknown),
        ("dummy-token", JobPhase::OutcomeUnknown),
        (
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            JobPhase::Rejected,
        ),
        (
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            JobPhase::OutcomeUnknown,
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({"status":code,"data":null})),
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
        let request = released_request().unwrap();
        let credentials =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        let (prepared, hash) = reader.prepare(&request).unwrap();
        let sending = tallyline_submission::api::model::SubmissionJob::queued(
            tallyline_submission::api::model::JobId(1),
            request,
        )
        .unwrap()
        .begin(Timestamp::from_seconds(101), hash)
        .unwrap();
        let result = reader.submit(prepared, &credentials, None).await;
        let completed = sending
            .complete(result, Timestamp::from_seconds(102))
            .unwrap();
        assert_eq!(completed.phase(), expected, "{code}");
        if expected == JobPhase::Rejected {
            let Some(SubmissionResult::Rejected(refusal)) = completed.result() else {
                panic!("permanent refusal must retain its typed correction");
            };
            assert_eq!(refusal.code().as_str(), code);
            assert_eq!(
                refusal.correction(),
                if code == "invalid_game_version" {
                    SubmissionCorrection::NewCapture
                } else {
                    SubmissionCorrection::ReviewClient
                }
            );
        }
        server.verify().await;
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9_r_sub_11__unlisted_production_errors_check_receipt_before_rejecting() {
    early_account_answer().await.unwrap();
    for code in [
        "dummy_unknown_error",
        "Future_Code",
        "future-code",
        "no_api_found",
    ] {
        for (reply,expected) in [
        (ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":17,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":100}]})),JobPhase::PartiallyAccepted),
        (ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[]})),JobPhase::Rejected),
        (ResponseTemplate::new(503).set_body_string("unavailable"),JobPhase::Held),
        (ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id_terminal":89,"id_commodity":7,"date_added":100}]})),JobPhase::Held),
    ] {
        let server=MockServer::start().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(400).set_body_json(json!({"status":code,"data":null}))).expect(1).mount(&server).await;
        Mock::given(method("GET")).and(path("/2.0/data_info/")).and(wiremock::matchers::query_param("username","dummy-a")).and(wiremock::matchers::query_param("id_terminal","89")).and(wiremock::matchers::query_param("type","commodity")).and(header("authorization","Bearer dummy-token")).and(header("secret-key","dummy-key")).respond_with(reply).expect(1).mount(&server).await;
        let settings=ReaderSettings::for_test_server(&format!("{}/2.0/",server.uri()),ClientIdentity::new("tallyline-test","1.2.3")).unwrap().with_expert_host_confirmation();
        let reader=UexReader::new(&settings).unwrap();
        let dry=released_request().unwrap();
        let mut header=dry.header().clone();header.mode=SubmissionMode::Production;
        let request=SubmissionRequest::new(header,dry.rows().to_vec()).unwrap();
        let credentials=CredentialValues::new("dummy-key".to_owned(),Some("dummy-token".to_owned())).unwrap();
        let (prepared,hash)=reader.prepare(&request).unwrap();
        let sending=tallyline_submission::api::model::SubmissionJob::queued(tallyline_submission::api::model::JobId(1),request).unwrap().begin(Timestamp::from_seconds(101),hash).unwrap();
        let completed=sending.complete(reader.submit(prepared,&credentials,None).await,Timestamp::from_seconds(102)).unwrap();
        let completed=if code=="no_api_found" {
            assert_eq!(completed.phase(),tallyline_submission::api::model::JobPhase::Held);
            if let Some(result)=reader.recheck_submission_refusal(&completed,&credentials,None).await { completed.resolve_outcome_check(result).unwrap() } else { completed }
        } else { completed };
        let expected = if code == "no_api_found" && expected == JobPhase::Rejected { JobPhase::Paused } else { expected };
        assert_eq!(completed.phase(),expected);
        if code == "no_api_found" {
            assert_eq!(completed.result().and_then(SubmissionResult::account_pause), Some(SubmissionPauseReason::AppToken), "receipt certainty must not erase account refusal");
        }
        let retained = match completed.result() {
            Some(SubmissionResult::AccountRefusal { code, .. }) => code.as_ref(),
            Some(SubmissionResult::Rechecked { code, .. } | SubmissionResult::NeedsOutcomeCheck { code, .. } | SubmissionResult::PartialError { code, .. }) => Some(code),
            Some(SubmissionResult::Rejected(refusal)) => Some(refusal.code()),
            _ => None,
        };
        assert_eq!(retained.map(|value| value.as_str()), Some(code));
        server.verify().await;
    }
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9_r_sub_11__deferred_receipt_recheck_never_repeats_the_write() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"status":"dummy_unknown_error","data":null})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let unavailable = Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let dry = released_request().unwrap();
    let mut header = dry.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, dry.rows().to_vec()).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let (prepared, hash) = reader.prepare(&request).unwrap();
    let sending = tallyline_submission::api::model::SubmissionJob::queued(
        tallyline_submission::api::model::JobId(1),
        request,
    )
    .unwrap()
    .begin(Timestamp::from_seconds(101), hash)
    .unwrap();
    let held = sending
        .complete(
            reader.submit(prepared, &credentials, None).await,
            Timestamp::from_seconds(102),
        )
        .unwrap();
    assert_eq!(held.recover(Timestamp::from_seconds(103)).unwrap(), held);
    assert!(held.cancel().is_err());
    drop(unavailable);
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[]})))
        .expect(1)
        .mount(&server)
        .await;
    let result = reader
        .recheck_submission_refusal(&held, &credentials, None)
        .await
        .unwrap();
    let resolved = held
        .apply(tallyline_submission::api::model::SubmissionCommand::ResolveOutcomeCheck { result })
        .unwrap()
        .into_state();
    assert_eq!(
        resolved.phase(),
        tallyline_submission::api::model::JobPhase::Rejected
    );
    assert_eq!(resolved.attempts().len(), 1);
    assert!(format!("{:?}", resolved.result()).contains("dummy_unknown_error"));
    assert_eq!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.method == "POST")
            .count(),
        1
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_6_r_sub_11__remaining_transient_and_version_lockout_codes_keep_their_action()
{
    for (code, status, expected) in [
        ("image_upload_error", 500, JobPhase::Held),
        ("image_storage_error", 500, JobPhase::Held),
        ("requests_limit_reached", 429, JobPhase::Held),
        ("too_many_reports", 429, JobPhase::Held),
        ("exceeded_daily_quota", 429, JobPhase::Held),
        ("dummy_rate_limit", 429, JobPhase::Held),
        ("upgrade_required", 403, JobPhase::Paused),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(status).set_body_json(json!({"status":code,"data":null})),
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
        let request = released_request().unwrap();
        let credentials =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        let (prepared, hash) = reader.prepare(&request).unwrap();
        let sending = tallyline_submission::api::model::SubmissionJob::queued(
            tallyline_submission::api::model::JobId(1),
            request,
        )
        .unwrap()
        .begin(Timestamp::from_seconds(101), hash)
        .unwrap();
        let completed = sending
            .complete(
                reader.submit(prepared, &credentials, None).await,
                Timestamp::from_seconds(102),
            )
            .unwrap();
        assert_eq!(completed.phase(), expected, "{code}");
        if code == "dummy_rate_limit" {
            let Some(SubmissionResult::RateLimitedCode(retained)) = completed.result() else {
                panic!("rate refusal must retain its safe technical code");
            };
            assert_eq!(retained.as_str(), code);
        }
        server.verify().await;
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_11__duplicated_first_row_waits_for_cooldown_instead_of_becoming_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(400).set_body_json(json!({"status":"duplicated_report","data":{"report":{"id_terminal":89,"id_commodity":7}}}))).expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let request = released_request().unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let (prepared, hash) = reader.prepare(&request).unwrap();
    let sending = tallyline_submission::api::model::SubmissionJob::queued(
        tallyline_submission::api::model::JobId(1),
        request,
    )
    .unwrap()
    .begin(Timestamp::from_seconds(101), hash)
    .unwrap();
    let completed = sending
        .complete(
            reader.submit(prepared, &credentials, None).await,
            Timestamp::from_seconds(102),
        )
        .unwrap();
    assert_eq!(
        completed.phase(),
        tallyline_submission::api::model::JobPhase::Held
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_6_r_sub_11__unclassified_authentication_answers_recheck_read_only_and_pause() {
    for (response, production_phase) in [
        (
            ResponseTemplate::new(401).set_body_string("authentication unavailable"),
            tallyline_submission::api::model::JobPhase::OutcomeUnknown,
        ),
        (
            ResponseTemplate::new(403)
                .set_body_json(json!({"status":"dummy_auth_error","data":null})),
            tallyline_submission::api::model::JobPhase::Held,
        ),
        (
            ResponseTemplate::new(401).set_body_json(json!({"status":"dummy-key","data":null})),
            tallyline_submission::api::model::JobPhase::OutcomeUnknown,
        ),
        (
            ResponseTemplate::new(403).set_body_json(json!({"status":"dummy-token","data":null})),
            tallyline_submission::api::model::JobPhase::OutcomeUnknown,
        ),
    ] {
        for mode in [SubmissionMode::DryRun, SubmissionMode::Production] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(response.clone())
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET")).and(path("/2.0/user/")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"username":"dummy-a","is_datarunner":1,"is_datarunner_banned":0,"is_staff":0}}))).expect(1).mount(&server).await;
            let settings = ReaderSettings::for_test_server(
                &format!("{}/2.0/", server.uri()),
                ClientIdentity::new("tallyline-test", "1.2.3"),
            )
            .unwrap()
            .with_expert_host_confirmation();
            let reader = UexReader::new(&settings).unwrap();
            let base = released_request().unwrap();
            let mut header = base.header().clone();
            header.mode = mode;
            let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
            let credentials =
                CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))
                    .unwrap();
            let (prepared, hash) = reader.prepare(&request).unwrap();
            let sending = tallyline_submission::api::model::SubmissionJob::queued(
                tallyline_submission::api::model::JobId(1),
                request,
            )
            .unwrap()
            .begin(Timestamp::from_seconds(101), hash)
            .unwrap();
            let completed = sending
                .complete(
                    reader.submit(prepared, &credentials, None).await,
                    Timestamp::from_seconds(102),
                )
                .unwrap();
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
            let checked = reader
                .recheck_submission_refusal(&completed, &credentials, None)
                .await
                .unwrap();
            let tallyline_submission::api::model::SubmissionAccountCheck::Completed(action) =
                checked.account_evidence().unwrap().check()
            else {
                panic!("account read must complete independently");
            };
            let completed = completed.resolve_account_check(action).unwrap();
            assert!(!completed.attempts()[0].receipt_checked());
            assert_eq!(
                completed.phase(),
                if mode == SubmissionMode::DryRun {
                    tallyline_submission::api::model::JobPhase::Paused
                } else {
                    production_phase
                }
            );
            assert!(format!("{:?}", completed.result()).contains("KeyChecked"));
            assert!(!format!("{:?}", completed.result()).contains("dummy-key"));
            assert!(!format!("{:?}", completed.result()).contains("dummy-token"));
            server.verify().await;
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2__submission_policy_preserves_the_ninety_second_send_timeout() {
    assert_eq!(
        SubmissionRetrySettings::default().send_timeout(),
        Duration::from_secs(90)
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__timeout_and_gateway_timeout_after_receipt_each_make_one_request_and_stay_unknown()
{
    for response in [
        ResponseTemplate::new(504).set_body_string("gateway timeout"),
        ResponseTemplate::new(200)
            .set_body_json(json!({"status":"ok","data":{"ids_reports":["0"],"date_added":100}}))
            .set_delay(Duration::from_secs(2)),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        let settings = ReaderSettings::for_test_server(
            &format!("{}/2.0/", server.uri()),
            ClientIdentity::new("tallyline-test", "1.2.3"),
        )
        .unwrap()
        .with_expert_host_confirmation()
        .with_test_submission_timeout(Duration::from_millis(500));
        let reader = UexReader::new(&settings).unwrap();
        let request = released_request().unwrap();
        let credentials =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        let (prepared, hash) = reader.prepare(&request).unwrap();
        let sending = tallyline_submission::api::model::SubmissionJob::queued(
            tallyline_submission::api::model::JobId(1),
            request,
        )
        .unwrap()
        .begin(Timestamp::from_seconds(101), hash)
        .unwrap();
        let completed = sending
            .complete(
                tokio::time::timeout(
                    Duration::from_secs(5),
                    reader.submit(prepared, &credentials, None),
                )
                .await
                .expect("the HTTP request timeout must complete before the contract deadline"),
                Timestamp::from_seconds(192),
            )
            .unwrap();
        assert_eq!(
            completed.phase(),
            tallyline_submission::api::model::JobPhase::OutcomeUnknown
        );
        assert!(!completed.can_retry());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        server.verify().await;
    }
}

async fn early_account_answer() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(403).set_body_json(json!({"status":"no_api_found","data":null})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .respond_with(ResponseTemplate::new(503).set_delay(Duration::from_secs(30)))
        .expect(0)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?
    .with_expert_host_confirmation();
    let reader =
        UexReader::new(&settings).map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let base = released_request().ok_or("fixture request")?;
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec())
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let credentials = CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned()))
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let prepared = reader
        .prepare(&request)
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?
        .0;
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        reader.submit(prepared, &credentials, None),
    )
    .await
    .map_err(|_| {
        std::io::Error::other("account evidence must return for persistence before the blocked GET")
    })?;
    assert!(matches!(
        result.final_result(),
        SubmissionResult::NeedsOutcomeCheck { .. }
    ));
    assert!(result.account_pause().is_some());
    server.verify().await;
    Ok(())
}

#[path = "queue_contract_tests/verified_row_receipts.rs"]
mod verified_row_receipts;

#[path = "queue_contract_tests/partial_refusals.rs"]
mod partial_refusals;

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11_r_api_3__daily_quota_retains_reset_without_receipt_probe() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/data_submit/"))
        .respond_with(
            ResponseTemplate::new(429).set_body_json(
                json!({"status":"exceeded_daily_quota","data":{"reset":1793858400}}),
            ),
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
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(
            reader.prepare(&released_request().unwrap()).unwrap().0,
            &credentials,
            None,
        )
        .await;
    assert_eq!(
        result.daily_quota_reset(),
        Some(Timestamp::from_seconds(1793858400))
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11_r_api_3__quota_reset_retains_its_response_server_time_after_tracker_changes()
 {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("date", "Thu, 01 Jan 1970 00:16:40 GMT")
                .set_body_json(json!({"status":"exceeded_daily_quota","data":{"reset":1010}})),
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
    let clock = Arc::new(ServerClock {
        tick: AtomicU64::new(0),
    });
    let reader = UexReader::new_with_server_time(
        &settings,
        crate::api::model::ServerTimeTracker::new(clock.clone()),
    )
    .unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(
            reader.prepare(&released_request().unwrap()).unwrap().0,
            &values,
            None,
        )
        .await;
    clock.tick.store(20_000, Ordering::SeqCst);
    assert_eq!(
        result.daily_quota_reset(),
        Some(Timestamp::from_seconds(1010))
    );
    assert_eq!(
        result.daily_quota_observed_at(),
        Some(Timestamp::from_seconds(1000))
    );
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_11_r_api_3__authenticated_receipt_quota_is_observable_without_a_post_attempt()
 {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("date", "Thu, 01 Jan 1970 00:16:40 GMT")
                .set_body_json(json!({"status":"exceeded_daily_quota","data":{"reset":1010}})),
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
    let reader = UexReader::new_with_server_time(
        &settings,
        crate::api::model::ServerTimeTracker::new(Arc::new(ServerClock {
            tick: AtomicU64::new(0),
        })),
    )
    .unwrap();
    let base = released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
    let original = accepted_job(
        &request,
        SubmissionResult::Unknown,
        Timestamp::from_seconds(100),
    )
    .unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert!(
        tallyline_submission::api::SubmissionRowReceiptVerifier::verify_row_receipts(
            &reader,
            &original,
            &credentials,
            None
        )
        .await
        .is_none()
    );
    let observations = reader.take_quota_observations();
    assert_eq!(observations.len(), 1);
    assert_eq!(
        observations[0].reset().unwrap().usable_reset(),
        Some(Timestamp::from_seconds(1010))
    );
    assert_eq!(original.attempts().len(), 1);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert!(reader.take_quota_observations().is_empty());
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_2_r_sub_9_r_api_3__partial_receipt_quota_stops_remaining_ids_and_window_before_handoff()
 {
    use tallyline_shared_kernel::api::model::UexReportId;
    use tallyline_submission::api::SubmissionRowReceiptVerifier as _;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/data_info/"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("date", "Thu, 01 Jan 1970 00:16:40 GMT")
                .set_body_json(json!({"status":"exceeded_daily_quota","data":{"reset":1010}})),
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
    let reader = UexReader::new(&settings)
        .unwrap()
        .with_authenticated_read_gate(crate::api::AuthenticatedReadGate::new(|_| true));
    let base = released_request().unwrap();
    let mut header = base.header().clone();
    header.mode = SubmissionMode::Production;
    let mut rows = base.rows().to_vec();
    for (row, commodity) in [(8, 8), (9, 9)] {
        rows.push(
            SubmissionRow::new(
                RowId(row),
                CommodityId(commodity),
                "120".parse().unwrap(),
                "0".parse().unwrap(),
                1,
            )
            .unwrap(),
        );
    }
    let request = SubmissionRequest::new(header, rows).unwrap();
    let job = accepted_job(
        &request,
        SubmissionResult::Partial(vec![
            UexReportId::new(81, Timestamp::from_seconds(100)).unwrap(),
            UexReportId::new(82, Timestamp::from_seconds(100)).unwrap(),
        ]),
        Timestamp::from_seconds(100),
    )
    .unwrap();
    assert_eq!(job.phase(), JobPhase::PartiallyAccepted);
    let original = job.clone();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    assert!(
        reader
            .verify_row_receipts(&job, &credentials, None)
            .await
            .is_none()
    );
    assert!(
        reader
            .verify_row_receipts(&job, &credentials, None)
            .await
            .is_none()
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "a known local quota must stop remaining returned IDs, the final window and repeated reads before owner handoff"
    );
    assert!(
        requests[0]
            .url
            .query_pairs()
            .any(|(key, value)| key == "id" && value == "81")
    );
    let observations = reader.take_quota_observations();
    assert_eq!(observations.len(), 1);
    assert_eq!(
        observations[0].reset().map(|reset| reset.reset()),
        Some(Timestamp::from_seconds(1010))
    );
    assert_eq!(job, original);
    server.verify().await;
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_13__commodity_payload_contains_only_explicit_report_affinity_including_zero() {
    for affinity in [Some(-100), Some(0), Some(100), None] {
        let server = MockServer::start().await;
        let mut expected = json!({"id_terminal":89,"type":"commodity","is_production":0,"game_version":"4.10.2","date_added":100,"prices":[{"id_commodity":7,"price_sell":120.125,"status_sell":1}]});
        if let Some(value) = affinity {
            expected
                .as_object_mut()
                .unwrap()
                .insert("faction_affinity".to_owned(), json!(value));
        }
        Mock::given(method("POST"))
            .and(path("/2.0/data_submit/"))
            .and(body_json(expected))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"status":"ok","data":{"ids_reports":null,"date_added":100}}),
                ),
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
        let base = released_request().unwrap();
        let mut header = base.header().clone();
        header.faction_affinity = affinity
            .map(|value| tallyline_shared_kernel::api::model::FactionAffinity::new(value).unwrap());
        let request = SubmissionRequest::new(header, base.rows().to_vec()).unwrap();
        let credentials =
            CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
        assert_eq!(
            reader
                .submit(reader.prepare(&request).unwrap().0, &credentials, None)
                .await,
            SubmissionResult::DryRunSucceeded
        );
        server.verify().await;
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_4_r_sub_13__cooldown_child_requests_keep_exact_partitioned_wire_values() {
    use tallyline_submission::api::SubmissionSchedule;
    use tallyline_submission::api::model::{
        JobId, PayloadHash, ScheduleDecision, SubmissionCommand, SubmissionJob,
    };
    let first = released_request().unwrap();
    let row = first.rows()[0].clone();
    let second = SubmissionRow::new(
        RowId(2),
        CommodityId(8),
        row.price(),
        row.stock(),
        row.status(),
    )
    .unwrap();
    let mut header = first.header().clone();
    header.faction_affinity = Some("0".parse().unwrap());
    let request = SubmissionRequest::new(header, vec![row, second]).unwrap();
    let original = SubmissionJob::queued(JobId(2), request.clone()).unwrap();
    let previous = SubmissionJob::queued(JobId(1), first)
        .unwrap()
        .begin(Timestamp::from_seconds(110), PayloadHash::new([1; 32]))
        .unwrap()
        .complete(
            SubmissionResult::DryRunSucceeded,
            Timestamp::from_seconds(115),
        )
        .unwrap();
    let partition = SubmissionSchedule::cooldown_partition(
        &original,
        std::slice::from_ref(&previous),
        Timestamp::from_seconds(200),
    )
    .unwrap();
    let cancelled = original
        .apply(SubmissionCommand::CancelForSplit {
            report: ReportId(3),
            at: Timestamp::from_seconds(200),
        })
        .unwrap()
        .into_state();
    let history = vec![previous, cancelled];
    let server = MockServer::start().await;
    for commodity in [8, 7] {
        Mock::given(method("POST")).and(path("/2.0/data_submit/"))
            .and(body_json(json!({"id_terminal":89,"type":"commodity","is_production":0,"game_version":"4.10.2","date_added":100,"faction_affinity":0,"prices":[{"id_commodity":commodity,"price_sell":120.125,"status_sell":1}]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"ids_reports":null,"date_added":100}})))
            .expect(1).mount(&server).await;
    }
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap()
    .with_expert_host_confirmation();
    let reader = UexReader::new(&settings).unwrap();
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let cooling = partition
        .cooling_rows()
        .iter()
        .map(|row| row.row())
        .collect::<Vec<_>>();
    for (id, rows, at) in [
        (3, partition.ready_rows(), Timestamp::from_seconds(200)),
        (4, cooling.as_slice(), Timestamp::from_seconds(415)),
    ] {
        let mut header = request.header().clone();
        header.report = ReportId(id);
        header.report_version = 4;
        let child = SubmissionRequest::new(
            header,
            request
                .rows()
                .iter()
                .filter(|row| rows.contains(&row.id()))
                .cloned()
                .collect(),
        )
        .unwrap();
        let job = SubmissionJob::queued(JobId(id), child).unwrap();
        assert_eq!(
            SubmissionSchedule::assess(&job, &history, at),
            ScheduleDecision::Ready
        );
        assert_eq!(
            reader
                .submit(reader.prepare(job.request()).unwrap().0, &credentials, None)
                .await,
            SubmissionResult::DryRunSucceeded
        );
    }
    assert_eq!(original.request(), &request);
    server.verify().await;
}

mod lifecycle_reads;
