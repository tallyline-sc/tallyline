// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference freshness through the drawn global status.

use egui_kittest::kittest::Queryable;
use tallyline_reference_data::api::model::{CachedReference, EndpointData, UexEnvironment};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_ui::api::model::BackgroundEvent;

struct Cache;
impl ReferenceDataCache for Cache {
    type Error = &'static str;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(vec![CachedReference::new(
            Timestamp::from_seconds(60),
            EndpointData::Commodities(Vec::new()),
        )])
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(120)
    }
}

#[test]
fn reference_age_is_visible_without_expanding_details_and_clock_mismatch_is_explicit() {
    let app = ReferenceApplication::load(Cache, FixedClock).unwrap();
    let mut harness = super::shell(egui::vec2(800.0, 640.0)).unwrap();
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::ReferenceUpdated {
            live: app.snapshot(UexEnvironment::Live),
            ptu: app.snapshot(UexEnvironment::Ptu),
            observed_at: Timestamp::from_seconds(120),
        });
    harness.run();
    let age = harness
        .get_by_label("Oldest reference age at last update: 60 seconds")
        .rect();
    assert!(age.top() > harness.get_by_label("Search terminals").rect().bottom());
    assert!(age.bottom() <= 640.0);
    assert!(
        harness
            .query_by_label("Commodities: Age at last update: 60 seconds")
            .is_none()
    );
    harness
        .state_mut()
        .shell
        .receive(BackgroundEvent::ReferenceUpdated {
            live: app.snapshot(UexEnvironment::Live),
            ptu: app.snapshot(UexEnvironment::Ptu),
            observed_at: Timestamp::from_seconds(30),
        });
    harness.run();
    assert!(
        harness
            .query_by_label("Age unknown - clock mismatch")
            .is_some()
    );
}

pub(super) fn reference_event() -> Result<BackgroundEvent, String> {
    let app =
        ReferenceApplication::load(Cache, FixedClock).map_err(|error| format!("{error:?}"))?;
    Ok(BackgroundEvent::ReferenceUpdated {
        live: app.snapshot(UexEnvironment::Live),
        ptu: app.snapshot(UexEnvironment::Ptu),
        observed_at: Timestamp::from_seconds(120),
    })
}
