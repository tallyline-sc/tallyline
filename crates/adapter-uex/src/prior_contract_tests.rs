// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! LIVE prior boundary contracts through the typed public port.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use tallyline_reference_data::api::ReferenceDataFetcher;
use tallyline_shared_kernel::api::model::{ContainerSize, TerminalId, Timestamp, TradeSide};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn r_api_5_r_api_6__prior_batches_preserve_exact_stock_side_time_version_and_container_values()
 {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/commodities_prices/")).and(query_param("id_terminal","17,18"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"status":"ok","data":[{"id_terminal":"17","id_commodity":7,"price_buy":0,"price_sell":"97.125","scu_buy":0,"scu_sell_stock":"0.0625","scu_sell":99999,"status_buy":0,"status_sell":0,"container_sizes":"1,16","game_version":"4.10.2","date_modified":"12345"},{"id_terminal":18,"id_commodity":"8","price_buy":9007199254740993.125,"price_sell":0,"scu_buy":1.5,"scu_sell_stock":0,"status_buy":2,"status_sell":0,"container_sizes":null,"game_version":"4.9","date_modified":12900}]}"#,"application/json"))
        .expect(1).mount(&server).await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.0.0"),
    )
    .unwrap();
    let rows = UexReader::new(&settings)
        .unwrap()
        .fetch_terminal_priors(&[TerminalId(17), TerminalId(18)])
        .await
        .unwrap();
    assert_eq!(
        rows.iter().map(|rows| rows.terminal()).collect::<Vec<_>>(),
        vec![TerminalId(17), TerminalId(18)]
    );
    let sell = &rows[0].rows()[0];
    assert_eq!(sell.side(), TradeSide::Sell);
    assert_eq!(sell.values().price, "97.125".parse().unwrap());
    assert_eq!(sell.values().stock, "0.0625".parse().unwrap());
    assert_eq!(sell.values().status, None);
    assert_eq!(
        sell.values().container_sizes,
        vec![ContainerSize::Scu1, ContainerSize::Scu16]
    );
    assert_eq!(
        sell.provenance().modified_at,
        Timestamp::from_seconds(12345)
    );
    assert_eq!(sell.provenance().game_version, "4.10.2");
    let buy = &rows[1].rows()[0];
    assert_eq!(buy.side(), TradeSide::Buy);
    assert_eq!(buy.values().price, "9007199254740993.125".parse().unwrap());
    assert_eq!(buy.values().stock, "1.5".parse().unwrap());
    assert_eq!(buy.values().status.unwrap().get(), 2);
    assert!(buy.values().container_sizes.is_empty());
    assert_eq!(buy.provenance().game_version, "4.9");
    let requests = server.received_requests().await.unwrap();
    assert!(!requests[0].headers.contains_key("authorization"));
    assert!(!requests[0].headers.contains_key("secret-key"));
}

#[tokio::test]
async fn r_api_5__prior_batches_accept_ten_distinct_terminals_and_refuse_invalid_requests_before_io()
 {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/commodities_prices/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"ok","data":[]})),
        )
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.0.0"),
    )
    .unwrap();
    let reader = UexReader::new(&settings).unwrap();
    let ten = (1..=10).map(TerminalId).collect::<Vec<_>>();
    let records = reader.fetch_terminal_priors(&ten).await.unwrap();
    assert_eq!(records.len(), 10);
    assert!(records.iter().all(|record| record.rows().is_empty()));
    for invalid in [
        vec![],
        (1..=11).map(TerminalId).collect(),
        vec![TerminalId(0)],
        vec![TerminalId(1), TerminalId(1)],
    ] {
        assert!(matches!(
            reader.fetch_terminal_priors(&invalid).await,
            Err(crate::api::ReadError::Malformed { .. })
        ));
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn r_api_5__malformed_or_unrequested_prior_rows_reject_the_whole_batch() {
    use serde_json::json;
    let server = MockServer::start().await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.0.0"),
    )
    .unwrap();
    let reader = UexReader::new(&settings).unwrap();
    let base = json!({"id_terminal":17,"id_commodity":7,"price_buy":0,"price_sell":10,"scu_buy":0,"scu_sell_stock":"1.25","status_buy":0,"status_sell":1,"container_sizes":"1,16","game_version":"4.10.2","date_modified":12345});
    let mut cases = Vec::new();
    for (key, value) in [
        ("id_terminal", json!(18)),
        ("id_commodity", json!(0)),
        ("price_buy", json!(1)),
        ("price_sell", json!(0)),
        ("scu_sell_stock", json!("-1")),
        ("status_sell", json!(256)),
        ("container_sizes", json!("3")),
        ("game_version", json!("")),
        ("date_modified", json!("1.5")),
    ] {
        let mut row = base.clone();
        row[key] = value;
        cases.push(json!([row]));
    }
    cases.push(json!([base.clone(), base]));
    for rows in cases {
        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/2.0/commodities_prices/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":rows})),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            reader.fetch_terminal_priors(&[TerminalId(17)]).await,
            Err(crate::api::ReadError::Malformed { .. })
        ));
    }
}

#[tokio::test]
async fn r_api_1__single_terminal_endpoint_retains_successful_empty_assortments() {
    use tallyline_reference_data::api::model::{Endpoint, EndpointData};
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/2.0/commodities_prices/"))
        .and(query_param("id_terminal", "17"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"ok","data":[]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.0.0"),
    )
    .unwrap();
    let EndpointData::TerminalPriors(priors) = UexReader::new(&settings)
        .unwrap()
        .fetch(Endpoint::TerminalPriors(TerminalId(17)))
        .await
        .unwrap()
    else {
        panic!("wrong endpoint")
    };
    assert_eq!(priors.terminal(), TerminalId(17));
    assert!(priors.rows().is_empty());
}
