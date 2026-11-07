// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact manual price review through the public Reporting application.

#[path = "price_review/boundaries.rs"]
mod boundaries;
#[path = "price_review/policy.rs"]
mod policy;
#[path = "price_review/references.rs"]
mod references;
mod support;

use tallyline_reference_data::api::model::{
    AcceptanceSwitch, CommodityConstraints, EndpointData, PricePrior, PriorProvenance, PriorValues,
    ReferenceParameters, ReferenceSnapshot, SubmissionAcceptance, TerminalPriors, UexEnvironment,
};
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp, TradeSide};
fn price_reference(
    price: &str,
    percent: u32,
) -> Result<ReferenceSnapshot, Box<dyn std::error::Error>> {
    let original = support::snapshot(UexEnvironment::Live)?;
    let original = original
        .terminal_priors(TerminalId(11))
        .and_then(|priors| priors.rows().first())
        .ok_or("missing fixture prior")?;
    let prior = PricePrior::new(
        CommodityId(42),
        TradeSide::Buy,
        PriorValues {
            price: price.parse()?,
            stock: original.values().stock,
            status: original.values().status,
            container_sizes: vec![],
        },
        PriorProvenance {
            game_version: "4.10.2".to_owned(),
            modified_at: Timestamp::from_seconds(900),
        },
    )?;
    Ok(support::cached_snapshot(
        UexEnvironment::Live,
        vec![
            EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(11), vec![prior])?),
            EndpointData::Parameters(ReferenceParameters::new(
                SubmissionAcceptance {
                    datacenter: AcceptanceSwitch::Enabled,
                    reports: AcceptanceSwitch::Enabled,
                    commodity: AcceptanceSwitch::Enabled,
                    ptu: AcceptanceSwitch::Disabled,
                },
                CommodityConstraints {
                    price_variation_percent: percent,
                    prior_ttl_days: std::num::NonZeroU32::new(15).ok_or("invalid fixture TTL")?,
                    scu_variation_percent: 5000,
                },
                None,
            )),
        ],
    ))
}
