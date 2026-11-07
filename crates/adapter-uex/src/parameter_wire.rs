// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Submission parameters translated independently of browsing availability.

use crate::api::ReadError;
use crate::wire::{Numeric, malformed, parse, unescaped};
use serde::Deserialize;
use serde_json::Value;
use std::num::NonZeroU32;
use tallyline_reference_data::api::model::{
    CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
};

#[derive(Deserialize)]
struct ParametersWire {
    global: GlobalWire,
    commodity: CommodityWire,
}
#[derive(Deserialize)]
struct GlobalWire {
    is_datacenter_enabled: Numeric,
    is_accepting_reports: Numeric,
    is_accepting_ptu_reports: Numeric,
}
#[derive(Deserialize)]
struct CommodityWire {
    is_accepted: Numeric,
    price_variation: Numeric,
    ttl: Numeric,
    scu_variation: Numeric,
    notification: Option<String>,
}

pub(crate) fn parameters(data: Value) -> Result<ReferenceParameters, ReadError> {
    let wire: ParametersWire = parse(data, "data_parameters")?;
    let acceptance = SubmissionAcceptance {
        datacenter: (wire
            .global
            .is_datacenter_enabled
            .flag("is_datacenter_enabled")?)
        .into(),
        reports: (wire
            .global
            .is_accepting_reports
            .flag("is_accepting_reports")?)
        .into(),
        commodity: (wire.commodity.is_accepted.flag("is_accepted")?).into(),
        ptu: (wire
            .global
            .is_accepting_ptu_reports
            .flag("is_accepting_ptu_reports")?)
        .into(),
    };
    let constraints = CommodityConstraints {
        price_variation_percent: whole(&wire.commodity.price_variation, "price_variation")?,
        prior_ttl_days: NonZeroU32::new(whole(&wire.commodity.ttl, "ttl")?)
            .ok_or_else(|| malformed("commodity ttl is zero"))?,
        scu_variation_percent: whole(&wire.commodity.scu_variation, "scu_variation")?,
    };
    let notification = wire.commodity.notification.map(|text| unescaped(&text));
    Ok(ReferenceParameters::new(
        acceptance,
        constraints,
        notification.as_deref(),
    ))
}
fn whole(value: &Numeric, field: &str) -> Result<u32, ReadError> {
    u32::try_from(value.whole(field)?)
        .map_err(|_| malformed(&format!("{field} exceeds the supported range")))
}
