// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed public status-fetcher contracts.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::json;
use tallyline_reference_data::api::ReferenceDataFetcher;
use tallyline_reference_data::api::model::{Endpoint, EndpointData};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn r_api_1_r_api_5__status_fetcher_preserves_independent_decoded_side_labels() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/commodities_status/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":{
            "buy":[{"code":"7","name":"Maximum &amp; Full","name_short":"Maximum","name_abbr":"MA"}],
            "sell":[{"code":7,"name":"Maximum &amp; No Demand","name_short":"Maximum","name_abbr":"ND"}]
        }}))).expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap();
    let EndpointData::StatusLevels(levels) = UexReader::new(&settings)
        .unwrap()
        .fetch(Endpoint::StatusLevels)
        .await
        .unwrap()
    else {
        panic!("wrong endpoint data")
    };
    assert_eq!(levels.buy.first().unwrap().code.get(), 7);
    assert_eq!(levels.buy.first().unwrap().name, "Maximum & Full");
    assert_eq!(levels.sell.first().unwrap().name, "Maximum & No Demand");
    assert_eq!(levels.buy.first().unwrap().abbreviation, "MA");
    assert_eq!(levels.sell.first().unwrap().abbreviation, "ND");
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| !request.headers.contains_key("authorization")
                && !request.headers.contains_key("secret-key"))
    );
}

#[tokio::test]
async fn r_api_5__status_fetcher_rejects_zero_or_repeated_codes_within_one_side() {
    use crate::api::ReadError;
    for codes in [vec![7, 7], vec![0]] {
        let server = MockServer::start().await;
        let buy = codes
            .into_iter()
            .map(|code| json!({"code":code,"name":"Label","name_short":"Label","name_abbr":"XX"}))
            .collect::<Vec<_>>();
        Mock::given(method("GET"))
            .and(path("/2.0/commodities_status/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"status":"ok","data":{"buy":buy,"sell":[]}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let settings = ReaderSettings::for_test_server(
            &format!("{}/2.0/", server.uri()),
            ClientIdentity::new("tallyline-test", "1.2.3"),
        )
        .unwrap();
        assert!(matches!(
            UexReader::new(&settings)
                .unwrap()
                .fetch(Endpoint::StatusLevels)
                .await,
            Err(ReadError::Malformed { .. })
        ));
    }
}
