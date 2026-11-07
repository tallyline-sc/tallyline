// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public contract metadata and credential-free request boundaries.

mod failures;
mod snapshots;

use crate::api::model::{PublicContractEndpoint, PublicContractPayload};
use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn r_api_5_r_doc_2__public_schema_retains_field_types_without_response_values_or_credentials()
{
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/game_versions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status":"ok", "http_code":200,
            "message":"never-emit-the-server-message",
            "data":{"live":"never-emit-the-game-version","ptu":null}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-contract-test", "1.2.3"),
    )
    .unwrap();
    let payload = UexReader::new(&settings)
        .unwrap()
        .public_contract_sample(PublicContractEndpoint::GameVersions)
        .await
        .unwrap();
    assert!(!format!("{payload:?}").contains("never-emit"));
    let metadata = payload.into_schema().unwrap().to_json().unwrap();
    assert!(metadata.contains("data.live"));
    assert!(metadata.contains("string"));
    assert!(metadata.contains("http_code"));
    assert!(!metadata.contains("never-emit"));
    assert!(!metadata.contains("1.2.3"));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = requests.first().unwrap();
    assert!(request.body.is_empty());
    assert!(!request.headers.contains_key("authorization"));
    assert!(!request.headers.contains_key("secret-key"));
    assert!(!request.headers.contains_key("cookie"));
}

#[tokio::test]
async fn r_api_5_r_doc_2__a_failed_or_changed_envelope_cannot_be_a_successful_schema_sample() {
    let cases = [
        (200, json!([])),
        (200, json!({"http_code":200,"data":{}})),
        (200, json!({"status":true,"http_code":200,"data":{}})),
        (
            200,
            json!({"status":"private-refusal","http_code":200,"data":{}}),
        ),
        (200, json!({"status":"ok","data":{}})),
        (200, json!({"status":"ok","http_code":403,"data":{}})),
        (200, json!({"status":"ok","http_code":200})),
        (503, json!({"status":"ok","http_code":503,"data":{}})),
    ];
    for (status, answer) in cases {
        assert!(sample(status, answer).await.into_schema().is_err());
    }
    assert!(
        sample(
            200,
            json!({"status":"ok","http_code":"200","data":{"live":"withheld"}}),
        )
        .await
        .into_schema()
        .is_ok()
    );
}

async fn sample(status: u16, answer: serde_json::Value) -> PublicContractPayload {
    sample_for(
        PublicContractEndpoint::GameVersions,
        "/2.0/game_versions/",
        status,
        answer,
    )
    .await
}

async fn sample_for(
    endpoint: PublicContractEndpoint,
    resource: &str,
    status: u16,
    answer: serde_json::Value,
) -> PublicContractPayload {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(resource))
        .respond_with(ResponseTemplate::new(status).set_body_json(answer))
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-contract-test", "1.2.3"),
    )
    .unwrap();
    let payload = UexReader::new(&settings)
        .unwrap()
        .public_contract_sample(endpoint)
        .await
        .unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = requests.first().unwrap();
    assert!(request.body.is_empty());
    assert!(!request.headers.contains_key("authorization"));
    assert!(!request.headers.contains_key("secret-key"));
    assert!(!request.headers.contains_key("cookie"));
    match endpoint {
        PublicContractEndpoint::Terminals => assert_eq!(
            request.url.query_pairs().collect::<Vec<_>>(),
            vec![("type".into(), "commodity".into())],
        ),
        PublicContractEndpoint::GameVersions
        | PublicContractEndpoint::CommoditiesStatus
        | PublicContractEndpoint::DataParameters
        | PublicContractEndpoint::Commodities => assert!(request.url.query().is_none()),
    }
    payload
}

#[tokio::test]
async fn r_api_5_r_doc_2__only_status_vocabulary_codes_survive_as_scalar_metadata() {
    let payload = sample_for(
        PublicContractEndpoint::CommoditiesStatus,
        "/2.0/commodities_status/",
        200,
        json!({
            "status":"ok", "http_code":200, "message":"never-emit-message",
            "data":{
                "buy":[{"code":4,"name":"never-emit-buy"},{"code":"1","name":"never-emit-name"}],
                "sell":[{"code":6,"name":"never-emit-sell"},{"code":1,"name":"never-emit-name"}]
            }
        }),
    )
    .await;
    let metadata = payload.into_schema().unwrap().to_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert_eq!(value["status_codes"]["buy"], json!([1, 4]));
    assert_eq!(value["status_codes"]["sell"], json!([1, 6]));
    assert!(!metadata.contains("never-emit"));
    assert!(!metadata.contains("\"ok\""));
}

#[tokio::test]
async fn r_api_5_r_doc_2__schema_comparison_reports_names_and_types_but_ignores_changed_values() {
    let reference = sample(
        200,
        json!({"status":"ok","http_code":200,"data":{"live":"never-emit-old","ptu":null,"reset":10}}),
    )
    .await
    .into_schema()
    .unwrap();
    let unchanged = sample(
        200,
        json!({"status":"ok","http_code":200,"data":{"live":"never-emit-new","ptu":"never-emit-ptu","reset":999}}),
    )
    .await
    .into_schema()
    .unwrap();
    assert!(unchanged.changes_from(&reference).is_empty());
    let changed = sample(
        200,
        json!({"status":"ok","http_code":200,"data":{"live_version":"never-emit-third","ptu":null,"reset":"never-emit-type"}}),
    )
    .await
    .into_schema()
    .unwrap();
    let changes = changed.changes_from(&reference);
    assert_eq!(changes.len(), 3);
    assert!(
        changes
            .iter()
            .any(|line| line.contains("removed") && line.contains("data.live"))
    );
    assert!(
        changes
            .iter()
            .any(|line| line.contains("added") && line.contains("data.live_version"))
    );
    assert!(
        changes
            .iter()
            .any(|line| line.contains("type") && line.contains("data.reset"))
    );
    assert!(!changes.join("\n").contains("never-emit"));
}

#[tokio::test]
async fn r_api_5_r_doc_2__an_unobservable_reference_set_cannot_be_a_successful_baseline() {
    for data in [json!(null), json!([]), json!({}), json!([null])] {
        assert!(
            sample(200, json!({"status":"ok","http_code":200,"data":data}))
                .await
                .into_schema()
                .is_err()
        );
    }
}
