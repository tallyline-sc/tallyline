// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict semantic error-code persistence with legacy identifier admission.

use serde::{Deserialize, Serialize};
use tallyline_submission::api::model::{
    RowFailureKind, RowFieldConflict, SubmissionRepositoryError, UexErrorCode,
};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ErrorCodeDocument {
    Classified(ClassifiedCodeDocument),
    Legacy(String),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClassifiedCodeDocument {
    value: String,
    #[serde(with = "RowFailureKindDocument")]
    row_failure: RowFailureKind,
}
impl From<&UexErrorCode> for ErrorCodeDocument {
    fn from(code: &UexErrorCode) -> Self {
        Self::Classified(ClassifiedCodeDocument {
            value: code.as_str().to_owned(),
            row_failure: code.row_failure(),
        })
    }
}
impl ErrorCodeDocument {
    pub(crate) fn restore(self) -> Result<UexErrorCode, SubmissionRepositoryError> {
        let (value, kind) = match self {
            Self::Classified(code) => (code.value, code.row_failure),
            Self::Legacy(value) => {
                let kind = crate::legacy_row_failure::classify(&value);
                (value, kind)
            }
        };
        UexErrorCode::classified(value, kind).ok_or(SubmissionRepositoryError::Invalid)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "RowFailureKind", rename_all = "snake_case")]
enum RowFailureKindDocument {
    Database,
    Duplicate,
    InvalidPrices,
    MissingReference,
    NoCommodities,
    MissingCommodity,
    InvalidCommodity,
    MissingPrice,
    ConflictingSides(#[serde(with = "RowFieldConflictDocument")] RowFieldConflict),
    InvalidBuyStatus,
    InvalidSellStatus,
    InvalidQuality,
    Unrecognised,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "RowFieldConflict", rename_all = "snake_case")]
enum RowFieldConflictDocument {
    Prices,
    Quantities,
    Statuses,
    BuyPriceSellStatus,
    BuyPriceSellQuantity,
    SellPriceBuyStatus,
    SellPriceBuyQuantity,
}
