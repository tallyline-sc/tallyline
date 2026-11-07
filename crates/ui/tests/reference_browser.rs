// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference browsing through public shell state and commands.

use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, EndpointData, UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, Timestamp};
use tallyline_ui::api::ShellState;
use tallyline_ui::api::model::{BackgroundEvent, View};

mod resources;

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(120)
    }
}
struct MemoryCache(Vec<CachedReference>);
impl ReferenceDataCache for MemoryCache {
    type Error = &'static str;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[path = "reference_browser/drawn.rs"]
mod drawn;
#[path = "reference_browser/state.rs"]
mod state;
#[path = "reference_browser/status.rs"]
mod status;
