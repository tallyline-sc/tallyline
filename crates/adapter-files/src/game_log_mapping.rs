// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finite mappings validated against paired current-build captures.

use std::collections::BTreeSet;
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId};

pub(crate) const BUILD: &str = "4.10.197.2878";
pub(crate) const BRANCH: &str = "sc-alpha-4.10.2";
pub(crate) fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("{name}[");
    let (before, after) = text.split_once(&marker)?;
    if before
        .chars()
        .last()
        .is_some_and(|last| !last.is_whitespace())
        || after.contains(&marker)
    {
        return None;
    }
    after.split_once(']').map(|(value, _)| value)
}

pub(crate) fn location_mapping(code: &str) -> Option<TerminalId> {
    match code {
        "RR_P6_LEO" => Some(TerminalId(466)),
        "RR_CRU_LEO" => Some(TerminalId(259)),
        "Nyx_Levski" => Some(TerminalId(778)),
        "RR_MIC_L2" => Some(TerminalId(55)),
        "RR_P3_LEO" => Some(TerminalId(443)),
        _ => None,
    }
}

pub(crate) fn terminal_mapping(location: Option<TerminalId>, shop: &str) -> Option<TerminalId> {
    match (location, shop) {
        (Some(TerminalId(466)), "SCShop_Pyro_RestStop_Rund_Admin") => location,
        (Some(TerminalId(259)), "SCShop_Admin_lt_base_g") => location,
        (Some(TerminalId(778)), "SCShop_Levski_CargoOffice_Commodities") => location,
        (Some(TerminalId(55)), "SCShop_Admin_lt_base_g") => location,
        (Some(TerminalId(443)), "SCShop_Pyro_RestStop_Rund_Admin") => location,
        _ => None,
    }
}

pub(crate) fn commodity_mapping(name: &str) -> Option<CommodityId> {
    match name {
        "ResourceType.PlasmaFuel" => Some(CommodityId(104)),
        "ResourceType.QuantumFuel" => Some(CommodityId(87)),
        "ResourceType.RMC" => Some(CommodityId(63)),
        "ResourceType.AltruciaToxin_unprocessed" => Some(CommodityId(66)),
        "ResourceType.Astatine" => Some(CommodityId(9)),
        "ResourceType.Carbon" => Some(CommodityId(102)),
        "ResourceType.Copper" => Some(CommodityId(20)),
        "ResourceType.Diamond" => Some(CommodityId(25)),
        "ResourceType.Potassium" => Some(CommodityId(116)),
        "ResourceType.Scrap" => Some(CommodityId(67)),
        "ResourceType.Waste" => Some(CommodityId(79)),
        _ => None,
    }
}

pub(crate) fn sizes(text: &str) -> Option<BTreeSet<u8>> {
    let (_, values) = text.split_once("Available Box Sizes:")?;
    let mut sizes = BTreeSet::new();
    for value in values.split("boxSize[").skip(1) {
        let (number, _) = value.split_once(']')?;
        let size = number.parse().ok()?;
        if ![1, 2, 4, 8, 16, 24, 32].contains(&size) || !sizes.insert(size) {
            return None;
        }
    }
    if sizes.is_empty() {
        return None;
    }
    Some(sizes)
}
