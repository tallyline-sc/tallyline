// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Complete fixed-route metadata baselines and safe decoding.

use crate::api::model::{PublicContractEndpoint, PublicContractSchema, PublicContractSnapshot};
use serde_json::json;

use super::sample_for;

#[tokio::test]
async fn r_api_5_r_doc_2__a_baseline_requires_all_five_routes_and_roundtrips_only_metadata() {
    let schemas = schemas().await;
    let baseline = PublicContractSnapshot::new(schemas.clone()).unwrap();
    let text = baseline.to_json().unwrap();
    assert!(!text.contains("never-emit"));
    let decoded = PublicContractSnapshot::from_json(&text).unwrap();
    assert!(decoded.changes_from(&baseline).is_empty());
    let mut partial = schemas.clone();
    partial.pop();
    assert!(PublicContractSnapshot::new(partial).is_err());
    let mut duplicated = schemas.clone();
    duplicated.pop();
    duplicated.push(schemas.first().unwrap().clone());
    assert!(PublicContractSnapshot::new(duplicated).is_err());
    let mut invalid: serde_json::Value = serde_json::from_str(&text).unwrap();
    invalid["endpoints"][0]["fields"]["data.invalid\nname"] = json!(["string"]);
    assert!(PublicContractSnapshot::from_json(&invalid.to_string()).is_err());
    assert!(PublicContractSnapshot::from_json("{\"message\":\"never-emit\"}").is_err());
}

#[tokio::test]
async fn r_api_5_r_doc_2__changed_status_codes_are_reported_without_labels() {
    let reference = PublicContractSnapshot::new(schemas().await).unwrap();
    let mut current = schemas().await;
    let changed = sample_for(
        PublicContractEndpoint::CommoditiesStatus,
        "/2.0/commodities_status/",
        200,
        json!({"status":"ok","http_code":200,"message":"never-emit","data":{
            "buy":[{"code":2,"name":"never-emit-changed-label"}],
            "sell":[{"code":1,"name":"never-emit-changed-label"}]
        }}),
    )
    .await
    .into_schema()
    .unwrap();
    let slot = current.get_mut(1).unwrap();
    *slot = changed;
    let changes = PublicContractSnapshot::new(current)
        .unwrap()
        .changes_from(&reference);
    assert_eq!(changes.len(), 1);
    let line = changes.first().unwrap();
    assert!(line.contains("codes changed"));
    assert!(line.contains("buy"));
    assert!(line.contains('1') && line.contains('2'));
    assert!(!line.contains("never-emit"));
}

async fn schemas() -> Vec<PublicContractSchema> {
    let cases = [
        (
            PublicContractEndpoint::GameVersions,
            "/2.0/game_versions/",
            json!({"live":"never-emit-version","ptu":null}),
        ),
        (
            PublicContractEndpoint::CommoditiesStatus,
            "/2.0/commodities_status/",
            json!({"buy":[{"code":1,"name":"never-emit"}],"sell":[{"code":1,"name":"never-emit"}]}),
        ),
        (
            PublicContractEndpoint::DataParameters,
            "/2.0/data_parameters/",
            json!({"global":{"game_version":"never-emit","limit":987654}}),
        ),
        (
            PublicContractEndpoint::Commodities,
            "/2.0/commodities/",
            json!([{"id":987654,"name":"never-emit"}]),
        ),
        (
            PublicContractEndpoint::Terminals,
            "/2.0/terminals/",
            json!([{"id":987654,"name":"never-emit"}]),
        ),
    ];
    let mut schemas = Vec::new();
    for (endpoint, path, data) in cases {
        schemas.push(
            sample_for(
                endpoint,
                path,
                200,
                json!({"status":"ok","http_code":200,"message":"never-emit","data":data}),
            )
            .await
            .into_schema()
            .unwrap(),
        );
    }
    schemas
}
