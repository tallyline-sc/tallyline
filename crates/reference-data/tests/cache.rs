// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference application acceptance tests.

use tallyline_reference_data::api::model::{
    CachedReference, Commodity, CommodityAverages, Endpoint, EndpointData, ReferenceState,
    UexEnvironment,
};
use tallyline_reference_data::api::{ReferenceApplication, ReferenceDataCache};
use tallyline_shared_kernel::api::Clock;
use tallyline_shared_kernel::api::model::{CommodityId, Timestamp, TradeSide};

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(7_200)
    }
}

struct MemoryCache(Vec<CachedReference>);
impl ReferenceDataCache for MemoryCache {
    type Error = &'static str;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(self.0.clone())
    }
    fn replace(&mut self, entry: &CachedReference) -> Result<(), Self::Error> {
        self.0.retain(|old| old.endpoint() != entry.endpoint());
        self.0.push(entry.clone());
        Ok(())
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_ui_13__expired_cached_vocabulary_is_searchable_before_refresh_and_survives_failure() {
    let cached = CachedReference::new(
        Timestamp::from_seconds(100),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(42),
            "Agricium",
            "AGRI",
            true,
            CommodityAverages::new(Some("120".parse().unwrap()), None),
        )]),
    );
    let mut application =
        ReferenceApplication::load(MemoryCache(vec![cached]), FixedClock).unwrap();
    assert_eq!(
        application.commodities(UexEnvironment::Live, "agri").len(),
        1
    );
    assert!(application.due().contains(&Endpoint::Commodities));
    application.refresh_failed(Endpoint::Commodities);
    assert_eq!(application.state(), ReferenceState::Offline);
    assert_eq!(
        application
            .commodities(UexEnvironment::Live, "agri")
            .first()
            .unwrap()
            .id(),
        CommodityId(42)
    );
    assert_eq!(
        application.fetched_at(Endpoint::Commodities),
        Some(Timestamp::from_seconds(100))
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_13__empty_offline_cache_has_an_explicit_no_reference_data_state() {
    let mut application = ReferenceApplication::load(MemoryCache(vec![]), FixedClock).unwrap();
    assert_eq!(application.state(), ReferenceState::NoReferenceData);
    application.refresh_failed(Endpoint::Commodities);
    assert_eq!(application.state(), ReferenceState::NoReferenceData);
    assert!(application.commodities(UexEnvironment::Live, "").is_empty());
}

struct ReadOnlyCache;
impl ReferenceDataCache for ReadOnlyCache {
    type Error = &'static str;
    fn load(&mut self) -> Result<Vec<CachedReference>, Self::Error> {
        Ok(vec![])
    }
    fn replace(&mut self, _: &CachedReference) -> Result<(), Self::Error> {
        Err("read-only")
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__successful_refresh_becomes_visible_only_after_atomic_persistence() {
    let mut application = ReferenceApplication::load(ReadOnlyCache, FixedClock).unwrap();
    let data = EndpointData::Commodities(vec![Commodity::new(
        CommodityId(42),
        "Agricium",
        "AGRI",
        true,
        CommodityAverages::new(Some("120".parse().unwrap()), None),
    )]);
    assert_eq!(application.refreshed(data), Err("read-only"));
    assert!(application.commodities(UexEnvironment::Live, "").is_empty());
    assert_eq!(application.fetched_at(Endpoint::Commodities), None);
    assert_eq!(application.state(), ReferenceState::NoReferenceData);
    let mut application = ReferenceApplication::load(MemoryCache(vec![]), FixedClock).unwrap();
    application
        .refreshed(EndpointData::Commodities(vec![Commodity::new(
            CommodityId(42),
            "Agricium",
            "AGRI",
            true,
            CommodityAverages::new(Some("120".parse().unwrap()), None),
        )]))
        .unwrap();
    assert_eq!(
        application.fetched_at(Endpoint::Commodities),
        Some(Timestamp::from_seconds(7_200))
    );
    assert_eq!(application.commodities(UexEnvironment::Live, "").len(), 1);
    assert!(!application.due().contains(&Endpoint::Commodities));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_6__ptu_includes_unavailable_live_vocabulary_without_live_average_substitution() {
    let cached = CachedReference::new(
        Timestamp::from_seconds(100),
        EndpointData::Commodities(vec![
            Commodity::new(
                CommodityId(7),
                "Test Ore",
                "SHARED",
                false,
                CommodityAverages::new(Some("82".parse().unwrap()), None),
            ),
            Commodity::new(
                CommodityId(8),
                "Live Ore",
                "SHARED",
                true,
                CommodityAverages::new(Some("120".parse().unwrap()), None),
            ),
        ]),
    );
    let application = ReferenceApplication::load(MemoryCache(vec![cached]), FixedClock).unwrap();
    assert_eq!(
        application
            .commodities(UexEnvironment::Live, "")
            .iter()
            .map(|item| item.id())
            .collect::<Vec<_>>(),
        vec![CommodityId(8)]
    );
    let items = application.commodities(UexEnvironment::Ptu, "shared");
    assert_eq!(
        items.iter().map(|item| item.id()).collect::<Vec<_>>(),
        vec![CommodityId(7), CommodityId(8)]
    );
    assert!(items.iter().all(|item| {
        item.average_price(UexEnvironment::Ptu, TradeSide::Buy)
            .is_none()
    }));
    assert_eq!(
        items
            .first()
            .unwrap()
            .average_price(UexEnvironment::Live, TradeSide::Buy),
        Some("82".parse().unwrap())
    );
    assert_eq!(
        application.commodities(UexEnvironment::Ptu, "TEST").len(),
        1
    );
    assert!(
        application
            .commodities(UexEnvironment::Live, "missing")
            .is_empty()
    );
}

struct At(u64);
impl Clock for At {
    fn now(&self) -> Timestamp {
        Timestamp::from_seconds(self.0)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__commodity_refresh_is_due_at_the_one_hour_boundary() {
    for (now, due) in [(3_699, false), (3_700, true)] {
        let entry = CachedReference::new(
            Timestamp::from_seconds(100),
            EndpointData::Commodities(vec![]),
        );
        let application = ReferenceApplication::load(MemoryCache(vec![entry]), At(now)).unwrap();
        assert_eq!(application.due().contains(&Endpoint::Commodities), due);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__later_success_replaces_old_records_and_clears_offline_state() {
    let old = CachedReference::new(
        Timestamp::from_seconds(100),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(1),
            "Old",
            "A",
            true,
            CommodityAverages::default(),
        )]),
    );
    let mut application = ReferenceApplication::load(MemoryCache(vec![old]), FixedClock).unwrap();
    application.refresh_failed(Endpoint::Commodities);
    application
        .refreshed(EndpointData::Commodities(vec![Commodity::new(
            CommodityId(2),
            "New",
            "B",
            true,
            CommodityAverages::default(),
        )]))
        .unwrap();
    assert_eq!(application.state(), ReferenceState::Cached);
    assert_eq!(
        application
            .commodities(UexEnvironment::Live, "")
            .iter()
            .map(|item| item.id())
            .collect::<Vec<_>>(),
        vec![CommodityId(2)]
    );
    let reopened = ReferenceApplication::load(application.into_store(), FixedClock).unwrap();
    assert_eq!(
        reopened
            .commodities(UexEnvironment::Live, "")
            .iter()
            .map(|item| item.id())
            .collect::<Vec<_>>(),
        vec![CommodityId(2)]
    );
    assert_eq!(
        reopened.fetched_at(Endpoint::Commodities),
        Some(Timestamp::from_seconds(7_200))
    );
}

struct MillisecondClock(u64);
impl Clock for MillisecondClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(self.0)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__freshness_keeps_milliseconds_and_rechecks_future_dated_cache() {
    for (now, due) in [(3_699_998, false), (3_699_999, true), (99_998, true)] {
        let entry = CachedReference::new(
            Timestamp::from_millis(99_999),
            EndpointData::Commodities(vec![]),
        );
        let application =
            ReferenceApplication::load(MemoryCache(vec![entry]), MillisecondClock(now)).unwrap();
        assert_eq!(application.due().contains(&Endpoint::Commodities), due);
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_api_6_r_ui_8__cached_terminal_search_preserves_identity_and_location_context() {
    use tallyline_reference_data::api::model::{Terminal, TerminalText};
    use tallyline_shared_kernel::api::model::TerminalId;
    let terminals = vec![
        Terminal::new(
            TerminalId(17),
            TerminalText {
                name: "Trade and Development Division".to_owned(),
                nickname: "TDD".to_owned(),
                display_name: "Trade Centre".to_owned(),
                location: "Stanton / ArcCorp / Area18".to_owned(),
            },
            true,
        ),
        Terminal::new(
            TerminalId(18),
            TerminalText {
                name: "Trade and Development Division".to_owned(),
                nickname: "TDD".to_owned(),
                display_name: "Trade Centre".to_owned(),
                location: "Pyro / Test Station".to_owned(),
            },
            false,
        ),
    ];
    let cached = CachedReference::new(
        Timestamp::from_seconds(100),
        EndpointData::Terminals(terminals),
    );
    let mut application =
        ReferenceApplication::load(MemoryCache(vec![cached]), FixedClock).unwrap();
    for query in ["DEVELOPMENT", "tdd", "trade centre", "AREA18"] {
        assert_eq!(
            application
                .terminals(UexEnvironment::Live, query)
                .iter()
                .map(|terminal| terminal.id())
                .collect::<Vec<_>>(),
            vec![TerminalId(17)]
        );
    }
    assert_eq!(
        application
            .terminals(UexEnvironment::Ptu, "tdd")
            .iter()
            .map(|terminal| terminal.id())
            .collect::<Vec<_>>(),
        vec![TerminalId(17), TerminalId(18)]
    );
    assert_eq!(
        application
            .terminals(UexEnvironment::Ptu, "test station")
            .first()
            .unwrap()
            .text()
            .location,
        "Pyro / Test Station"
    );
    assert!(
        application
            .terminals(UexEnvironment::Live, "unmatched")
            .is_empty()
    );
    assert!(!application.due().contains(&Endpoint::Terminals));
    application.refresh_failed(Endpoint::Terminals);
    assert_eq!(application.state(), ReferenceState::Offline);
    assert_eq!(application.terminals(UexEnvironment::Live, "tdd").len(), 1);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__another_endpoint_success_does_not_clear_an_unrecovered_failure() {
    let mut application = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(100),
            EndpointData::Commodities(vec![]),
        )]),
        FixedClock,
    )
    .unwrap();
    application.refresh_failed(Endpoint::Commodities);
    application
        .refreshed(EndpointData::Terminals(vec![]))
        .unwrap();
    assert_eq!(application.state(), ReferenceState::Offline);
    assert_eq!(
        application.fetched_at(Endpoint::Commodities),
        Some(Timestamp::from_seconds(100))
    );
    assert_eq!(
        application.fetched_at(Endpoint::Terminals),
        Some(Timestamp::from_seconds(7_200))
    );
    application
        .refreshed(EndpointData::Commodities(vec![]))
        .unwrap();
    assert_eq!(application.state(), ReferenceState::Cached);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__terminal_refresh_expires_at_twelve_hours_independently_of_commodities() {
    for (now, terminal_due) in [(43_299, false), (43_300, true)] {
        let application = ReferenceApplication::load(
            MemoryCache(vec![
                CachedReference::new(
                    Timestamp::from_seconds(100),
                    EndpointData::Terminals(vec![]),
                ),
                CachedReference::new(
                    Timestamp::from_seconds(100),
                    EndpointData::Commodities(vec![]),
                ),
            ]),
            At(now),
        )
        .unwrap();
        assert_eq!(
            application.due().contains(&Endpoint::Terminals),
            terminal_due
        );
        assert!(application.due().contains(&Endpoint::Commodities));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_cap_3b__cached_versions_remain_read_data_and_successful_fetches_return_fresh_observations() {
    use tallyline_reference_data::api::model::GameVersions;
    let cached = CachedReference::new(
        Timestamp::from_seconds(100),
        EndpointData::GameVersions(GameVersions {
            live: "4.9".to_owned(),
            ptu: Some("3.24.2a".to_owned()),
        }),
    );
    let mut application =
        ReferenceApplication::load(MemoryCache(vec![cached]), FixedClock).unwrap();
    assert_eq!(application.game_versions().unwrap().live, "4.9");
    assert_eq!(
        application.fetched_at(Endpoint::GameVersions),
        Some(Timestamp::from_seconds(100))
    );
    let fetched = GameVersions {
        live: "4.10.2".to_owned(),
        ptu: None,
    };
    let observation = application
        .refreshed(EndpointData::GameVersions(fetched.clone()))
        .unwrap();
    assert_eq!(observation.endpoint, Endpoint::GameVersions);
    assert_eq!(observation.fetched_at, Timestamp::from_seconds(7_200));
    assert_eq!(observation.game_versions, Some(fetched.clone()));
    assert_eq!(application.game_versions(), Some(&fetched));
    let next = application
        .refreshed(EndpointData::GameVersions(fetched.clone()))
        .unwrap();
    assert_eq!(next.game_versions, Some(fetched));
    let vocabulary = application
        .refreshed(EndpointData::Commodities(vec![]))
        .unwrap();
    assert_eq!(vocabulary.game_versions, None);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_sub_5__version_send_freshness_is_fifteen_minutes_and_normal_ttl_is_one_day() {
    use tallyline_reference_data::api::model::GameVersions;
    for (now, send_due, normal_due) in [
        (999, false, false),
        (1_000, true, false),
        (86_499, true, false),
        (86_500, true, true),
    ] {
        let cached = CachedReference::new(
            Timestamp::from_seconds(100),
            EndpointData::GameVersions(GameVersions {
                live: "4.10.2".to_owned(),
                ptu: None,
            }),
        );
        let application = ReferenceApplication::load(MemoryCache(vec![cached]), At(now)).unwrap();
        assert_eq!(
            application
                .due_before_send()
                .contains(&Endpoint::GameVersions),
            send_due
        );
        assert_eq!(
            application.due().contains(&Endpoint::GameVersions),
            normal_due
        );
    }
    let empty = ReferenceApplication::load(MemoryCache(vec![]), At(0)).unwrap();
    assert_eq!(empty.game_versions(), None);
    assert!(empty.due_before_send().contains(&Endpoint::GameVersions));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__cached_status_levels_keep_side_specific_labels_and_daily_freshness() {
    use std::num::NonZeroU8;
    use tallyline_reference_data::api::model::{StatusLevel, StatusLevels};
    let levels = StatusLevels {
        buy: vec![StatusLevel {
            code: NonZeroU8::new(7).unwrap(),
            name: "Maximum Inventory (Full)".to_owned(),
            short_name: "Maximum".to_owned(),
            abbreviation: "MA".to_owned(),
        }],
        sell: vec![StatusLevel {
            code: NonZeroU8::new(7).unwrap(),
            name: "Maximum Inventory (No Demand)".to_owned(),
            short_name: "Maximum".to_owned(),
            abbreviation: "ND".to_owned(),
        }],
    };
    for (now, due) in [(86_499, false), (86_500, true)] {
        let cached = CachedReference::new(
            Timestamp::from_seconds(100),
            EndpointData::StatusLevels(levels.clone()),
        );
        let application = ReferenceApplication::load(MemoryCache(vec![cached]), At(now)).unwrap();
        assert_eq!(application.status_levels(), Some(&levels));
        assert_eq!(application.due().contains(&Endpoint::StatusLevels), due);
    }
    let mut empty = ReferenceApplication::load(MemoryCache(vec![]), FixedClock).unwrap();
    assert_eq!(empty.status_levels(), None);
    let event = empty
        .refreshed(EndpointData::StatusLevels(levels.clone()))
        .unwrap();
    assert_eq!(event.endpoint, Endpoint::StatusLevels);
    assert_eq!(event.game_versions, None);
    assert_eq!(empty.status_levels(), Some(&levels));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_6__environment_snapshots_physically_remove_live_averages_and_remain_immutable() {
    let mut application = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(100),
            EndpointData::Commodities(vec![
                Commodity::new(
                    CommodityId(7),
                    "Test Ore",
                    "SAME",
                    false,
                    CommodityAverages::new(Some("82".parse().unwrap()), None),
                ),
                Commodity::new(
                    CommodityId(8),
                    "Live Ore",
                    "SAME",
                    true,
                    CommodityAverages::new(Some("120".parse().unwrap()), None),
                ),
            ]),
        )]),
        FixedClock,
    )
    .unwrap();
    let ptu = application.snapshot(UexEnvironment::Ptu);
    assert_eq!(ptu.environment(), UexEnvironment::Ptu);
    assert_eq!(ptu.commodities("same").len(), 2);
    assert!(ptu.commodities("").iter().all(|commodity| {
        commodity
            .average_price(UexEnvironment::Live, TradeSide::Buy)
            .is_none()
    }));
    assert_eq!(ptu.commodities("TEST")[0].id(), CommodityId(7));
    assert!(!ptu.commodities("TEST")[0].available(UexEnvironment::Live));
    assert!(ptu.commodities("unmatched").is_empty());
    let live = application.snapshot(UexEnvironment::Live);
    assert_eq!(live.commodities("").len(), 1);
    assert_eq!(
        live.commodities("")[0].average_price(UexEnvironment::Live, TradeSide::Buy),
        Some("120".parse().unwrap())
    );
    application
        .refreshed(EndpointData::Commodities(vec![Commodity::new(
            CommodityId(9),
            "Later Ore",
            "LATER",
            true,
            CommodityAverages::default(),
        )]))
        .unwrap();
    application.refresh_failed(Endpoint::Commodities);
    assert_eq!(ptu.commodities("same").len(), 2);
    assert_eq!(
        ptu.fetched_at(Endpoint::Commodities),
        Some(Timestamp::from_seconds(100))
    );
    assert_eq!(ptu.state(), ReferenceState::Cached);
    assert_eq!(
        application.snapshot(UexEnvironment::Ptu).state(),
        ReferenceState::Offline
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_api_6__snapshots_search_terminal_context_and_retain_typed_metadata() {
    use std::num::NonZeroU8;
    use tallyline_reference_data::api::model::{
        GameVersions, StatusLevel, StatusLevels, Terminal, TerminalText,
    };
    use tallyline_shared_kernel::api::model::TerminalId;
    let terminals = vec![
        Terminal::new(
            TerminalId(17),
            TerminalText {
                name: "Same".to_owned(),
                nickname: "NICK".to_owned(),
                display_name: "Market".to_owned(),
                location: "Stanton / Area18".to_owned(),
            },
            true,
        ),
        Terminal::new(
            TerminalId(18),
            TerminalText {
                name: "Same".to_owned(),
                nickname: "NICK".to_owned(),
                display_name: "Market".to_owned(),
                location: "Pyro / Test Station".to_owned(),
            },
            false,
        ),
    ];
    let versions = GameVersions {
        live: "4.9".to_owned(),
        ptu: None,
    };
    let levels = StatusLevels {
        buy: vec![StatusLevel {
            code: NonZeroU8::new(7).unwrap(),
            name: "Full".to_owned(),
            short_name: "Full".to_owned(),
            abbreviation: "MA".to_owned(),
        }],
        sell: vec![],
    };
    let application = ReferenceApplication::load(
        MemoryCache(vec![
            CachedReference::new(
                Timestamp::from_seconds(100),
                EndpointData::Terminals(terminals),
            ),
            CachedReference::new(
                Timestamp::from_seconds(110),
                EndpointData::GameVersions(versions.clone()),
            ),
            CachedReference::new(
                Timestamp::from_seconds(120),
                EndpointData::StatusLevels(levels.clone()),
            ),
        ]),
        FixedClock,
    )
    .unwrap();
    let live = application.snapshot(UexEnvironment::Live);
    for query in ["SAME", "nick", "market", "AREA18"] {
        assert_eq!(
            live.terminals(query)
                .iter()
                .map(|terminal| terminal.id())
                .collect::<Vec<_>>(),
            vec![TerminalId(17)]
        );
    }
    assert!(live.terminals("unmatched").is_empty());
    let ptu = application.snapshot(UexEnvironment::Ptu);
    assert_eq!(ptu.terminals("market").len(), 2);
    assert_eq!(ptu.terminals("test station")[0].id(), TerminalId(18));
    assert_eq!(ptu.game_versions(), Some(&versions));
    assert_eq!(ptu.status_levels(), Some(&levels));
    let empty = ReferenceApplication::load(MemoryCache(vec![]), FixedClock)
        .unwrap()
        .snapshot(UexEnvironment::Live);
    assert!(empty.commodities("").is_empty());
    assert!(empty.terminals("").is_empty());
    assert_eq!(empty.game_versions(), None);
    assert_eq!(empty.status_levels(), None);
    assert_eq!(empty.fetched_at(Endpoint::Commodities), None);
    assert_eq!(empty.state(), ReferenceState::NoReferenceData);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_6_r_val_2a__side_specific_exact_averages_are_never_retained_in_ptu_snapshots() {
    use tallyline_reference_data::api::model::CommodityAverages;
    use tallyline_shared_kernel::api::model::TradeSide;
    let buy = "9007199254740993.125".parse().unwrap();
    let sell = "97.25".parse().unwrap();
    let commodity = Commodity::new(
        CommodityId(1),
        "Exact Ore",
        "EX",
        true,
        CommodityAverages::new(Some(buy), Some(sell)),
    );
    let application = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(1),
            EndpointData::Commodities(vec![commodity]),
        )]),
        FixedClock,
    )
    .unwrap();
    let live = application.snapshot(UexEnvironment::Live);
    let live = live.commodities("");
    assert_eq!(
        live[0].average_price(UexEnvironment::Live, TradeSide::Buy),
        Some(buy)
    );
    assert_eq!(
        live[0].average_price(UexEnvironment::Live, TradeSide::Sell),
        Some(sell)
    );
    let ptu = application.snapshot(UexEnvironment::Ptu);
    for side in [TradeSide::Buy, TradeSide::Sell] {
        assert_eq!(
            ptu.commodities("")[0].average_price(UexEnvironment::Live, side),
            None
        );
        assert_eq!(live[0].average_price(UexEnvironment::Ptu, side), None);
    }
    let zero = "0".parse().unwrap();
    let zero = Commodity::new(
        CommodityId(2),
        "Missing",
        "ZERO",
        true,
        CommodityAverages::new(Some(zero), Some(zero)),
    );
    assert_eq!(
        zero.average_price(UexEnvironment::Live, TradeSide::Buy),
        None
    );
    assert_eq!(
        zero.average_price(UexEnvironment::Live, TradeSide::Sell),
        None
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_6__terminal_container_limit_is_optional_immutable_and_retained_in_snapshots() {
    use tallyline_reference_data::api::model::{Terminal, TerminalText};
    use tallyline_shared_kernel::api::model::TerminalId;
    let original = Terminal::new(
        TerminalId(1),
        TerminalText {
            name: "Trade".to_owned(),
            nickname: String::new(),
            display_name: String::new(),
            location: String::new(),
        },
        true,
    );
    let changed = original
        .clone()
        .with_max_container_size(std::num::NonZeroU32::new(24));
    assert_eq!(original.max_container_size(), None);
    assert_eq!(changed.max_container_size().unwrap().get(), 24);
    let application = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(1),
            EndpointData::Terminals(vec![changed]),
        )]),
        FixedClock,
    )
    .unwrap();
    for environment in [UexEnvironment::Live, UexEnvironment::Ptu] {
        assert_eq!(
            application.snapshot(environment).terminals("")[0]
                .max_container_size()
                .unwrap()
                .get(),
            24
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_sub_5_r_ui_8__closed_submission_parameters_do_not_remove_cached_browsing_or_interpret_staff_text()
 {
    use tallyline_reference_data::api::model::{
        CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
    };
    let constraints = CommodityConstraints {
        price_variation_percent: 25,
        prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
        scu_variation_percent: 5000,
    };
    for bits in 0..16 {
        let acceptance = SubmissionAcceptance {
            datacenter: (bits & 1 != 0).into(),
            reports: (bits & 2 != 0).into(),
            commodity: (bits & 4 != 0).into(),
            ptu: (bits & 8 != 0).into(),
        };
        assert_eq!(acceptance.allows(UexEnvironment::Live), bits & 7 == 7);
        assert_eq!(acceptance.allows(UexEnvironment::Ptu), bits & 13 == 13);
    }
    let notification = format!("<b>&amp;</b>{}", "🛰".repeat(300));
    let parameters = ReferenceParameters::new(
        SubmissionAcceptance {
            datacenter: (false).into(),
            reports: (true).into(),
            commodity: (true).into(),
            ptu: (true).into(),
        },
        constraints,
        Some(&notification),
    );
    let application = ReferenceApplication::load(
        MemoryCache(vec![
            CachedReference::new(
                Timestamp::from_seconds(1),
                EndpointData::Commodities(vec![Commodity::new(
                    CommodityId(1),
                    "Ore",
                    "ORE",
                    true,
                    CommodityAverages::default(),
                )]),
            ),
            CachedReference::new(
                Timestamp::from_seconds(1),
                EndpointData::Parameters(parameters),
            ),
        ]),
        FixedClock,
    )
    .unwrap();
    let snapshot = application.snapshot(UexEnvironment::Live);
    assert_eq!(snapshot.commodities("ore").len(), 1);
    let parameters = snapshot.parameters().unwrap();
    assert!(!parameters.acceptance().allows(UexEnvironment::Live));
    assert_eq!(parameters.constraints(), constraints);
    let text = parameters.staff_notification().unwrap();
    assert!(text.starts_with("<b>&amp;</b>"));
    assert_eq!(text.chars().count(), 280);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_sub_5__parameter_cache_has_daily_ttl_and_fifteen_minute_send_freshness() {
    use tallyline_reference_data::api::model::{
        CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
    };
    let parameters = ReferenceParameters::new(
        SubmissionAcceptance {
            datacenter: (true).into(),
            reports: (true).into(),
            commodity: (true).into(),
            ptu: (false).into(),
        },
        CommodityConstraints {
            price_variation_percent: 25,
            prior_ttl_days: std::num::NonZeroU32::new(15).unwrap(),
            scu_variation_percent: 5000,
        },
        None,
    );
    for (now, send_due, daily_due) in [
        (1000, false, false),
        (900999, false, false),
        (901000, true, false),
        (86400999, true, false),
        (86401000, true, true),
        (999, true, true),
    ] {
        let cached = CachedReference::new(
            Timestamp::from_millis(1000),
            EndpointData::Parameters(parameters.clone()),
        );
        let mut application =
            ReferenceApplication::load(MemoryCache(vec![cached]), MillisecondClock(now)).unwrap();
        assert_eq!(application.due().contains(&Endpoint::Parameters), daily_due);
        assert_eq!(
            application
                .due_before_send()
                .contains(&Endpoint::Parameters),
            send_due
        );
        application.refresh_failed(Endpoint::Parameters);
        assert_eq!(
            application.snapshot(UexEnvironment::Live).parameters(),
            Some(&parameters)
        );
    }
    let empty = ReferenceApplication::load(MemoryCache(vec![]), At(0)).unwrap();
    assert!(empty.due_before_send().contains(&Endpoint::Parameters));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1_r_api_6__terminal_priors_expire_after_thirty_minutes_and_are_physically_absent_from_ptu()
{
    use tallyline_reference_data::api::model::{
        PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    };
    use tallyline_shared_kernel::api::model::{ContainerSize, TerminalId};
    let stock = "0.0625".parse().unwrap();
    let values = PriorValues {
        price: "97.125".parse().unwrap(),
        stock,
        status: std::num::NonZeroU8::new(2),
        container_sizes: vec![ContainerSize::Scu1, ContainerSize::Scu16],
    };
    let provenance = PriorProvenance {
        game_version: "4.10.2".to_owned(),
        modified_at: Timestamp::from_seconds(12345),
    };
    let row = PricePrior::new(
        CommodityId(7),
        TradeSide::Sell,
        values.clone(),
        provenance.clone(),
    )
    .unwrap();
    let priors = TerminalPriors::new(TerminalId(17), vec![row.clone()]).unwrap();
    for (now, due) in [(1800999, false), (1801000, true), (999, true)] {
        let mut application = ReferenceApplication::load(
            MemoryCache(vec![CachedReference::new(
                Timestamp::from_millis(1000),
                EndpointData::TerminalPriors(priors.clone()),
            )]),
            MillisecondClock(now),
        )
        .unwrap();
        assert_eq!(application.terminal_priors_due(TerminalId(17)), due);
        assert!(application.terminal_priors_due(TerminalId(18)));
        let live = application.snapshot(UexEnvironment::Live);
        let cached = live.terminal_priors(TerminalId(17)).unwrap();
        assert_eq!(cached.terminal(), TerminalId(17));
        assert_eq!(cached.rows(), std::slice::from_ref(&row));
        assert_eq!(cached.rows()[0].commodity(), CommodityId(7));
        assert_eq!(cached.rows()[0].side(), TradeSide::Sell);
        assert_eq!(cached.rows()[0].values(), &values);
        assert_eq!(cached.rows()[0].provenance(), &provenance);
        assert!(live.terminal_priors(TerminalId(18)).is_none());
        let ptu = application.snapshot(UexEnvironment::Ptu);
        assert!(ptu.terminal_priors(TerminalId(17)).is_none());
        assert_eq!(
            ptu.fetched_at(Endpoint::TerminalPriors(TerminalId(17))),
            None
        );
        application.refresh_failed(Endpoint::TerminalPriors(TerminalId(17)));
        assert_eq!(
            application
                .snapshot(UexEnvironment::Live)
                .terminal_priors(TerminalId(17)),
            Some(&priors)
        );
        assert_eq!(
            application.fetched_at(Endpoint::TerminalPriors(TerminalId(17))),
            Some(Timestamp::from_millis(1000))
        );
        let previous = application.snapshot(UexEnvironment::Live);
        let event = application
            .refreshed(EndpointData::TerminalPriors(
                TerminalPriors::new(TerminalId(17), vec![]).unwrap(),
            ))
            .unwrap();
        assert_eq!(event.endpoint, Endpoint::TerminalPriors(TerminalId(17)));
        assert_eq!(event.game_versions, None);
        assert_eq!(previous.terminal_priors(TerminalId(17)), Some(&priors));
        assert!(
            application
                .snapshot(UexEnvironment::Live)
                .terminal_priors(TerminalId(17))
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_5__invalid_prior_identity_price_version_and_duplicate_assortments_are_refused() {
    use tallyline_reference_data::api::model::{
        InvalidPrior, PricePrior, PriorProvenance, PriorValues, TerminalPriors,
    };
    use tallyline_shared_kernel::api::model::TerminalId;
    for (id, price, version) in [
        (0, "1", "4.10.2"),
        (1, "0", "4.10.2"),
        (1, "1", ""),
        (1, "1", "  "),
    ] {
        assert_eq!(
            PricePrior::new(
                CommodityId(id),
                TradeSide::Buy,
                PriorValues {
                    price: price.parse().unwrap(),
                    stock: "0".parse().unwrap(),
                    status: None,
                    container_sizes: vec![]
                },
                PriorProvenance {
                    game_version: version.to_owned(),
                    modified_at: Timestamp::from_seconds(1)
                }
            ),
            Err(InvalidPrior)
        );
    }
    let row = PricePrior::new(
        CommodityId(1),
        TradeSide::Buy,
        PriorValues {
            price: "1".parse().unwrap(),
            stock: "0".parse().unwrap(),
            status: None,
            container_sizes: vec![],
        },
        PriorProvenance {
            game_version: "4.9".to_owned(),
            modified_at: Timestamp::from_seconds(1),
        },
    )
    .unwrap();
    assert_eq!(
        TerminalPriors::new(TerminalId(0), vec![row.clone()]),
        Err(InvalidPrior)
    );
    assert_eq!(
        TerminalPriors::new(TerminalId(1), vec![row.clone(), row]),
        Err(InvalidPrior)
    );
    assert!(
        TerminalPriors::new(TerminalId(1), vec![])
            .unwrap()
            .rows()
            .is_empty()
    );
    assert!(InvalidPrior.to_string().contains("prior"));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_api_1__failed_forced_refresh_retries_even_when_the_previous_success_is_still_fresh() {
    use tallyline_reference_data::api::model::{GameVersions, TerminalPriors};
    use tallyline_shared_kernel::api::model::TerminalId;
    let versions = EndpointData::GameVersions(GameVersions {
        live: "4.9".to_owned(),
        ptu: None,
    });
    let priors = EndpointData::TerminalPriors(TerminalPriors::new(TerminalId(17), vec![]).unwrap());
    let mut application = ReferenceApplication::load(
        MemoryCache(vec![
            CachedReference::new(Timestamp::from_seconds(0), versions.clone()),
            CachedReference::new(Timestamp::from_seconds(0), priors.clone()),
        ]),
        At(0),
    )
    .unwrap();
    assert!(!application.due().contains(&Endpoint::GameVersions));
    assert!(!application.terminal_priors_due(TerminalId(17)));
    application.refresh_failed(Endpoint::GameVersions);
    application.refresh_failed(Endpoint::TerminalPriors(TerminalId(17)));
    assert!(application.due().contains(&Endpoint::GameVersions));
    assert!(
        application
            .due_before_send()
            .contains(&Endpoint::GameVersions)
    );
    assert!(
        application
            .due()
            .contains(&Endpoint::TerminalPriors(TerminalId(17)))
    );
    assert!(application.terminal_priors_due(TerminalId(17)));
    assert_eq!(
        application.fetched_at(Endpoint::GameVersions),
        Some(Timestamp::from_seconds(0))
    );
    assert_eq!(
        application
            .snapshot(UexEnvironment::Live)
            .game_versions()
            .unwrap()
            .live,
        "4.9"
    );
    application.refreshed(versions).unwrap();
    assert!(!application.due().contains(&Endpoint::GameVersions));
    assert_eq!(application.state(), ReferenceState::Offline);
    application.refreshed(priors).unwrap();
    assert!(
        !application
            .due()
            .contains(&Endpoint::TerminalPriors(TerminalId(17)))
    );
    assert!(!application.terminal_priors_due(TerminalId(17)));
    assert_eq!(application.state(), ReferenceState::Cached);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_man_1__manual_terminal_search_accepts_fuzzy_names_without_resolving_shared_display_names() {
    use tallyline_reference_data::api::model::{Terminal, TerminalText};
    use tallyline_shared_kernel::api::model::TerminalId;
    let terminal = |id, location: &str| {
        Terminal::new(
            TerminalId(id),
            TerminalText {
                name: "Orbital Depot".to_owned(),
                nickname: "OD".to_owned(),
                display_name: "Shared shop".to_owned(),
                location: location.to_owned(),
            },
            true,
        )
    };
    let app = ReferenceApplication::load(
        MemoryCache(vec![CachedReference::new(
            Timestamp::from_seconds(100),
            EndpointData::Terminals(vec![
                terminal(1, "Stanton / ArcCorp"),
                terminal(2, "Pyro / Ruin"),
            ]),
        )]),
        FixedClock,
    )
    .unwrap();
    assert_eq!(
        app.snapshot(UexEnvironment::Live)
            .terminals("orb dpt")
            .len(),
        2
    );
    assert_eq!(
        app.snapshot(UexEnvironment::Live)
            .terminals("Shared shop")
            .len(),
        2
    );
    assert_eq!(
        app.snapshot(UexEnvironment::Live)
            .terminals("Pyro")
            .first()
            .unwrap()
            .id(),
        TerminalId(2)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_11_q_manual_r_nf_5__effective_staleness_uses_fifteen_days_until_uex_and_only_accepts_raises()
 {
    use std::num::NonZeroU32;
    use tallyline_reference_data::api::model::{
        AcceptanceSwitch, CommodityConstraints, ReferenceParameters, SubmissionAcceptance,
    };
    let mut application = ReferenceApplication::load(MemoryCache(vec![]), FixedClock).unwrap();
    let snapshot = application.snapshot(UexEnvironment::Live);
    assert_eq!(snapshot.effective_staleness(None).days(), 15);
    assert_eq!(
        snapshot.effective_staleness(None).source(),
        tallyline_reference_data::api::model::StalenessSource::SettingsFallback
    );
    assert_eq!(snapshot.effective_staleness(NonZeroU32::new(2)).days(), 15);
    application
        .refreshed(EndpointData::Parameters(ReferenceParameters::new(
            SubmissionAcceptance {
                datacenter: AcceptanceSwitch::Enabled,
                reports: AcceptanceSwitch::Enabled,
                commodity: AcceptanceSwitch::Enabled,
                ptu: AcceptanceSwitch::Disabled,
            },
            CommodityConstraints {
                price_variation_percent: 25,
                prior_ttl_days: NonZeroU32::new(20).unwrap(),
                scu_variation_percent: 5000,
            },
            None,
        )))
        .unwrap();
    let snapshot = application.snapshot(UexEnvironment::Live);
    assert_eq!(snapshot.effective_staleness(NonZeroU32::new(16)).days(), 20);
    assert_eq!(snapshot.effective_staleness(NonZeroU32::new(30)).days(), 30);
    assert_eq!(snapshot.effective_staleness(None).days(), 20);
    assert_eq!(
        snapshot.effective_staleness(None).source(),
        tallyline_reference_data::api::model::StalenessSource::Uex
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual__effective_price_policy_has_a_sourced_documented_fallback_and_cached_uex_provenance()
 {
    use std::num::NonZeroU32;
    use tallyline_reference_data::api::model::{
        AcceptanceSwitch, CommodityConstraints, PriceToleranceSource, ReferenceParameters,
        SubmissionAcceptance,
    };
    let mut app = ReferenceApplication::load(MemoryCache(vec![]), FixedClock).unwrap();
    let policy = app
        .snapshot(UexEnvironment::Live)
        .effective_price_tolerance(None);
    assert_eq!(policy.percent().get(), 25);
    let PriceToleranceSource::SettingsFallback(source) = policy.source() else {
        panic!("missing documented fallback provenance")
    };
    assert_eq!(source.percent().get(), 25);
    assert_eq!(source.verified_on(), "2026-11-02");
    assert_eq!(
        source.source_url(),
        "https://api.uexcorp.uk/2.0/data_parameters"
    );
    app.refreshed(EndpointData::Parameters(ReferenceParameters::new(
        SubmissionAcceptance {
            datacenter: AcceptanceSwitch::Enabled,
            reports: AcceptanceSwitch::Enabled,
            commodity: AcceptanceSwitch::Enabled,
            ptu: AcceptanceSwitch::Disabled,
        },
        CommodityConstraints {
            price_variation_percent: 10,
            prior_ttl_days: NonZeroU32::new(15).unwrap(),
            scu_variation_percent: 5000,
        },
        None,
    )))
    .unwrap();
    let policy = app
        .snapshot(UexEnvironment::Live)
        .effective_price_tolerance(None);
    assert_eq!(policy.percent().get(), 10);
    assert_eq!(
        policy.source(),
        PriceToleranceSource::Uex(Timestamp::from_seconds(7200))
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_nf_5__price_override_only_tightens_and_reports_an_ignored_relaxation() {
    use tallyline_shared_kernel::api::model::Percent;
    let app = ReferenceApplication::load(MemoryCache(vec![]), FixedClock).unwrap();
    let reference = app.snapshot(UexEnvironment::Live);
    let tighter = reference.effective_price_tolerance(Some(Percent::whole(10)));
    assert_eq!(tighter.percent().get(), 10);
    assert!(!tighter.override_ignored());
    let looser = reference.effective_price_tolerance(Some(Percent::whole(50)));
    assert_eq!(looser.percent().get(), 25);
    assert!(looser.override_ignored());
    assert!(
        !reference
            .effective_price_tolerance(Some(Percent::whole(25)))
            .override_ignored()
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_val_2_q_manual_r_val_2a_q_manual__shared_price_policy_owns_both_reference_bands() {
    use tallyline_reference_data::api::model::PriceReferenceSource;
    use tallyline_shared_kernel::api::model::PricePerScu;
    let app = ReferenceApplication::load(MemoryCache(vec![]), FixedClock).unwrap();
    let policy = app
        .snapshot(UexEnvironment::Live)
        .effective_price_tolerance(None);
    let boundary = "120"
        .parse::<PricePerScu>()
        .unwrap()
        .difference("80".parse().unwrap())
        .unwrap();
    assert_eq!(
        policy.exceeded_by(boundary, PriceReferenceSource::Terminal),
        Ok(true)
    );
    assert_eq!(
        policy.exceeded_by(boundary, PriceReferenceSource::CommodityAverage),
        Ok(false)
    );
    let outside = "120.00000000000000000000000001"
        .parse::<PricePerScu>()
        .unwrap()
        .difference("80".parse().unwrap())
        .unwrap();
    assert_eq!(
        policy.exceeded_by(outside, PriceReferenceSource::CommodityAverage),
        Ok(true)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_sub_2_r_val_7__release_reference_comparison_refuses_changed_values_and_refreshes() {
    let entry = CachedReference::new(
        Timestamp::from_seconds(1000),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(1),
            "Gold",
            "GOLD",
            true,
            CommodityAverages::new(None, None),
        )]),
    );
    let app = ReferenceApplication::load(MemoryCache(vec![entry.clone()]), FixedClock).unwrap();
    let snapshot = app.snapshot(UexEnvironment::Live);
    assert!(snapshot.matches_cached_references(std::slice::from_ref(&entry)));
    assert!(!snapshot.matches_cached_references(&[]));
    let refreshed = CachedReference::new(Timestamp::from_seconds(1001), entry.data().clone());
    assert!(!snapshot.matches_cached_references(&[refreshed]));
    let changed = CachedReference::new(
        entry.fetched_at(),
        EndpointData::Commodities(vec![Commodity::new(
            CommodityId(1),
            "Gold",
            "GOLD",
            true,
            CommodityAverages::new(Some("123".parse().unwrap()), None),
        )]),
    );
    assert!(!snapshot.matches_cached_references(&[changed]));
}
