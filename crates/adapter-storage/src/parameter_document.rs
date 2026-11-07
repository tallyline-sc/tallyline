// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Private persisted projection of validated reference parameters.

use crate::api::StorageError;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use tallyline_reference_data::api::model::{
    AcceptanceSwitch, CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
};

#[derive(Serialize, Deserialize)]
struct ParametersDocument {
    global: GlobalDocument,
    commodity: bool,
    price_variation_percent: u32,
    prior_ttl_days: NonZeroU32,
    scu_variation_percent: u32,
    notification: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct GlobalDocument {
    datacenter: bool,
    reports: bool,
    ptu: bool,
}

pub(crate) fn encode(parameters: &ReferenceParameters) -> Result<String, serde_json::Error> {
    let acceptance = parameters.acceptance();
    let constraints = parameters.constraints();
    serde_json::to_string(&ParametersDocument {
        global: GlobalDocument {
            datacenter: acceptance.datacenter == AcceptanceSwitch::Enabled,
            reports: acceptance.reports == AcceptanceSwitch::Enabled,
            ptu: acceptance.ptu == AcceptanceSwitch::Enabled,
        },
        commodity: acceptance.commodity == AcceptanceSwitch::Enabled,
        price_variation_percent: constraints.price_variation_percent,
        prior_ttl_days: constraints.prior_ttl_days,
        scu_variation_percent: constraints.scu_variation_percent,
        notification: parameters.staff_notification().map(str::to_owned),
    })
}
pub(crate) fn decode(payload: &str) -> Result<ReferenceParameters, StorageError> {
    let document: ParametersDocument =
        serde_json::from_str(payload).map_err(|_| StorageError::InvalidCache)?;
    Ok(ReferenceParameters::new(
        SubmissionAcceptance {
            datacenter: document.global.datacenter.into(),
            reports: document.global.reports.into(),
            commodity: document.commodity.into(),
            ptu: document.global.ptu.into(),
        },
        CommodityConstraints {
            price_variation_percent: document.price_variation_percent,
            prior_ttl_days: document.prior_ttl_days,
            scu_variation_percent: document.scu_variation_percent,
        },
        document.notification.as_deref(),
    ))
}
