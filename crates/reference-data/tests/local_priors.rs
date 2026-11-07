// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Accepted local records remain distinct from usable environment-specific UEX references.

#[path = "local_priors/history.rs"]
mod history;
#[path = "local_priors/identity.rs"]
mod identity;
#[path = "local_priors/restoration.rs"]
mod restoration;
use std::{num::NonZeroU8, sync::Arc};
use tallyline_reference_data::api::model::{
    AcceptedLocalPrior, AcceptedPriorContext, CachedReference, EndpointData, PricePrior,
    PriorProvenance, PriorReferenceSource, PriorValues, TerminalPriors,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_shared_kernel::api::{
    Clock,
    model::{
        CommodityId, ReportId, SubmissionJobId, TerminalId, Timestamp, TradeSide, UexEnvironment,
    },
};
struct Time;
impl Clock for Time {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(120)
    }
}
struct Cache(Vec<CachedReference>);
impl ReferenceDataCache for Cache {
    type Error = ();
    fn load(&mut self) -> Result<Vec<CachedReference>, ()> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, entry: &CachedReference) -> Result<(), ()> {
        self.0.push(entry.clone());
        Ok(())
    }
}
fn values(price: &str) -> Option<PriorValues> {
    Some(PriorValues {
        price: price.parse().ok()?,
        stock: "0".parse().ok()?,
        status: NonZeroU8::new(1),
        container_sizes: vec![],
    })
}
fn local(environment: UexEnvironment) -> Option<AcceptedLocalPrior> {
    AcceptedLocalPrior::new(
        Arc::new(AcceptedPriorContext {
            report: ReportId(9),
            report_version: 3,
            submission: SubmissionJobId(7),
            environment,
            terminal: TerminalId(89),
            side: TradeSide::Sell,
            game_version: "4.10.2".to_owned(),
            observed_at: Timestamp::from_seconds(100),
            accepted_at: Timestamp::from_seconds(110),
        }),
        CommodityId(7),
        values("125")?,
    )
    .ok()
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_7_r_api_6__local_accepted_provenance_is_explicit_and_never_becomes_a_ptu_price_reference()
{
    let prior = PricePrior::new(
        CommodityId(7),
        TradeSide::Sell,
        values("120").unwrap(),
        PriorProvenance {
            game_version: "4.10.1".to_owned(),
            modified_at: Timestamp::from_seconds(90),
        },
    )
    .unwrap();
    let cache = Cache(vec![CachedReference::new(
        Timestamp::from_seconds(95),
        EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(89), vec![prior]).unwrap()),
    )]);
    let application = ReferenceApplication::load(cache, Time).unwrap();
    let base = application.snapshot(UexEnvironment::Live);
    let selected = base
        .prior_reference(TerminalId(89), CommodityId(7), TradeSide::Sell)
        .unwrap();
    assert_eq!(
        (
            selected.values(),
            selected.source(),
            selected.game_version(),
            selected.modified_at(),
            selected.local_source()
        ),
        (
            &values("120").unwrap(),
            PriorReferenceSource::Uex,
            "4.10.1",
            Timestamp::from_seconds(90),
            None
        )
    );
    let records = vec![
        local(UexEnvironment::Live).unwrap(),
        local(UexEnvironment::Ptu).unwrap(),
    ];
    let live = base.clone().with_local_priors(&records).unwrap();
    let selected = live
        .prior_reference(TerminalId(89), CommodityId(7), TradeSide::Sell)
        .unwrap();
    assert_eq!(
        (
            selected.values(),
            selected.source(),
            selected.game_version(),
            selected.modified_at(),
            selected.local_source()
        ),
        (
            &values("125").unwrap(),
            PriorReferenceSource::AcceptedLocal,
            "4.10.2",
            Timestamp::from_seconds(100),
            Some(&AcceptedPriorContext {
                report: ReportId(9),
                report_version: 3,
                submission: SubmissionJobId(7),
                environment: UexEnvironment::Live,
                terminal: TerminalId(89),
                side: TradeSide::Sell,
                game_version: "4.10.2".to_owned(),
                observed_at: Timestamp::from_seconds(100),
                accepted_at: Timestamp::from_seconds(110),
            })
        )
    );
    assert_eq!(
        live.terminal_priors(TerminalId(89)).unwrap().rows()[0]
            .values()
            .price
            .to_string(),
        "120"
    );
    let ptu = application
        .snapshot(UexEnvironment::Ptu)
        .with_local_priors(&records)
        .unwrap();
    assert!(
        ptu.prior_reference(TerminalId(89), CommodityId(7), TradeSide::Sell)
            .is_none()
    );
    assert!(ptu.terminal_priors(TerminalId(89)).is_none());
    assert!(
        base.with_local_priors(&[records[0].clone(), records[0].clone()])
            .is_err()
    );
}
