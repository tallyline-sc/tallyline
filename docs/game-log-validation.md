# Current Game.log witness validation

> **Doc type:** Living reference — finite validation complete; supported scope and exclusions recorded. Last reviewed: 2026-11-03.

Corrected 2026-11-04: the owner marked all seven existing public corpus entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Earlier checkpoint verification states and inspection results below remain historical; this decision supplies no missing metadata, frozen reference snapshot, class coverage or recognition accuracy measurement.

This is the dated result of the local current-LIVE review for issue #107, R-OCR-20, A24 and O-101. It records the validated initial evidence scope, not a production parser or exhaustive mapping registry. The witness remains opt-in and default off; unknown formats and mappings supply no automatic resolving evidence. The implementation boundary and its privacy checks are recorded below (issue #108).

## Evidence boundary

Reviewed 2026-11-01: the user-authorised LIVE installation manifest and a current-build backup containing kiosk records. The manifest reports build `4.10.197.2878`, branch `sc-alpha-4.10.2`. Its `Data.Version` and `Data.Branch` are the relevant allowlisted fields. The current log and recent backups were surveyed locally; the initial 370-record survey below comes from one backup. The follow-up paired-capture review adds two backups from 2026-10-14 with 411 and 175 valid shop rows; both report the same build and branch. These row counts describe separate reviews and are not counts of distinct kiosk openings.

Raw logs remain local. No raw line, player handle, player or session identifier, endpoint, balance, personal path or complete reference-data dump is included here. Field names below describe the format; all example values in the synthetic specimens are invented. Reading and summarising selected fields is not proof that an eventual parser drops private fields: that requires the #108 privacy test through `GameLogSource`.

## Current format observations

Of 385 records containing `AddingCommodityBox`, 370 also contain `CEntityComponentCommodityUIProvider::LoadShopInventoryData` and `Available Box Sizes`. All 370 have a UTC timestamp shaped `<YYYY-MM-DDTHH:MM:SS.mmmZ>` and `[Notice]` severity. Their fields include `playerId[]`, `shopId[]`, `shopName[]`, `commodityName[ResourceType.<internal name>]` and repeated `boxSize[]` values. The source names the load of shop inventory; it is not a scroll event or proof of sell-side assortment.

The other 15 records come from `CEntityComponentCommodityUIProvider::AddPlayerCommodityItem` and lack `Available Box Sizes`. They describe player inventory, so matching `AddingCommodityBox` alone would admit the wrong source. They must never establish kiosk assortment or sell-side facts.

The valid records contain these size sets: `{8,16,24,32}`, `{16,24,32}`, `{1,2,4,8,16,24,32}`, `{24,32}` and `{4,8,16,24,32}`. No stock, quantity, `scu_buy`, `scu_sell` or price field was found on these 370 records. This confirms the current shape of the buy-side witness; the historical screen/UEX comparison in A24 remains the evidence for side interpretation and size agreement. Corrected 2026-11-01: the follow-up below compares selected current-version paired captures and size sets. No new current UEX per-row size comparison was performed.

Location records retain the historical `<RequestLocationInventory>` shape: `Player[] requested inventory for Location[]`. Current observations include `RR_P6_LEO`, `RR_P3_L3` and `RR_P2_L4`. The same `SCShop_Pyro_RestStop_Rund_Admin` template occurs alongside these locations. A shared template cannot resolve a terminal by itself, and temporal proximity alone has not validated each code-to-terminal pair.

Six `SendCommodityBuyRequest` records were also found. Transaction fields include `price`, `shopPricePerCentiSCU`, `quantity`, `boxSize` and `unitAmount`. Their values were neither retained nor used. Transaction records supply no price, SCU, status or sell-assortment witness (07 §2.5 item 6).

## Mapping evidence and unresolved candidates

The installed loose English `global.ini` supplies the labels below. Its provenance against the current packaged game data was not established: the packaged English entry was found, but the available ZIP reader rejected its compression method. Consequently these are corroborating names and candidates, not a validated current-build mapping registry.

| Internal name or code | Local label | Public UEX cross-check on 2026-11-01 | Status |
|---|---|---|---|
| `ResourceType.PlasmaFuel` | `items_commodities_type_plasmaFuel=Hydrogen Fuel` | Commodity 104, Hydrogen Fuel | Paired capture corroboration below; current packaged localisation not verified |
| `ResourceType.RMC` | `items_commodities_type_RMC=Recycled Material Composite` | Commodity 63, Recycled Material Composite | Paired capture corroboration below; current packaged localisation not verified |
| `ResourceType.QuantumFuel` | `items_commodities_type_quantumFuel=Quantum Fuel` | Commodity 87, Quantum Fuel | Paired capture corroboration below; packaged provenance unverified |
| `ResourceType.Waste` | `items_commodities_type_waste=Waste` | Commodity 79, Waste | Name agreement only; no independently inspected card in this review |
| `RR_P2_L4` | Checkmate | Commodity terminal 436, Admin - Checkmate, at Checkmate Station | Location candidate only; UEX also lists other terminal types there |
| `RR_P3_L3` | Patch City | Commodity terminal 453, Admin - Patch City, at Patch City | Location candidate only; the admin template does not prove identity |
| `RR_P6_LEO` | Ruin Station, visible in paired capture | Commodity terminal 466, Admin - Ruin Station | Corrected 2026-11-01: paired correspondence below |
| `ResourceType.AltruciaToxin_unprocessed` | Revenant Tree Pollen (follow-up below) | Commodity 66, Revenant Tree Pollen; distinct from commodity 4, Altruciatoxin | Corrected 2026-11-01: paired buy capture and explicit localisation key corroborate pollen; never discard the suffix |
| `ResourceType.ShipAmmoSize1` through `ResourceType.ShipAmmoSize7`, `ResourceType.ShipCountermeasuresDecoy`, `ResourceType.ShipCountermeasuresNoise` | Loose keys name Ship Ammunition - Size 1 through 7, Ship Decoy Countermeasures and Ship Noise Countermeasures | Selected UEX names include Ship Decoy Countermeasures 208 and Ship Noise Countermeasures 209 | Candidate labels; no complete independently inspected mapping validation |

Other observed internal commodity names are `AgriculturalSupplies`, `Carbon`, `DistilledSpirits`, `Dynaflex`, `Helium`, `Mercury`, `Potassium`, `Pressurized_Ice`, `ProcessedFood` and `Silicon` (all under `ResourceType`). Selected public UEX names have plausible counterparts, but removing punctuation or comparing similar strings does not validate their identities. No automatic mapping is enabled by this review.

Public credential-free sources: [UEX commodities](https://api.uexcorp.uk/2.0/commodities), [UEX terminals](https://api.uexcorp.uk/2.0/terminals) and [UEX game versions](https://api.uexcorp.uk/2.0/game_versions). These were GET-only spot checks; no credentials or API writes were used and no price table was retained.

## Version and capture matching

The public `game_versions` response on 2026-11-01 names LIVE `4.10.2` and PTU `4.10.3`. The local manifest branch agrees with LIVE on this occasion. That coincidence does not establish a stable conversion from the four-part build version or branch to the UEX patch string. The R-CAP-3b witness remains major/minor only; a differing major or minor version makes the capture uncertain and cannot make it certain.

Corrected 2026-11-01: the initial survey overlooked the existing public 4.10.2 captures dated 2026-10-14. Their `capturedAt` values agree with all 150 recorded original screenshot filenames; the metadata pins the original SHA-256 and describes lossless redaction. No capture time was inferred from a later file modification time. All four entries remain `verified: false`: these timing measurements and selected pixel inspections do not make their transcriptions golden data.

Offsets below use UTC capture time minus the latest preceding valid shop-inventory row in the reviewed logs. The row must contain all three markers: `LoadShopInventoryData`, `AddingCommodityBox` and `Available Box Sizes`. Taking the latest row of the preceding load avoids a future load and excludes player inventory. The latest location-inventory code before that row agrees with the latest code before the capture in every case. The measured row times retain milliseconds; capture metadata has only whole seconds, so the displayed decimal offsets do not imply subsecond capture accuracy.

| Corpus entry | Captures | Offset range (seconds) | Median (seconds) | Captures within 300 seconds |
|---|---:|---:|---:|---:|
| `ruin-station-01` | 27 | 7.958–88.958 | 52.958 | 27 |
| `patch-city-01` | 11 | 4.812–35.812 | 16.812 | 11 |
| `seraphim-station-01` | 26 | 9.723–69.226 | 39.475 | 26 |
| `levski-01` | 86 | 5.665–104.013 | 51.876 | 86 |
| Combined follow-up dataset | 150 | 4.812–104.013 | 45.120 | 150 |

These include buy and sell screenshots. For sell captures the match can support location only, never sell assortment or container sizes. Keep A24's separate 2026-10-09 dataset unchanged: 76 captures, 7–170 seconds, median 55 seconds. The new observations support retaining the existing five-minute starting window; they do not establish a new maximum or safety margin across patches. A future load or `CaptureTimeUncertain` capture remains ineligible under 07 §2.5 item 6.

## Paired location and commodity follow-up

The selected screenshots were inspected directly, independently of `expectedTerminalId` and card transcriptions. The table records observed correspondences for the reviewed 4.10.2 build, not an exhaustive mapping registry. Public UEX GETs on 2026-11-01 identify one commodity terminal at each of these named locations; other terminal types are not interchangeable with it.

| Log location and shop template | Independently visible evidence | Exact UEX commodity terminal | Limit |
|---|---|---|---|
| `RR_P6_LEO` + `SCShop_Pyro_RestStop_Rund_Admin` | `ruin-station-01/buy-1.jpg`: location field `RUIN STATION`; debug overlay also gives `RR_P6_LEO` | 466, Admin - Ruin Station, at Ruin Station | Supports this paired correspondence; the shared template alone does not identify it |
| `RR_CRU_LEO` + `SCShop_Admin_lt_base_g` | `seraphim-station-01/buy-1.jpg`: `SERAPHIM STATION` and buy panel | 259, Admin - Seraphim, at Seraphim Station | Supports this paired correspondence, not other users of the template |
| `Nyx_Levski` + `SCShop_Levski_CargoOffice_Commodities` | `levski-01/v1-buy-5.jpg`: `Levski` and buy panel | 778, Levski, at Levski | Supports this paired correspondence |
| `RR_P3_L3` + `SCShop_Pyro_RestStop_Rund_Admin` | `patch-city-01/buy-1.jpg`: overlay gives `RR_P3_L3`, but the location field says `SELECT LOCATION` | Candidate 453, Admin - Patch City | Exact identity remains unvalidated: the unverified expected terminal and loose localisation label do not replace an independently established station name |

The installed loose localisation file is not the file pinned by these captures: current SHA-256 `10f4636f5408fee3aabd780857907e4c3555aefeff5a14648b4e3e785b68625c`, 11,267,066 bytes, versus corpus SHA-256 `a9e6e4b126c61a97fd6ccbe7ad846f2d6522534eb347a90cab276853c36023ec`, 10,715,737 bytes. The original pinned localisation file was not available in this review. No claim of packaged or capture-time localisation provenance follows from the current loose file.

Selected aliases have stronger evidence than similar names: explicit loose localisation keys agree with independently visible buy cards, their paired internal names and size sets, and exact public UEX names. This corroborates the following correspondences for the reviewed captures; it does not validate unrelated aliases or establish packaged localisation provenance.

| Internal commodity | Independently inspected capture | Visible name / exact UEX commodity | Size set visible on screen and present in paired load |
|---|---|---|---|
| `ResourceType.PlasmaFuel` | `levski-01/v1-buy-5.jpg`, `seraphim-station-01/buy-7.jpg` | Hydrogen Fuel / 104 | `{1,2,4,8,16,24,32}` |
| `ResourceType.QuantumFuel` | Same captures | Quantum Fuel / 87 | `{1,2,4,8,16,24,32}` |
| `ResourceType.RMC` | `seraphim-station-01/buy-7.jpg` | Recycled Material Composite / 63 | `{1,2,4,8,16,24,32}` |
| `ResourceType.AltruciaToxin_unprocessed` | `ruin-station-01/buy-1.jpg` | `[!] Revenant Tree Pollen` / 66 | `{24,32}` |

The last correspondence is also named explicitly by `items_commodities_altruciatoxin_unprocessed` in the loose file. It is a concrete counterexample to removing `_unprocessed`: UEX's Altruciatoxin is commodity 4, a different name and ID. `ResourceType.Hydrogen` is also distinct from `PlasmaFuel`: the Seraphim buy screen shows Hydrogen with `{8,16,24,32}`, and UEX names Hydrogen as 41 and Hydrogen Fuel as 104. No price, quantity or status from the log was used to establish these correspondences.

## Synthetic format specimens

These are documentation fixtures for the observed payload shapes, not raw excerpts, executable parser tests or accepted real mappings. The omitted prefix and suffix are deliberate: they do not specify a complete grammar. Names, IDs, locations, timestamp and size combinations are synthetic. Production storage would contain only allowlisted witness facts, never the specimens' private-field sentinels.

Buy-side load candidate payload:

```text
<2000-01-01T12:00:00.000Z> [Notice] ... CEntityComponentCommodityUIProvider::LoadShopInventoryData ... AddingCommodityBox - playerId[SYNTHETIC_PLAYER_ID] shopId[SYNTHETIC_SHOP_ID] shopName[SCShop_SYNTHETIC_Admin] commodityName[ResourceType.SyntheticCommodity] Available Box Sizes:  boxSize[1] boxSize[2] ...
```

Expected interpretation: candidate buy-side internal commodity and sizes at the log time; no resolved terminal or UEX commodity until separately validated. Drop both identifier fields immediately. Nothing supplies a reported value.

Player-inventory exclusion payload:

```text
<2000-01-01T12:00:01.000Z> [Notice] ... CEntityComponentCommodityUIProvider::AddPlayerCommodityItem ... AddingCommodityBox ... commodityName[ResourceType.SyntheticCommodity] ...
```

Expected interpretation: reject as kiosk assortment despite the shared keyword. Do not infer sell assortment.

Location payload:

```text
<2000-01-01T11:59:59.000Z> [Notice] ... <RequestLocationInventory> Player[SYNTHETIC_HANDLE] requested inventory for Location[Synthetic_Location] ...
```

Expected interpretation: candidate location code at the log time; drop the handle immediately. The invented code has no mapping.

Synthetic matching examples under the existing rule: with a certain capture at `12:04:00Z` and the latest validated kiosk load at `12:00:00Z`, the four-minute offset is inside the default window. A capture at `12:06:00Z` is outside it; a load after the capture and a `CaptureTimeUncertain` capture are ineligible. These examples illustrate the requirement, not a newly measured offset.

## Completed initial validation and unsupported scope

Corrected 2026-11-01: the research spike for issue #107 is complete for the finite initial allowlist below. The earlier partial status incorrectly treated exhaustive mapping coverage as a completion prerequisite. R-OCR-20 requires mappings validated per game version, and the M2 spike calls for the first validated format and matching window; neither requires every location or internal commodity name to be mapped. The two issue acceptance criteria are met by the current-version buy-side and size evidence, unavailable sell-side facts, limited version witness, and privacy-preserving findings and synthetic specimens.

The validated initial scope is:

- **Build and format:** build `4.10.197.2878`, branch `sc-alpha-4.10.2`; the shop-inventory payload with all three required markers, the location-inventory shape and the player-inventory exclusion documented above. This does not validate every 4.10 build or a future patch.
- **Terminal mapping allowlist:** `RR_P6_LEO` + `SCShop_Pyro_RestStop_Rund_Admin` → terminal 466; `RR_CRU_LEO` + `SCShop_Admin_lt_base_g` → terminal 259; `Nyx_Levski` + `SCShop_Levski_CargoOffice_Commodities` → terminal 778. The paired location and template are required evidence; no template is a terminal identity by itself.
- **Commodity mapping allowlist:** `ResourceType.PlasmaFuel` → commodity 104; `ResourceType.QuantumFuel` → 87; `ResourceType.RMC` → 63; `ResourceType.AltruciaToxin_unprocessed` → 66. The independently inspected captures, explicit labels and exact UEX cross-checks above establish this selected scope without claiming packaged-localisation provenance.
- **Matching:** retain the existing five-minute starting window, supported by 150 measured pairs. Only the latest preceding valid load within the window and a certain capture time can count. Buy-side assortment and sizes remain distinct from the location-only use for sell captures; the version witness remains major/minor only.

The Patch City candidate, Checkmate candidate, every unlisted location/template or commodity mapping, and other builds remain unsupported by this initial allowlist. Further independently established mappings can extend coverage; similar names, discarded suffixes and unverified corpus expectations cannot. The original capture-time localisation and current packaged-localisation provenance remain unverified, without invalidating the independently inspected paired-capture evidence.

The M2 research-spike checkbox is complete. Corrected 2026-11-01: issue #108 implements the initial source boundary described below, with finite mappings, opt-in/default-off behaviour, unsupported-format handling and independent privacy checks at the present output/storage boundaries. This document introduces none of that implementation, no persistent game-log data, normative change or API write. Corpus verification flags remained false at that checkpoint; selected pixel inspection does not make an entry golden data.

## Finite mapping extension — 2026-11-03

The source allowlist additionally accepts the exact terminal pairs and literal commodity names below for the same build `4.10.197.2878` and branch `sc-alpha-4.10.2`. The three initial terminal pairs and four commodity aliases remain unchanged. There is no general name normaliser or cross-template fallback.

The independent clock review used owner-confirmed original capture filenames in MEZ (UTC+01:00), converted to UTC at whole-second precision. Nineteen captures from `mic-l2-long-forest-station-01` and `orbituary-01` follow the latest preceding eligible load by 4.903–58.422 seconds. Their opaque filename suffixes supply no milliseconds. Direct inspection of seven public redacted buy PNGs establishes both visible station labels and 23 complete-card name/size agreements. These are paired observations, not verified golden metrics; twelve sell captures support location only.

Only one complete, stable, strict UTF-8 backup supplied this evidence: 900,762 bytes, SHA-256 `090a46f59a6148ebe6b5e408d11aac7fd4fc976b74a86239009c1783839d9e7b`. Its timestamped build/branch headers agree exactly with the manifest. Its latest preceding location requests establish `RR_MIC_L2` before the admin load and `RR_P3_LEO` before the Pyro rest-stop admin load, with no intervening location change during either capture sequence. Two other inspected sources were wholly refused as invalid UTF-8; none of their headers or rows participates. The retained sanitised clock receipt has SHA-256 `75af9ac1070d2a632f8253fb44da3813730382d060923b73aaf26bbbf0a94b84`.

Separate credential-free first-party GETs on 2026-11-03 at 08:51:48–49 UTC identified the current public UEX identities. The selected [terminal](https://api.uexcorp.uk/2.0/terminals) rows each have numeric `is_available=1`, `is_available_live=1`, `is_visible=1` and `is_player_owned=0`. Each is the unique `commodity` terminal at both the exact display label and station ID; other terminal types at that station are excluded.

| Exact location + shop template | Terminal identity | Selected location identity |
|---|---|---|
| `RR_MIC_L2` + `SCShop_Admin_lt_base_g` | 55, Admin - MIC-L2; code `MICL2`; Commodity Shop - Admin - MIC-L2 | MIC-L2 Long Forest Station, station 18; Stanton 68; MicroTech 190; microTech Lagrange Point 2, orbit 340 |
| `RR_P3_LEO` + `SCShop_Pyro_RestStop_Rund_Admin` | 443, Admin - Orbituary; code `ORBIT`; Commodity Shop - Admin - Orbituary | Orbituary, station 32; Pyro 64; Bloom, planet/orbit 242 |

There are 13 terminal rows at the MIC-L2 display and 17 at Orbituary. In particular, Refinery Ore Sales 240 and 691 have type `commodity_raw` and are excluded, as are refinery, item, fuel and vehicle terminals. Terminal `game_version=4.0` is last-update metadata, not the capture patch. The separate [game versions](https://api.uexcorp.uk/2.0/game_versions) response names LIVE `4.10.2` and PTU `4.10.3`; neither observation widens the accepted source build or proves PTU eligibility.

| Exact internal commodity | Unique exact public UEX name / ID | Independently visible buy sizes |
|---|---|---|
| `ResourceType.Astatine` | Astatine / 9 | `{1,2,4,8,16}` |
| `ResourceType.Carbon` | Carbon / 102 | `{8,16,24,32}` |
| `ResourceType.Copper` | Copper / 20 | `{8,16,24,32}` |
| `ResourceType.Diamond` | Diamond / 25 | `{1,2,4,8,16}` |
| `ResourceType.Potassium` | Potassium / 116 | `{8,16,24,32}` |
| `ResourceType.Scrap` | Scrap / 67 | `{1,2,4,8,16}` |
| `ResourceType.Waste` | Waste / 79 | `{1,2,4,8,16,24,32}` |

Each name matches exactly one of the 205 [commodity](https://api.uexcorp.uk/2.0/commodities) rows; all seven IDs are distinct positive integers and the numeric availability, LIVE and visibility flags are 1. Their literal internal suffixes agree with directly inspected visible names, ignoring case only in the research comparison. Runtime matching remains exact and case-sensitive. The retained minimal identity projection has SHA-256 `ab312aad2bf7ba5f8480c275a1f4ee0c66a0613d34c8d23be4d241024de7978a`; current UEX identity evidence is separate from the capture/source pairing.

MedicalSupplies and the seven ShipAmmoSize plus two ShipCountermeasures aliases remain outside this extension. Their visible UEX identities and equal size sets do not independently establish their internal-name bridges. A qualified loose-localisation supplement does not establish packaged or capture-time provenance. No price, quantity or status from the log supplies evidence. The five-minute matching rule, certain-time prerequisite, buy-only assortment/sizes and sell location-only rule remain unchanged. Corpus PNGs, IDs, expected values, gameVersion and every verification flag are untouched; this extension makes no runtime or package acceptance claim.

## Initial source implementation and local format check

Implemented 2026-11-01 for R-OCR-20: the `game` application API supplies immutable typed buy-side observations with shared-kernel timestamps and IDs. `adapter-files` owns the bounded streaming parser and exact build/branch and name allowlists above. Unknown mappings remain absent and are never inferred from similar strings or shared shop templates. Player-inventory and transaction records create no kiosk facts. Each row retains its own UTC timestamp at millisecond precision; a future location line cannot identify an earlier row.

The authorised local read-only format check on 2026-11-01 confirmed that manifest `Data.Version` and `Data.Branch` are strings. The current log and three locally reviewed backups put a UTC timestamp and one space before `FileVersion: <build>` and `Branch: <branch>`, without a suffix. No raw header was copied. A synthetic specimen of that header shape is `<2000-01-01T11:00:00.000Z> FileVersion: 4.10.197.2878`; the parser checks its timestamp and exact validated value. The production eval command against the then-current log returned an enabled validated source with zero kiosk observations. That practical check proves manifest/header acceptance, not real-row parsing or a new capture comparison; the anonymised parser fixtures pin the row semantics.

Game files are never opened for writing. Read limits are 64 KiB per manifest, 128 MiB per log and 16 KiB per line. A malformed recognised payload, unsupported header/build, unreadable source or exceeded bound supplies a closed unavailable reason and no partial snapshot. Raw lines and all non-allowlisted fields remain local to the parsing call and never enter a result or error. The public models cannot carry a reported price, quantity or status, and implement no Reader or submission interface.

Opt-in and the local channel directory persist atomically in config schema 2; channel changes disable access. Settings and explicit `ocr-eval witness --channel <directory>` inspection show only sanitised facts. Real-file tests prove unchanged game-file bytes, finite identities, player/transaction exclusions and private-field sentinel removal from typed debug and allowlisted JSON/CLI output; configuration tests cover migration and safe rejected-value messages. This source does not add logging or a diagnostics exporter merely to test them. Existing result/error formatting is tested, and future logging, diagnostics and Scan storage integrations must independently preserve the same allowlist and the G1 export exclusion. Current facts are not persisted with a Scan; that ownership arrives with the later Scan persistence workflow.

The later recognition resolver still owns latest-preceding-load/certain-capture matching within the five-minute starting window, location-only use on sell captures, assortment/size witnessing on buy captures, and the major/minor-only version rule. No OCR, resolution, confidence, report or submission result is manufactured by this source implementation.
