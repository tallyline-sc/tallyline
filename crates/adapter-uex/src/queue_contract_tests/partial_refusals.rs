// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Row refusal semantics and positive safety proofs through isolated HTTP.

use crate::api::{ClientIdentity, ReaderSettings, UexReader};
use serde_json::json;
use tallyline_shared_kernel::api::model::{SubmissionMode, Timestamp};
use tallyline_submission::api::SubmissionGateway;
use tallyline_submission::api::model::{CredentialValues, SubmissionRequest};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9__positive_no_row_refusal_safety_read_retains_exact_row_identity_without_second_get()
 {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/2.0/data_submit/"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"status":"vendor_row_refusal","data":null})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path("/2.0/data_info/")).and(query_param("username","dummy-a")).and(query_param("id_terminal","89")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok","data":[{"id":718,"type":"commodity","id_terminal":89,"id_commodity":7,"date_added":100}]}))).expect(1).mount(&server).await;
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
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await;
    let job = super::accepted_job(&request, result, Timestamp::from_seconds(100)).unwrap();
    assert_eq!(
        job.verified_row_receipts()
            .map(|proof| (proof.row().0, proof.id(), proof.date_added().seconds()))
            .collect::<Vec<_>>(),
        vec![(1, 718, 100)],
        "the original safety GET must retain its actual proof, without another network read"
    );
    server.verify().await;
}
async fn row_refusal(
    code: &str,
    data: serde_json::Value,
) -> tallyline_submission::api::model::SubmissionResult {
    use tallyline_shared_kernel::api::model::{CommodityId, RowId};
    use tallyline_submission::api::model::SubmissionRow;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({"status":code,"data":data})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
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
    let mut header = super::released_request().unwrap().header().clone();
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
    let credentials =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    let result = reader
        .submit(reader.prepare(&request).unwrap().0, &credentials, None)
        .await;
    server.verify().await;
    result
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9_r_sub_11__named_failure_retains_index_or_exact_commodity_provenance() {
    use tallyline_submission::api::model::{RowFailureSource, SubmissionResult};
    for (data, expected) in [
        (json!({"row":1}), RowFailureSource::Index),
        (
            json!({"row":1,"reset":1793858400_u64}),
            RowFailureSource::Index,
        ),
        (
            json!({"report":{"id_terminal":89,"id_commodity":8},"reset":1793858400_u64}),
            RowFailureSource::Commodity,
        ),
        (
            json!({"report":{"id_terminal":89,"id_commodity":8}}),
            RowFailureSource::Commodity,
        ),
    ] {
        let result = row_refusal("invalid_status_sell", data).await;
        let SubmissionResult::PartialError {
            row, source, code, ..
        } = result
        else {
            panic!("named failure must retain partial evidence");
        };
        assert_eq!(row, Some(1));
        assert_eq!(source, expected);
        assert_eq!(
            code.row_failure(),
            tallyline_submission::api::model::RowFailureKind::InvalidSellStatus
        );
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9_r_sub_11__documented_row_refusals_retain_closed_causes_and_original_codes() {
    use tallyline_submission::api::model::{
        RowFailureKind, RowFailureSource, RowFieldConflict, SubmissionResult,
    };
    for (technical, expected) in [
        ("database_error", RowFailureKind::Database),
        ("duplicated_report", RowFailureKind::Duplicate),
        ("invalid_prices_array", RowFailureKind::InvalidPrices),
        ("invalid_prices_array_format", RowFailureKind::InvalidPrices),
        (
            "reference_key_not_supplied",
            RowFailureKind::MissingReference,
        ),
        ("no_commodities_found", RowFailureKind::NoCommodities),
        ("missing_id_commodity", RowFailureKind::MissingCommodity),
        ("invalid_id_commodity", RowFailureKind::InvalidCommodity),
        (
            "has_no_prices_and_no_is_missing_set",
            RowFailureKind::MissingPrice,
        ),
        (
            "has_both_price_buy_and_price_sell",
            RowFailureKind::ConflictingSides(RowFieldConflict::Prices),
        ),
        (
            "has_both_scu_buy_and_scu_sell",
            RowFailureKind::ConflictingSides(RowFieldConflict::Quantities),
        ),
        (
            "cannot_have_both_status_buy_and_status_sell",
            RowFailureKind::ConflictingSides(RowFieldConflict::Statuses),
        ),
        (
            "cannot_have_both_price_buy_and_status_sell",
            RowFailureKind::ConflictingSides(RowFieldConflict::BuyPriceSellStatus),
        ),
        (
            "cannot_have_both_price_buy_and_scu_sell",
            RowFailureKind::ConflictingSides(RowFieldConflict::BuyPriceSellQuantity),
        ),
        (
            "cannot_have_both_price_sell_and_status_buy",
            RowFailureKind::ConflictingSides(RowFieldConflict::SellPriceBuyStatus),
        ),
        (
            "cannot_have_both_price_sell_and_scu_buy",
            RowFailureKind::ConflictingSides(RowFieldConflict::SellPriceBuyQuantity),
        ),
        ("invalid_status_buy", RowFailureKind::InvalidBuyStatus),
        ("invalid_status_sell", RowFailureKind::InvalidSellStatus),
        ("invalid_quality", RowFailureKind::InvalidQuality),
        ("Future_Code", RowFailureKind::Unrecognised),
    ] {
        let result = row_refusal(technical, json!({"row":1})).await;
        let SubmissionResult::PartialError {
            code, row, source, ..
        } = result
        else {
            panic!("named row refusal must remain partial: {technical}");
        };
        assert_eq!(row, Some(1));
        assert_eq!(source, RowFailureSource::Index);
        assert_eq!(code.as_str(), technical);
        assert_eq!(code.row_failure(), expected);
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_sub_9_r_sub_11__missing_malformed_or_foreign_failure_positions_never_fabricate_row_zero()
{
    use tallyline_submission::api::model::{RowFailureSource, SubmissionResult};
    for code in [
        "database_error",
        "invalid_prices_array_format",
        "reference_key_not_supplied",
    ] {
        for data in [
            serde_json::Value::Null,
            json!({"row":999}),
            json!({"row":"1"}),
            json!({"row":-1}),
            json!({"report":{"id_terminal":90,"id_commodity":8}}),
            json!({"report":{"id_terminal":89,"id_commodity":99}}),
        ] {
            let result = row_refusal(code, data).await;
            let SubmissionResult::PartialError {
                row,
                source,
                receipts,
                ..
            } = result
            else {
                panic!("unidentified row-processing failure must remain partial: {code}");
            };
            assert_eq!(row, None);
            assert_eq!(source, RowFailureSource::Unmapped);
            assert!(receipts.is_empty());
        }
    }
}
