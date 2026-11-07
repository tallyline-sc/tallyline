// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parameter boundary contracts through the public typed fetcher.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::json;
use tallyline_reference_data::api::ReferenceDataFetcher;
use tallyline_reference_data::api::model::{Endpoint, EndpointData};
use tallyline_shared_kernel::api::model::UexEnvironment;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn r_api_5_r_ui_8__parameters_map_strict_flags_tolerances_and_one_literal_unescape_pass() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/data_parameters/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{"global":{"is_datacenter_enabled":"1","is_accepting_reports":1,"is_accepting_ptu_reports":"0"},"commodity":{"is_accepted":"1","price_variation":"25","ttl":15,"scu_variation":"5000","notification":"<b>&amp;amp;</b>"}}})))
        .mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.0.0"),
    )
    .unwrap();
    let EndpointData::Parameters(parameters) = UexReader::new(&settings)
        .unwrap()
        .fetch(Endpoint::Parameters)
        .await
        .unwrap()
    else {
        panic!("wrong endpoint")
    };
    assert!(parameters.acceptance().allows(UexEnvironment::Live));
    assert!(!parameters.acceptance().allows(UexEnvironment::Ptu));
    assert_eq!(parameters.constraints().price_variation_percent, 25);
    assert_eq!(parameters.constraints().prior_ttl_days.get(), 15);
    assert_eq!(parameters.constraints().scu_variation_percent, 5000);
    assert_eq!(parameters.staff_notification(), Some("<b>&amp;</b>"));
}

#[tokio::test]
async fn r_api_5__malformed_parameter_flags_tolerances_and_notification_reject_the_response() {
    let server = MockServer::start().await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.0.0"),
    )
    .unwrap();
    let reader = UexReader::new(&settings).unwrap();
    for (section, key, value) in [
        ("global", "is_accepting_reports", json!(2)),
        ("global", "is_datacenter_enabled", json!("-1")),
        ("global", "is_accepting_ptu_reports", json!(null)),
        ("commodity", "is_accepted", json!("true")),
        ("commodity", "ttl", json!(0)),
        ("commodity", "price_variation", json!("1.5")),
        ("commodity", "scu_variation", json!(4294967296_u64)),
        ("commodity", "notification", json!(["invalid type"])),
    ] {
        server.reset().await;
        let mut data = json!({"global":{"is_datacenter_enabled":1,"is_accepting_reports":1,"is_accepting_ptu_reports":1},"commodity":{"is_accepted":1,"price_variation":25,"ttl":15,"scu_variation":5000,"notification":null}});
        data[section][key] = value;
        Mock::given(method("GET"))
            .and(path("/2.0/data_parameters/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":data})),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            reader.fetch(Endpoint::Parameters).await,
            Err(crate::api::ReadError::Malformed { .. })
        ));
    }
}
