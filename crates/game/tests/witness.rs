// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only game witness application acceptance tests.

use tallyline_game::api::model::{GameSettings, WitnessSnapshot};
use tallyline_game::api::{GameLogSource, inspect_witness};

struct UnavailableSource(std::cell::Cell<usize>);

impl GameLogSource for UnavailableSource {
    fn read(&self) -> WitnessSnapshot {
        self.0.set(self.0.get() + 1);
        WitnessSnapshot::Unavailable(tallyline_game::api::model::WitnessUnavailable::ReadFailure)
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__default_off_never_reads_game_files() {
    let source = UnavailableSource(std::cell::Cell::new(0));
    assert_eq!(
        inspect_witness(&GameSettings::default(), &source),
        WitnessSnapshot::Off
    );
    assert_eq!(source.0.get(), 0);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__enabled_witness_preserves_the_sources_closed_unavailability_reason() {
    struct Unsupported;
    impl GameLogSource for Unsupported {
        fn read(&self) -> WitnessSnapshot {
            WitnessSnapshot::Unavailable(
                tallyline_game::api::model::WitnessUnavailable::UnvalidatedBuild,
            )
        }
    }
    let settings = GameSettings {
        witness: tallyline_game::api::model::WitnessMode::On,
        ..GameSettings::default()
    };
    assert_eq!(
        inspect_witness(&settings, &Unsupported),
        WitnessSnapshot::Unavailable(
            tallyline_game::api::model::WitnessUnavailable::UnvalidatedBuild
        )
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20_r_nf_5__witness_preferences_are_applied_only_after_durable_opt_in() {
    use tallyline_game::api::model::WitnessMode;
    use tallyline_game::api::{GameSettingsApplication, GameSettingsStore};
    struct Store;
    impl GameSettingsStore for Store {
        type Error = &'static str;
        fn save_game_settings(&mut self, _: &GameSettings) -> Result<(), Self::Error> {
            Err("not writable")
        }
    }
    let mut application = GameSettingsApplication::new(GameSettings::default(), Store);
    let next = GameSettings {
        witness: WitnessMode::On,
        ..GameSettings::default()
    };
    assert_eq!(application.change_settings(next), Err("not writable"));
    assert_eq!(application.settings(), &GameSettings::default());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__enabled_application_exposes_only_typed_buy_side_facts_and_major_minor_version() {
    use std::collections::BTreeSet;
    use tallyline_game::api::model::{ShopInventoryFact, WitnessFacts, WitnessMode};
    use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp};
    struct ValidatedSource;
    impl GameLogSource for ValidatedSource {
        fn read(&self) -> WitnessSnapshot {
            WitnessSnapshot::Available(WitnessFacts::new(vec![ShopInventoryFact::new(
                Timestamp::from_millis(946728000125),
                Some(TerminalId(466)),
                Some(CommodityId(66)),
                BTreeSet::from([24, 32]),
            )]))
        }
    }
    let snapshot = inspect_witness(
        &GameSettings {
            witness: WitnessMode::On,
            ..GameSettings::default()
        },
        &ValidatedSource,
    );
    let WitnessSnapshot::Available(facts) = snapshot else {
        panic!("Expected the validated source facts")
    };
    assert_eq!(facts.major_minor(), (4, 10));
    assert_eq!(facts.rows().len(), 1);
    let row = facts.rows().first().unwrap();
    assert_eq!(row.logged_at().millis(), 946728000125);
    assert_eq!(row.terminal(), Some(TerminalId(466)));
    assert_eq!(row.commodity(), Some(CommodityId(66)));
    assert_eq!(row.container_sizes(), &BTreeSet::from([24, 32]));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__successful_persistence_applies_both_opt_in_and_opt_out() {
    use tallyline_game::api::model::WitnessMode;
    use tallyline_game::api::{GameSettingsApplication, GameSettingsStore};
    struct Writable;
    impl GameSettingsStore for Writable {
        type Error = std::convert::Infallible;
        fn save_game_settings(&mut self, _: &GameSettings) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    let mut application = GameSettingsApplication::new(GameSettings::default(), Writable);
    application
        .change_settings(GameSettings {
            witness: WitnessMode::On,
            ..GameSettings::default()
        })
        .unwrap();
    assert_eq!(application.settings().witness, WitnessMode::On);
    application
        .change_settings(GameSettings::default())
        .unwrap();
    assert_eq!(application.settings().witness, WitnessMode::Off);
}
