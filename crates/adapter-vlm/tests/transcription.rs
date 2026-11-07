// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public transcription contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_vlm::api as vlm_api;
use tallyline_recognition::api as recognition_api;
use tallyline_recognition::api::model::RawField;
use tallyline_shared_kernel::api::model::TradeSide;

const NAMED: &str = "TAB: Buy\nLOCATION: PATCH CITY\n| name | status | scu | price_per_scu | cargo_sizes |\n|---|---|---|---|---|\n| Omnapoxy | Medium Inventory | 333 | 3,237 | 1,2,4,8,16 |";

#[test]
fn named_example_preserves_column_ownership() -> Result<(), Box<dyn std::error::Error>> {
    let result = vlm_api::parse_answer(NAMED)?;
    assert_eq!(result.side, RawField::Read(TradeSide::Buy));
    assert_eq!(result.location, RawField::Read("PATCH CITY".to_owned()));
    assert_eq!(result.cards.len(), 1);
    let card = result.cards.first().ok_or("missing named card")?;
    assert_eq!(card.name, RawField::Read("Omnapoxy".to_owned()));
    assert_eq!(card.quantity_text, RawField::Read("333".to_owned()));
    assert_eq!(card.price_text, RawField::Read("3,237".to_owned()));
    Ok(())
}

#[test]
fn r_vlm_10__unreadable_marks_never_become_default_values() -> Result<(), Box<dyn std::error::Error>>
{
    let answer = NAMED
        .replace("Buy", "?")
        .replace("PATCH CITY", "?")
        .replace("333", "?")
        .replace("3,237", "?");
    let result = vlm_api::parse_answer(&answer)?;
    assert_eq!(result.side, RawField::Unreadable);
    assert_eq!(result.location, RawField::Unreadable);
    let card = result.cards.first().ok_or("missing unreadable card")?;
    assert_eq!(card.quantity_text, RawField::Unreadable);
    assert_eq!(card.price_text, RawField::Unreadable);
    Ok(())
}

#[test]
fn r_vlm_10__malformed_or_partial_answers_return_no_rows() {
    for answer in [
        NAMED.replace("TAB: Buy", "TAB: purchase"),
        NAMED.replace("LOCATION: PATCH CITY", "LOCATION: "),
        NAMED.replace("price_per_scu", "price"),
        NAMED.replace("| scu | price_per_scu |", "| price_per_scu | scu |"),
        NAMED.replace("|---|---|---|---|---|", "|---|---|---|---|"),
        NAMED.trim_end_matches('|').to_owned(),
        format!("{NAMED}\nThe result is certain."),
        format!("{NAMED}\nTAB: Sell"),
        NAMED.replace("Medium Inventory", ""),
        format!("```markdown\n{NAMED}\n```"),
        format!("{NAMED}\n| Omnapoxy | Medium Inventory | 333 | 3,237 | 1 |"),
        NAMED.replace("Omnapoxy", &"x".repeat(513)),
        "x".repeat(262_145),
    ] {
        assert!(
            vlm_api::parse_answer(&answer).is_err(),
            "accepted malformed answer: {answer}"
        );
    }
}

#[test]
fn row_limit_rejects_the_entire_answer() {
    let header = "TAB: Sell\nLOCATION: PATCH CITY\n|name|status|scu|price_per_scu|cargo_sizes|\n|---|---|---|---|---|";
    let rows: String = (0..129)
        .map(|index| format!("\n| Commodity {index} | ? | ? | ? | ? |"))
        .collect();
    assert!(vlm_api::parse_answer(&format!("{header}{rows}")).is_err());
}

#[test]
fn r_vlm_10__truncation_retries_once_with_explicit_contract()
-> Result<(), Box<dyn std::error::Error>> {
    use tallyline_adapter_vlm::api::{FinishReason, ModelAnswer};
    let mut calls = Vec::new();
    let result = vlm_api::parse_with_retry(|request| {
        calls.push((
            request.prompt(),
            request.temperature(),
            request.keep_alive(),
            request.token_limit(),
        ));
        Ok::<_, &'static str>(ModelAnswer {
            text: if calls.len() == 1 {
                "TAB: Sell".to_owned()
            } else {
                NAMED.to_owned()
            },
            finish: if calls.len() == 1 {
                FinishReason::Length
            } else {
                FinishReason::Complete
            },
        })
    })?;
    assert_eq!(result.cards.len(), 1);
    assert_eq!(calls.len(), 2);
    let first = calls.first().ok_or("missing first call")?;
    let second = calls.get(1).ok_or("missing retry")?;
    assert_eq!(first.1, 0);
    assert_eq!(first.2, std::time::Duration::from_secs(300));
    assert_eq!(first.3, 4096);
    assert_eq!(second.3, 8192);
    assert_eq!(first.0, second.0);
    assert!(first.0.contains("correct nothing"));
    assert!(!first.0.contains("Omnapoxy"));
    Ok(())
}

#[test]
fn r_vlm_10__terminal_failures_do_not_return_partial_rows_or_retry() {
    use tallyline_adapter_vlm::api::{FinishReason, ModelAnswer, TranscriptionFailure};
    let mut calls = 0;
    let truncated = vlm_api::parse_with_retry(|_| {
        calls += 1;
        Ok::<_, &'static str>(ModelAnswer {
            text: NAMED.to_owned(),
            finish: FinishReason::Length,
        })
    });
    assert_eq!(truncated, Err(TranscriptionFailure::Truncated));
    assert_eq!(calls, 2);
    calls = 0;
    let malformed = vlm_api::parse_with_retry(|_| {
        calls += 1;
        Ok::<_, &'static str>(ModelAnswer {
            text: "TAB: Sell".to_owned(),
            finish: FinishReason::Complete,
        })
    });
    assert!(matches!(
        malformed,
        Err(TranscriptionFailure::Unparseable(_))
    ));
    assert_eq!(calls, 1);
    calls = 0;
    let failed = vlm_api::parse_with_retry(|_| {
        calls += 1;
        Err::<ModelAnswer, _>("unavailable")
    });
    assert_eq!(failed, Err(TranscriptionFailure::Backend("unavailable")));
    assert_eq!(calls, 1);
}

#[test]
fn r_vlm_10__complete_first_answer_never_retries() {
    use tallyline_adapter_vlm::api::{FinishReason, ModelAnswer};
    let mut calls = 0;
    let result = vlm_api::parse_with_retry(|request| {
        calls += 1;
        assert_eq!(request.prompt_version(), "shop_panel_v1");
        Ok::<_, &'static str>(ModelAnswer {
            text: NAMED.to_owned(),
            finish: FinishReason::Complete,
        })
    });
    assert!(result.is_ok());
    assert_eq!(calls, 1);
}

#[test]
fn r_vlm_5__named_table_runs_through_shared_numeric_parser()
-> Result<(), Box<dyn std::error::Error>> {
    use tallyline_recognition::api::model::{DecimalPrices, NumericReading};
    use tallyline_shared_kernel::api::model::{PricePerScu, ScuQuantity};
    let result = vlm_api::parse_answer(NAMED)?;
    let card = result.cards.first().ok_or("missing named row")?;
    assert_eq!(
        recognition_api::parse_quantity_text(&card.quantity_text),
        NumericReading::Candidates {
            raw: Some(ScuQuantity::whole(333)),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    assert_eq!(
        recognition_api::parse_price_text(&card.price_text, DecimalPrices::WholeOnly),
        NumericReading::Candidates {
            raw: Some("3237".parse::<PricePerScu>()?),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    Ok(())
}

#[test]
fn r_vlm_5__magnitude_prices_allow_an_ungrouped_decimal_mantissa()
-> Result<(), Box<dyn std::error::Error>> {
    use tallyline_recognition::api::model::{DecimalPrices, NumericReading};
    use tallyline_shared_kernel::api::model::PricePerScu;
    let answer = vlm_api::parse_answer(&NAMED.replace("3,237", "96705.5K"))?;
    let card = answer.cards.first().ok_or("missing decimal-price row")?;
    assert_eq!(
        recognition_api::parse_price_text(&card.price_text, DecimalPrices::WholeOnly),
        NumericReading::Candidates {
            raw: Some("96705500".parse::<PricePerScu>()?),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    Ok(())
}
