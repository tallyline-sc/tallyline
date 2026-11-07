// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Privacy and read-only contracts through actual game files.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tallyline_adapter_files::api::GameLogFiles;
use tallyline_game::api::model::{GameSettings, WitnessMode, WitnessSnapshot, WitnessUnavailable};
use tallyline_shared_kernel::api::model::Timestamp;

struct GameDirectory(PathBuf);
impl GameDirectory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tallyline-witness-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn manifest(&self, version: &str, branch: &str) -> std::io::Result<()> {
        let document =
            serde_json::to_vec(&serde_json::json!({"Data":{"Version":version,"Branch":branch}}))
                .map_err(std::io::Error::other)?;
        std::fs::write(self.0.join("build_manifest.id"), document)
    }
    fn source(&self) -> GameLogFiles {
        GameLogFiles::new(self.0.clone())
    }
}
impl Drop for GameDirectory {
    #[expect(
        clippy::unwrap_used,
        reason = "Cleanup failure must fail the real-filesystem contract test."
    )]
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn enabled() -> GameSettings {
    GameSettings {
        witness: WitnessMode::On,
        ..GameSettings::default()
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__unvalidated_build_is_disabled_without_opening_the_log() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.9999", "sc-alpha-4.10.2")
        .unwrap();
    assert_eq!(
        directory.source().inspect(&enabled()),
        WitnessSnapshot::Unavailable(WitnessUnavailable::UnvalidatedBuild)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__validated_facts_discard_private_and_transaction_fields_at_the_boundary() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    std::fs::write(directory.0.join("Game.log"), concat!(
        "FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n<2000-01-01T11:59:59.000Z> [Notice] <RequestLocationInventory> Player[SYNTHETIC_PRIVATE_HANDLE] requested inventory for Location[RR_P6_LEO]\n",
        "<2000-01-01T12:00:00.125Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox - playerId[SYNTHETIC_PRIVATE_ID] shopId[SYNTHETIC_PRIVATE_SHOP] shopName[SCShop_Pyro_RestStop_Rund_Admin] commodityName[ResourceType.AltruciaToxin_unprocessed] Available Box Sizes: boxSize[24] boxSize[32] endpoint[SYNTHETIC_PRIVATE_ENDPOINT] price[SYNTHETIC_PRIVATE_PRICE] quantity[SYNTHETIC_PRIVATE_QUANTITY] status[SYNTHETIC_PRIVATE_STATUS]\n",
        "<2000-01-01T12:00:01.000Z> [Notice] SendCommodityBuyRequest price[SYNTHETIC_TRANSACTION] quantity[SYNTHETIC_TRANSACTION]\n"
    )).unwrap();
    let snapshot = directory.source().inspect(&enabled());
    let WitnessSnapshot::Available(facts) = &snapshot else {
        panic!("Expected validated facts: {snapshot:?}")
    };
    assert_eq!(facts.rows().len(), 1);
    let row = facts.rows().first().unwrap();
    assert_eq!(row.terminal().unwrap().0, 466);
    assert_eq!(row.commodity().unwrap().0, 66);
    assert_eq!(
        row.container_sizes().iter().copied().collect::<Vec<_>>(),
        vec![24, 32]
    );
    assert_eq!(
        row.logged_at(),
        tallyline_shared_kernel::api::model::Timestamp::from_millis(946728000125)
    );
    let output = format!("{snapshot:?}");
    assert!(!output.contains("SYNTHETIC_"));
    assert!(!output.contains("price"));
    assert!(!output.contains("quantity"));
    assert!(!output.contains("status"));
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__unreadable_sources_report_no_path_or_raw_operating_system_error() {
    let directory = GameDirectory::new().unwrap();
    assert_eq!(
        directory.source().inspect(&enabled()),
        WitnessSnapshot::Unavailable(WitnessUnavailable::ReadFailure)
    );
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    assert_eq!(
        directory.source().inspect(&enabled()),
        WitnessSnapshot::Unavailable(WitnessUnavailable::ReadFailure)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__malformed_validated_records_disable_the_whole_witness_without_retaining_text() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    std::fs::write(directory.0.join("Game.log"), "FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n<2000-01-01T12:00:00.000Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox SYNTHETIC_PRIVATE_MALFORMED\n").unwrap();
    assert_eq!(
        directory.source().inspect(&enabled()),
        WitnessSnapshot::Unavailable(WitnessUnavailable::UnvalidatedFormat)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__unmapped_names_and_player_inventory_never_gain_witness_identity() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    std::fs::write(directory.0.join("Game.log"), concat!(
        "FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n",
        "<2000-01-01T11:59:59.000Z> [Notice] <RequestLocationInventory> Player[SYNTHETIC_HANDLE] requested inventory for Location[RR_P3_L3]\n",
        "<2000-01-01T12:00:00.000Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox shopName[SCShop_Pyro_RestStop_Rund_Admin] commodityName[ResourceType.AltruciaToxin] Available Box Sizes: boxSize[24] boxSize[32]\n",
        "<2000-01-01T12:00:01.000Z> [Notice] CEntityComponentCommodityUIProvider::AddPlayerCommodityItem AddingCommodityBox commodityName[ResourceType.RMC]\n"
    )).unwrap();
    let WitnessSnapshot::Available(facts) = directory.source().inspect(&enabled()) else {
        panic!("Expected sanitised unmapped observations")
    };
    assert_eq!(facts.rows().len(), 1);
    assert!(facts.rows().first().unwrap().terminal().is_none());
    assert!(facts.rows().first().unwrap().commodity().is_none());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__bounded_sources_and_missing_channel_disable_without_disclosing_input() {
    assert_eq!(
        GameLogFiles::new(PathBuf::new()).inspect(&enabled()),
        WitnessSnapshot::Unavailable(WitnessUnavailable::NotConfigured)
    );
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    std::fs::write(
        directory.0.join("Game.log"),
        "SYNTHETIC_PRIVATE_LONG".repeat(1000),
    )
    .unwrap();
    assert_eq!(
        directory.source().inspect(&enabled()),
        WitnessSnapshot::Unavailable(WitnessUnavailable::InputLimit)
    );
    let file = std::fs::File::create(directory.0.join("Game.log")).unwrap();
    file.set_len(134_217_729).unwrap();
    drop(file);
    assert_eq!(
        directory.source().inspect(&enabled()),
        WitnessSnapshot::Unavailable(WitnessUnavailable::InputLimit)
    );
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__evaluation_output_is_allowlisted_and_game_files_remain_byte_exact() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    let log = b"FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n<2000-01-01T11:59:59.000Z> [Notice] <RequestLocationInventory> Player[SYNTHETIC_PRIVATE_HANDLE] requested inventory for Location[Nyx_Levski]\n<2000-01-01T12:00:00.000Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox playerId[SYNTHETIC_PRIVATE_ID] shopName[SCShop_Levski_CargoOffice_Commodities] commodityName[ResourceType.PlasmaFuel] Available Box Sizes: boxSize[1] boxSize[2]\n";
    std::fs::write(directory.0.join("Game.log"), log).unwrap();
    let manifest = std::fs::read(directory.0.join("build_manifest.id")).unwrap();
    let output = directory.source().evaluate();
    assert_eq!(output["state"], "on");
    assert_eq!(output["major_minor"], serde_json::json!([4, 10]));
    assert_eq!(
        output["buy_side_rows"][0],
        serde_json::json!({"logged_at_unix_millis":946728000000_u64,"terminal_id":778,"commodity_id":104,"container_sizes":[1,2]})
    );
    assert!(!output.to_string().contains("SYNTHETIC_"));
    assert!(
        !output
            .to_string()
            .contains(&directory.0.to_string_lossy().to_string())
    );
    assert_eq!(std::fs::read(directory.0.join("Game.log")).unwrap(), log);
    assert_eq!(
        std::fs::read(directory.0.join("build_manifest.id")).unwrap(),
        manifest
    );
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__future_location_entries_do_not_supply_a_terminal_for_earlier_rows() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    std::fs::write(directory.0.join("Game.log"), concat!(
        "FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n",
        "<2000-01-01T12:00:00.126Z> [Notice] <RequestLocationInventory> Player[SYNTHETIC] requested inventory for Location[RR_MIC_L2]\n",
        "<2000-01-01T12:00:00.125Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox shopName[SCShop_Admin_lt_base_g] commodityName[ResourceType.Scrap] Available Box Sizes: boxSize[1]\n"
    )).unwrap();
    let WitnessSnapshot::Available(facts) = directory.source().inspect(&enabled()) else {
        panic!("Expected sanitised facts")
    };
    assert!(facts.rows().first().unwrap().terminal().is_none());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__changed_kiosk_payload_and_non_digit_timestamps_disable_the_witness() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    for record in [
        "<2000-01-01T12:00:00.000Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData RenamedCommodityRecord SYNTHETIC_PRIVATE_CHANGED",
        "<2000-01-+1T12:00:00.000Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox shopName[SCShop_Levski_CargoOffice_Commodities] commodityName[ResourceType.RMC] Available Box Sizes: boxSize[1]",
    ] {
        std::fs::write(
            directory.0.join("Game.log"),
            format!("FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n{record}\n"),
        )
        .unwrap();
        assert_eq!(
            directory.source().inspect(&enabled()),
            WitnessSnapshot::Unavailable(WitnessUnavailable::UnvalidatedFormat)
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__allowlisted_observations_require_the_exact_location_and_template_pair() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    for (location, shop, commodity, terminal_id, commodity_id) in [
        (
            "RR_MIC_L2",
            "SCShop_Admin_lt_base_g",
            "ResourceType.Astatine",
            Some(55),
            Some(9),
        ),
        (
            "RR_P3_LEO",
            "SCShop_Pyro_RestStop_Rund_Admin",
            "ResourceType.Carbon",
            Some(443),
            Some(102),
        ),
        (
            "RR_P3_LEO",
            "SCShop_Pyro_RestStop_Rund_Admin",
            "ResourceType.Copper",
            Some(443),
            Some(20),
        ),
        (
            "RR_MIC_L2",
            "SCShop_Admin_lt_base_g",
            "ResourceType.Diamond",
            Some(55),
            Some(25),
        ),
        (
            "RR_P3_LEO",
            "SCShop_Pyro_RestStop_Rund_Admin",
            "ResourceType.Potassium",
            Some(443),
            Some(116),
        ),
        (
            "RR_MIC_L2",
            "SCShop_Admin_lt_base_g",
            "ResourceType.Scrap",
            Some(55),
            Some(67),
        ),
        (
            "RR_MIC_L2",
            "SCShop_Admin_lt_base_g",
            "ResourceType.Waste",
            Some(55),
            Some(79),
        ),
        (
            "RR_MIC_L2",
            "SCShop_Admin_lt_base_g",
            "ResourceType.PlasmaFuel",
            Some(55),
            Some(104),
        ),
        (
            "RR_P3_LEO",
            "SCShop_Pyro_RestStop_Rund_Admin",
            "ResourceType.RMC",
            Some(443),
            Some(63),
        ),
        (
            "RR_MIC_L2",
            "SCShop_Pyro_RestStop_Rund_Admin",
            "ResourceType.PlasmaFuel",
            None,
            Some(104),
        ),
        (
            "RR_P3_LEO",
            "SCShop_Admin_lt_base_g",
            "ResourceType.RMC",
            None,
            Some(63),
        ),
        (
            "RR_P6_LEO",
            "SCShop_Pyro_RestStop_Rund_Admin",
            "ResourceType.AltruciaToxin_unprocessed",
            Some(466),
            Some(66),
        ),
        (
            "RR_CRU_LEO",
            "SCShop_Admin_lt_base_g",
            "ResourceType.RMC",
            Some(259),
            Some(63),
        ),
        (
            "Nyx_Levski",
            "SCShop_Levski_CargoOffice_Commodities",
            "ResourceType.QuantumFuel",
            Some(778),
            Some(87),
        ),
        (
            "Nyx_Levski",
            "SCShop_Levski_CargoOffice_Commodities",
            "ResourceType.PlasmaFuel",
            Some(778),
            Some(104),
        ),
        (
            "RR_CRU_LEO",
            "SCShop_Pyro_RestStop_Rund_Admin",
            "ResourceType.Hydrogen",
            None,
            None,
        ),
        (
            "SYNTHETIC_UNKNOWN_LOCATION",
            "SCShop_Levski_CargoOffice_Commodities",
            "ResourceType.SyntheticUnknown",
            None,
            None,
        ),
    ] {
        std::fs::write(directory.0.join("Game.log"),format!("FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n<2000-01-01T11:59:59.000Z> [Notice] <RequestLocationInventory> Player[SYNTHETIC] requested inventory for Location[{location}]\n<2000-01-01T12:00:00.000Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox shopName[{shop}] commodityName[{commodity}] Available Box Sizes: boxSize[1] boxSize[2] boxSize[4] boxSize[8] boxSize[16] boxSize[24] boxSize[32]\n")).unwrap();
        let WitnessSnapshot::Available(facts) = directory.source().inspect(&enabled()) else {
            panic!("Expected typed observations")
        };
        let row = facts.rows().first().unwrap();
        assert_eq!(row.terminal().map(|id| id.0), terminal_id);
        assert_eq!(row.commodity().map(|id| id.0), commodity_id);
        assert_eq!(facts.rows().len(), 1);
        assert_eq!(facts.major_minor(), (4, 10));
        assert_eq!(row.logged_at(), Timestamp::from_millis(946728000000));
        assert_eq!(
            row.container_sizes().iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 4, 8, 16, 24, 32]
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__commodity_extensions_do_not_admit_similar_names_or_unvalidated_aliases() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    for commodity in [
        "ResourceType.astatine",
        "ResourceType.Carbon ",
        "ResourceType.Copper_ore",
        "ResourceType.DiamondLaminate",
        "OtherType.Potassium",
        "ResourceType.ScrapMetal",
        "ResourceType.Waste_unprocessed",
        "ResourceType.MedicalSupplies",
        "ResourceType.ShipAmmoSize1",
        "ResourceType.ShipAmmoSize2",
        "ResourceType.ShipAmmoSize3",
        "ResourceType.ShipAmmoSize4",
        "ResourceType.ShipAmmoSize5",
        "ResourceType.ShipAmmoSize6",
        "ResourceType.ShipAmmoSize7",
        "ResourceType.ShipCountermeasuresDecoy",
        "ResourceType.ShipCountermeasuresNoise",
    ] {
        std::fs::write(directory.0.join("Game.log"), format!("FileVersion: 4.10.197.2878\nBranch: sc-alpha-4.10.2\n<2000-01-01T11:59:59.000Z> [Notice] <RequestLocationInventory> Player[SYNTHETIC] requested inventory for Location[RR_P3_LEO]\n<2000-01-01T12:00:00.000Z> [Notice] CEntityComponentCommodityUIProvider::LoadShopInventoryData AddingCommodityBox shopName[SCShop_Pyro_RestStop_Rund_Admin] commodityName[{commodity}] Available Box Sizes: boxSize[8] boxSize[16] boxSize[24] boxSize[32]\n")).unwrap();
        let WitnessSnapshot::Available(facts) = directory.source().inspect(&enabled()) else {
            panic!("Expected sanitised unmapped observation")
        };
        assert_eq!(facts.rows().len(), 1);
        let row = facts.rows().first().unwrap();
        assert_eq!(row.terminal().map(|id| id.0), Some(443));
        assert!(
            row.commodity().is_none(),
            "Unexpected mapping for {commodity}"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ocr_20__timestamp_prefixed_headers_match_the_locally_verified_game_format() {
    let directory = GameDirectory::new().unwrap();
    directory
        .manifest("4.10.197.2878", "sc-alpha-4.10.2")
        .unwrap();
    std::fs::write(directory.0.join("Game.log"),"<2000-01-01T11:00:00.000Z> FileVersion: 4.10.197.2878\n<2000-01-01T11:00:00.000Z> Branch: sc-alpha-4.10.2\n").unwrap();
    assert!(matches!(
        directory.source().inspect(&enabled()),
        WitnessSnapshot::Available(_)
    ));
}
