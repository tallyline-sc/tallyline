// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sanitised game-log parsing state.

use crate::game_log_mapping::{self, BRANCH, BUILD, field};
use tallyline_game::api::model::{ShopInventoryFact, WitnessFacts, WitnessUnavailable};
use tallyline_shared_kernel::api::model::{TerminalId, Timestamp};

#[derive(Default)]
pub(crate) struct LogParser {
    rows: Vec<ShopInventoryFact>,
    location: Option<(Timestamp, Option<TerminalId>)>,
    build_validated: bool,
    branch_validated: bool,
}
impl LogParser {
    pub(crate) fn line(&mut self, text: &str) -> Result<(), WitnessUnavailable> {
        self.header(text)?;
        if text.contains("<RequestLocationInventory>") {
            let time = crate::log_time::parse(text).ok_or(WitnessUnavailable::UnvalidatedFormat)?;
            self.location = Some((
                time,
                field(text, "Location").and_then(game_log_mapping::location_mapping),
            ));
        }
        if text.contains("CEntityComponentCommodityUIProvider::LoadShopInventoryData") {
            self.row(text)?;
        }
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<WitnessFacts, WitnessUnavailable> {
        if !self.build_validated || !self.branch_validated {
            return Err(WitnessUnavailable::UnvalidatedFormat);
        }
        Ok(WitnessFacts::new(self.rows))
    }
    fn header(&mut self, text: &str) -> Result<(), WitnessUnavailable> {
        let header = if text.starts_with('<') {
            text.split_once("> ").map_or(text, |(_, payload)| payload)
        } else {
            text
        };
        for (label, expected, validated) in [
            ("FileVersion: ", BUILD, &mut self.build_validated),
            ("Branch: ", BRANCH, &mut self.branch_validated),
        ] {
            if let Some(value) = header.strip_prefix(label) {
                if header != text {
                    crate::log_time::parse(text).ok_or(WitnessUnavailable::UnvalidatedFormat)?;
                }
                if value != expected {
                    return Err(WitnessUnavailable::UnvalidatedFormat);
                }
                *validated = true;
            }
        }
        Ok(())
    }
    fn row(&mut self, text: &str) -> Result<(), WitnessUnavailable> {
        if !text.contains("AddingCommodityBox")
            || !self.build_validated
            || !self.branch_validated
            || !text.contains("[Notice]")
            || !text.contains("Available Box Sizes:")
        {
            return Err(WitnessUnavailable::UnvalidatedFormat);
        }
        let time = crate::log_time::parse(text).ok_or(WitnessUnavailable::UnvalidatedFormat)?;
        let shop = field(text, "shopName").ok_or(WitnessUnavailable::UnvalidatedFormat)?;
        let commodity = game_log_mapping::commodity_mapping(
            field(text, "commodityName").ok_or(WitnessUnavailable::UnvalidatedFormat)?,
        );
        let sizes = game_log_mapping::sizes(text).ok_or(WitnessUnavailable::UnvalidatedFormat)?;
        let terminal = game_log_mapping::terminal_mapping(
            self.location
                .filter(|(location_time, _)| *location_time <= time)
                .and_then(|(_, id)| id),
            shop,
        );
        self.rows
            .push(ShopInventoryFact::new(time, terminal, commodity, sizes));
        Ok(())
    }
}
