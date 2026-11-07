// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed failures retain only bounded technical error identifiers.

use serde_json::json;

use super::{sample, sample_for};
use crate::api::model::PublicContractEndpoint;

#[tokio::test]
async fn r_api_5_r_doc_2__new_uex_error_identifiers_are_visible_without_messages_or_values() {
    let error = sample(
        403,
        json!({"status":"api_contract_changed","http_code":403,
            "message":"never-emit-the-message","data":{"name":"never-emit-the-value"}}),
    )
    .await
    .into_schema()
    .unwrap_err();
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("api_contract_changed"));
    assert!(diagnostic.contains("403"));
    assert!(!diagnostic.contains("never-emit"));
    for status in [
        "never-emit\nprivate-value".to_owned(),
        "never-emit".to_owned(),
        "never_emit".repeat(8),
        "1never_emit".to_owned(),
    ] {
        let invalid = sample(
            403,
            json!({"status":status,"http_code":403,
                "message":"never-emit","data":null}),
        )
        .await
        .into_schema()
        .unwrap_err();
        assert!(!format!("{invalid:?}").contains("never"));
    }
}

#[tokio::test]
async fn r_api_5_r_doc_2__untrusted_field_names_and_oversized_shapes_have_closed_diagnostics() {
    let invalid_name = sample(
        200,
        json!({"status":"ok","http_code":200,"data":{"never-emit\nprivate-value":1}}),
    )
    .await
    .into_schema()
    .unwrap_err();
    assert!(!format!("{invalid_name:?}").contains("never"));
    let mut data = json!({"never_emit_private_value":"never-emit"});
    for _ in 0..20 {
        data = json!({"child":data});
    }
    let excessive_depth = sample(200, json!({"status":"ok","http_code":200,"data":data}))
        .await
        .into_schema()
        .unwrap_err();
    assert!(!format!("{excessive_depth:?}").contains("never"));
}

#[tokio::test]
async fn r_api_5_r_doc_2__malformed_or_ambiguous_status_codes_cannot_seed_a_baseline() {
    for buy in [
        json!([]),
        json!([{"code":0}]),
        json!([{"code":256}]),
        json!([{"code":"invalid"}]),
        json!([{"code":1},{"code":"1"}]),
    ] {
        assert!(
            sample_for(
                PublicContractEndpoint::CommoditiesStatus,
                "/2.0/commodities_status/",
                200,
                json!({"status":"ok","http_code":200,
                    "data":{"buy":buy,"sell":[{"code":1}]}}),
            )
            .await
            .into_schema()
            .is_err()
        );
    }
}
