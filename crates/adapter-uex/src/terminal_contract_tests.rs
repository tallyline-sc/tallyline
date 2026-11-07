// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public terminal-reader contracts against documented wire responses.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::json;
use tallyline_shared_kernel::api::model::{TerminalId, UexEnvironment};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn r_api_1_r_api_5_r_api_6__terminal_vocabulary_is_public_decoded_once_and_filtered_at_the_boundary()
 {
    let server = MockServer::start().await;
    let mut live = json!({
        "id":"17", "name":"Grey&apos;s Market", "nickname":"Grey&amp;apos;s",
        "displayname":"Trading &amp; Shipping", "type":"commodity",
        "is_player_owned":"0", "is_available_live":"1",
        "star_system_name":"Stanton", "planet_name":"ArcCorp", "orbit_name":null,
        "moon_name":null, "space_station_name":null, "outpost_name":null, "city_name":"Area18",
        "mcs": "deliberately ignored"
    });
    let mut test = live.clone();
    test["id"] = json!(18);
    test["is_available_live"] = json!(0);
    test["planet_name"] = json!("Stanton");
    let mut player_owned = live.clone();
    player_owned["id"] = json!(19);
    player_owned["is_player_owned"] = json!(1);
    let mut refinery = live.clone();
    refinery["id"] = json!(20);
    refinery["type"] = json!("commodity_raw");
    live["max_container_size"] = json!(32);
    Mock::given(method("GET"))
        .and(path("/2.0/terminals/"))
        .and(query_param("type", "commodity"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status":"ok","data":[live,test,player_owned,refinery]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap();
    let terminals = UexReader::new(&settings)
        .unwrap()
        .terminals()
        .await
        .unwrap();
    assert_eq!(
        terminals
            .iter()
            .map(|terminal| terminal.id())
            .collect::<Vec<_>>(),
        vec![TerminalId(17), TerminalId(18)]
    );
    let first = terminals.first().unwrap();
    assert_eq!(first.max_container_size().unwrap().get(), 32);
    assert_eq!(first.text().name, "Grey's Market");
    assert_eq!(first.text().nickname, "Grey&apos;s");
    assert_eq!(first.text().display_name, "Trading & Shipping");
    assert_eq!(first.text().location, "Stanton / ArcCorp / Area18");
    let last = terminals.last().unwrap();
    assert_eq!(last.max_container_size(), None);
    assert_eq!(last.text().location, "Stanton / Area18");
    assert!(!last.available(UexEnvironment::Live));
    assert!(last.available(UexEnvironment::Ptu));
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .all(|request| !request.headers.contains_key("authorization")
                && !request.headers.contains_key("secret-key"))
    );
}

#[tokio::test]
async fn r_api_5__malformed_or_duplicate_terminal_identity_rejects_the_whole_response() {
    use crate::api::ReadError;
    for change in [
        json!({"id":0}),
        json!({"id":4_294_967_296_u64}),
        json!({"id":17}),
        json!({"id":18,"is_available_live":2}),
        json!({"id":18,"is_player_owned":"invalid"}),
    ] {
        let server = MockServer::start().await;
        let valid = json!({"id":17,"name":"Same","nickname":"Same","displayname":"Same","type":"commodity","is_player_owned":0,"is_available_live":1});
        let mut invalid = valid.clone();
        for (key, value) in change.as_object().unwrap() {
            invalid[key] = value.clone();
        }
        Mock::given(method("GET"))
            .and(path("/2.0/terminals/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"status":"ok","data":[valid,invalid]})),
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
            UexReader::new(&settings).unwrap().terminals().await,
            Err(ReadError::Malformed { .. })
        ));
    }
}

#[tokio::test]
async fn r_api_1_r_api_6_r_cap_3b__fetcher_port_returns_typed_public_vocabulary_and_versions() {
    use tallyline_reference_data::api::ReferenceDataFetcher;
    use tallyline_reference_data::api::model::{Endpoint, EndpointData};
    let server = MockServer::start().await;
    for (endpoint, data) in [
        (
            "commodities",
            json!([{"id":"7","name":"Ore &amp; Metal","code":"SAME","is_available_live":"0","price_buy":"9007199254740993.125","price_sell":0},{"id":8,"name":"Refined Metal","code":"SAME","is_available_live":1,"price_buy":0,"price_sell":0}]),
        ),
        ("game_versions", json!({"live":"4.9","ptu":null})),
        ("terminals", json!([])),
    ] {
        Mock::given(method("GET"))
            .and(path(format!("/2.0/{endpoint}/")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":data})),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap();
    let reader = UexReader::new(&settings).unwrap();
    let EndpointData::Commodities(commodities) = reader.fetch(Endpoint::Commodities).await.unwrap()
    else {
        panic!("wrong endpoint data")
    };
    assert_eq!(commodities.len(), 2);
    assert_eq!(commodities.first().unwrap().name(), "Ore & Metal");
    assert!(!commodities.first().unwrap().available(UexEnvironment::Live));
    assert!(commodities.first().unwrap().available(UexEnvironment::Ptu));
    assert_eq!(
        commodities.first().unwrap().code(),
        commodities.last().unwrap().code()
    );
    let EndpointData::GameVersions(versions) = reader.fetch(Endpoint::GameVersions).await.unwrap()
    else {
        panic!("wrong endpoint data")
    };
    assert_eq!(versions.live, "4.9");
    assert_eq!(versions.ptu, None);
    assert_eq!(
        reader.fetch(Endpoint::Terminals).await.unwrap(),
        EndpointData::Terminals(vec![])
    );
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
async fn r_api_5__invalid_price_numbers_cannot_become_a_traded_side() {
    use crate::api::ReadError;
    use crate::api::model::TerminalId;
    for price in [
        "NaN",
        "inf",
        "-1",
        "1e309",
        "1.00000000000000000000000000001e0",
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/2.0/commodities_prices/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{
                    "id_terminal":17,"id_commodity":7,"price_buy":price,"price_sell":0,
                    "status_buy":1,"status_sell":0,"container_sizes":null
                }]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let settings = ReaderSettings::for_test_server(
            &format!("{}/2.0/", server.uri()),
            ClientIdentity::new("tallyline-test", "1.2.3"),
        )
        .unwrap();
        assert!(
            matches!(
                UexReader::new(&settings)
                    .unwrap()
                    .price_rows(TerminalId(17))
                    .await,
                Err(ReadError::Malformed { .. })
            ),
            "{price}"
        );
    }
}

#[tokio::test]
async fn r_api_5__json_price_numbers_reach_typed_values_without_binary_float_rounding() {
    use crate::api::model::{TerminalId, TradedSide};
    let server = MockServer::start().await;
    let body = r#"{"status":"ok","data":[
        {"id_terminal":17,"id_commodity":1,"price_buy":9007199254740993.125,"price_sell":0,"status_buy":1,"status_sell":0},
        {"id_terminal":17,"id_commodity":2,"price_buy":0,"price_sell":9.7e-7,"status_buy":0,"status_sell":1},
        {"id_terminal":17,"id_commodity":3,"price_buy":0,"price_sell":0,"status_buy":0,"status_sell":0},
        {"id_terminal":17,"id_commodity":4,"price_buy":2,"price_sell":3,"status_buy":1,"status_sell":1}
    ]}"#;
    Mock::given(method("GET"))
        .and(path("/2.0/commodities_prices/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap();
    let rows = UexReader::new(&settings)
        .unwrap()
        .price_rows(TerminalId(17))
        .await
        .unwrap();
    assert_eq!(rows[0].price.unwrap().to_string(), "9007199254740993.125");
    assert_eq!(rows[1].price.unwrap().to_string(), "0.00000097");
    assert_eq!(rows[0].side, TradedSide::Buy);
    assert_eq!(rows[1].side, TradedSide::Sell);
    assert_eq!(rows[2].price, None);
    assert_eq!(rows[3].price, None);
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_api_5_r_val_2a__commodity_averages_keep_exact_numbers_and_separate_missing_sides() {
    use tallyline_reference_data::api::ReferenceDataFetcher;
    use tallyline_reference_data::api::model::{Endpoint, EndpointData};
    use tallyline_shared_kernel::api::model::TradeSide;
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/commodities/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"status":"ok","data":[{"id":1,"name":"Exact","code":"EX","is_available_live":1,"price_buy":9007199254740993.125,"price_sell":"9.7e-7"},{"id":2,"name":"Missing","code":"NO","is_available_live":"0","price_buy":0,"price_sell":"0"}]}"#,"application/json")).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.0.0"),
    )
    .unwrap();
    let EndpointData::Commodities(rows) = UexReader::new(&settings)
        .unwrap()
        .fetch(Endpoint::Commodities)
        .await
        .unwrap()
    else {
        panic!("wrong endpoint")
    };
    assert_eq!(
        rows[0].average_price(UexEnvironment::Live, TradeSide::Buy),
        Some("9007199254740993.125".parse().unwrap())
    );
    assert_eq!(
        rows[0].average_price(UexEnvironment::Live, TradeSide::Sell),
        Some("0.00000097".parse().unwrap())
    );
    for side in [TradeSide::Buy, TradeSide::Sell] {
        assert_eq!(rows[1].average_price(UexEnvironment::Live, side), None);
        assert_eq!(rows[0].average_price(UexEnvironment::Ptu, side), None);
    }
}
