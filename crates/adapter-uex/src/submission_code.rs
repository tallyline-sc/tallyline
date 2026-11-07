// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! UEX technical row identifiers translated into Submission causes.

use tallyline_submission::api::model::{RowFailureKind, RowFieldConflict};
pub(crate) fn row_failure(code: &str) -> RowFailureKind {
    match code {
        "database_error" => RowFailureKind::Database,
        "duplicated_report" => RowFailureKind::Duplicate,
        "invalid_prices_array" | "invalid_prices_array_format" => RowFailureKind::InvalidPrices,
        "reference_key_not_supplied" => RowFailureKind::MissingReference,
        "no_commodities_found" => RowFailureKind::NoCommodities,
        "missing_id_commodity" => RowFailureKind::MissingCommodity,
        "invalid_id_commodity" => RowFailureKind::InvalidCommodity,
        "has_no_prices_and_no_is_missing_set" => RowFailureKind::MissingPrice,
        "has_both_price_buy_and_price_sell" => {
            RowFailureKind::ConflictingSides(RowFieldConflict::Prices)
        }
        "has_both_scu_buy_and_scu_sell" => {
            RowFailureKind::ConflictingSides(RowFieldConflict::Quantities)
        }
        "cannot_have_both_status_buy_and_status_sell" => {
            RowFailureKind::ConflictingSides(RowFieldConflict::Statuses)
        }
        "cannot_have_both_price_buy_and_status_sell" => {
            RowFailureKind::ConflictingSides(RowFieldConflict::BuyPriceSellStatus)
        }
        "cannot_have_both_price_buy_and_scu_sell" => {
            RowFailureKind::ConflictingSides(RowFieldConflict::BuyPriceSellQuantity)
        }
        "cannot_have_both_price_sell_and_status_buy" => {
            RowFailureKind::ConflictingSides(RowFieldConflict::SellPriceBuyStatus)
        }
        "cannot_have_both_price_sell_and_scu_buy" => {
            RowFailureKind::ConflictingSides(RowFieldConflict::SellPriceBuyQuantity)
        }
        "invalid_status_buy" => RowFailureKind::InvalidBuyStatus,
        "invalid_status_sell" => RowFailureKind::InvalidSellStatus,
        "invalid_quality" => RowFailureKind::InvalidQuality,
        _ => RowFailureKind::Unrecognised,
    }
}
