// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The values the UEX client returns and submits, translated from the UEX wire format.

use serde::{Deserialize, Serialize};
use tallyline_shared_kernel::api::model::PricePerScu;

pub use crate::public_contract::{
    PublicContractCode, PublicContractEndpoint, PublicContractError, PublicContractPayload,
    PublicContractSchema, PublicContractSnapshot,
};
pub use tallyline_shared_kernel::api::model::ContainerSize;

/// The current game versions of `GET /game_versions`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameVersions {
    /// The LIVE version, not always three-part (`4.9`, `3.24.2a`).
    pub live: String,
    /// The PTU version, `None` when UEX sends an empty string or `null`.
    pub ptu: Option<String>,
}

/// A UEX terminal ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalId(pub u64);

/// A UEX commodity ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommodityId(pub u64);

/// The status levels of `GET /commodities_status`, one list per side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLevels {
    /// The buy-side levels in the order UEX sends them.
    pub buy: Vec<StatusLevel>,
    /// The sell-side levels in the order UEX sends them.
    pub sell: Vec<StatusLevel>,
}

/// One inventory status level of one side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLevel {
    /// The level as UEX numbers it, starting at 1 (out of stock).
    pub code: u8,
    /// The long name, which may differ between the sides.
    pub name: String,
    /// The short name.
    pub name_short: String,
    /// The abbreviation.
    pub name_abbr: String,
}

/// The side a terminal trades a commodity on, taken from the side with a price above zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradedSide {
    /// Only the buy price is above zero.
    Buy,
    /// Only the sell price is above zero.
    Sell,
    /// Both prices are above zero, which the documented schema does not allow.
    Both,
    /// Neither price is above zero.
    Neither,
}

/// The exact price, side, status and container sizes of a commodity-price row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriceRow {
    /// The terminal of the row.
    pub terminal: TerminalId,
    /// The commodity of the row.
    pub commodity: CommodityId,
    /// The traded side.
    pub side: TradedSide,
    /// The positive exact price, absent unless precisely one traded side exists.
    pub price: Option<PricePerScu>,
    /// The status level of the traded side, `None` when it is 0 or the side is not single.
    pub status: Option<u8>,
    /// The container sizes of this row, in the order UEX sends them; empty when none are known.
    pub container_sizes: Vec<ContainerSize>,
}

/// The account state of `GET /user`, without the personal data UEX also sends (O-97).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UexAccount {
    /// The UEX username.
    pub username: String,
    /// Whether the account is a DataRunner.
    pub is_datarunner: bool,
    /// Whether the account is banned from data running.
    pub is_datarunner_banned: bool,
    /// Whether the account belongs to UEX staff.
    pub is_staff: bool,
}

/// The answer of a `GET /data_info` access check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportAccess {
    /// The number of report rows returned, at most the requested limit.
    pub rows: usize,
}

/// One commodity price row submitted to `POST /data_submit`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SubmitPrice {
    /// UEX commodity ID.
    pub id_commodity: u64,
    /// Buy price in UEC per SCU, when this is a buy row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_buy: Option<u64>,
    /// Sell price in UEC per SCU, when this is a sell row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_sell: Option<u64>,
    /// Buy-side status when this is a buy row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_buy: Option<u8>,
    /// Sell-side status when this is a sell row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_sell: Option<u8>,
    /// Buy-side stock in SCU.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scu_buy: Option<u32>,
    /// Sell-side stock in SCU.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scu_sell: Option<u32>,
    /// Offered containers for this row, as a comma-separated list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_sizes: Option<String>,
}

/// A dry-run commodity report. `is_production` is fixed to zero by construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DryRunReport {
    /// UEX terminal ID.
    pub id_terminal: u64,
    /// UEX report type.
    #[serde(rename = "type")]
    pub report_type: String,
    /// Test mode only; this type cannot construct a production report.
    is_production: u8,
    /// Current LIVE or accepted PTU version.
    pub game_version: String,
    /// Report timestamp in Unix seconds.
    pub date_added: i64,
    /// Optional screenshot payload as provided to UEX.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    /// Submitted commodity rows.
    pub prices: Vec<SubmitPrice>,
}

impl DryRunReport {
    /// Creates a commodity report that is always sent as a UEX dry run.
    #[must_use]
    pub fn new(
        terminal: u64,
        game_version: String,
        date_added: i64,
        screenshot: Option<String>,
        prices: Vec<SubmitPrice>,
    ) -> Self {
        Self {
            id_terminal: terminal,
            report_type: "commodity".to_owned(),
            is_production: 0,
            game_version,
            date_added,
            screenshot,
            prices,
        }
    }
}

/// The success data returned by `POST /data_submit`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct SubmitSuccess {
    /// Report IDs, numeric strings aligned with submitted `prices` order.
    pub ids_reports: Vec<String>,
    /// Server timestamp.
    pub date_added: i64,
}
pub use crate::server_time::ServerTimeTracker;
pub use crate::submission::PreparedUexSubmission;
