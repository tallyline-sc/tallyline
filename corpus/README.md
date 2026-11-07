# OCR Test Corpus

This directory contains the **public** part of the golden corpus (see `docs/plan/07-ocr-concept.md` §4). The private part lives outside the repo and is included via `TALLYLINE_CORPUS_DIR`.

## Rules

- The balance ("CURRENT BALANCE") is **always redacted** before an image ends up here. No player names, no chat windows.
- Each entry has a directory `<location>-<nr>/` with the images and `expected.json`.
- `expected.json` stays at `"verified": false` until a human has independently checked the transcription. Unverified entries count in the eval only as "smoke", not as a golden metric. Unverified entries and entries with `gameVersion: null` never validate a game version for a layout profile (R-OCR-19).
- **Original screenshots** (lossless, original resolution) are preferred. Images uploaded via chat are probably scaled and recompressed; this is noted in `source` (schema version 2: `sourceFidelity`).
- An entry with `sourceFidelity: reencoded-unknown` (assumption A6, e.g. `pyro-gateway-stanton-01`) never counts for a resolution or capture-method class. It counts only for its theme and text-height band and as a source for synthetic variants.
- **Real class captures go into the public corpus by default**, after redaction, so that CI can gate their classes. A capture is added only if the contributor took it themselves or the person who captured it consented, and the contributor agrees to its publication in `corpus/public/` ([CLA](../CLA.md) §6, [CONTRIBUTING.md](../CONTRIBUTING.md)). The private corpus holds only captures that cannot be redacted without destroying the evaluated region. A class that exists only privately has the status `insufficient` in CI and is evaluated locally before every release (`docs/release-process.md`).
- **Redaction** uses filled rectangles over the balance and personal data (player names, chat, and the `Session:` line of the `r_DisplayInfo` debug overlay when it is on) only. It must not touch the shop panel, the location field or the anchors, and it must not re-encode the rest of the image. PNG originals stay PNG. JPEG originals are redacted with a block-level lossless wipe (e.g. `jpegtran -wipe`); if that is not possible, they are decoded once and stored as PNG, and `sourceFidelity` records it.
- **Schema version 2** of `expected.json` (07 §4) adds per-capture `conditions` (capture method, HDR, renderer, resolutions, …) and a `measured` object, and moves the section to the card (`sectionHeaders` per capture; `section` and, for partial cards, `partialEdge` per card). Unknown values are `null`, never guessed. `pyro-gateway-stanton-01` was migrated on 2026-11-01 with unknown values set to `null`.
- Schema version 2 entries also carry the expected UEX IDs (`expectedTerminalId`, `expectedCommodityId`), capture times (`capturedAt`) and a frozen `reference/` directory of recorded, anonymised UEX responses (07 §4). Without them an entry counts only as smoke for prior-dependent metrics. Whether UEX data may be published here is open (06 open point 7); until it is cleared, `reference/` directories live in the private corpus under the same entry id.
- **"Report a misread" exports** (R-QA-5) become a new entry only if the contributor allows it in the misread form, under the same consent rule. They contain redacted panel crops, not full frames, so, like `reencoded-unknown` entries, they count only for their theme and text-height band, never for a resolution or capture-method class. The exported values stay `"verified": false` until a maintainer has checked them. The exported reference snapshot goes into the entry's `reference/` directory with `priorProvenance`, in the private corpus until 06 open point 7 is cleared.
- Corpus data outside the Git repository (the private corpus under `TALLYLINE_CORPUS_DIR`) has a `SHA256SUMS` file per entry; the eval verifies it before running.
- **Images are stored in Git LFS** (decided 2026-10-14, O-43): `.gitattributes` routes every `.jpg`, `.jpeg` and `.png` under `corpus/public/` through LFS, so Git holds only pointer files with the SHA-256 of each image. Contributors need `git-lfs` installed before they clone or add images. A clone without LFS objects has pointer files in place of the images; the corpus runner refuses such a file with a visible error and never reports it as passed. CI jobs that read the images check out with LFS and cache the objects; other jobs do not fetch them. CI never fetches unpinned corpus data.

## Licence

The images in `corpus/public/` are screenshots of Star Citizen. The game content they show is copyright Cloud Imperium Rights LLC and Cloud Imperium Rights Ltd. They are **not** covered by the project licence (GPL-3.0-or-later): [`REUSE.toml`](../REUSE.toml) marks them `LicenseRef-Game-Screenshots` ([`LICENSES/LicenseRef-Game-Screenshots.txt`](../LICENSES/LicenseRef-Game-Screenshots.txt)), and [`NOTICE`](../NOTICE) §3 lists them. They are here only as test data for the text recognition; this project grants no rights in them, and any use of the game content is subject to Cloud Imperium's terms. Whether those terms allow the screenshots in a public repository is not verified yet: the project owner checks the RSI Terms of Service and the Fankit and Fandom FAQ (O-28 in [`docs/adr/0000-open-points.md`](../docs/adr/0000-open-points.md)). If they do not, the images are removed.

- The transcriptions (`expected.json`) and every other text file here are under the project licence, GPL-3.0-or-later.
- The images are never shipped in the installers or app archives. They are part of the repository as Git LFS objects (the repository uses no `export-ignore`). GitHub's source archive of a tag holds only their pointer files: the repository setting "Include Git LFS objects in archives" stays off (O-43). To get the images, clone the repository with `git-lfs`.
- Recorded UEX data (`reference/` directories, the reference snapshot of a misread export) gets its own licence marking only once UEX allows publishing it ([06](../docs/plan/06-uex-api.md) open point 7); until then it stays in the private corpus.
- The `REUSE.toml` patterns cover `.jpg`, `.jpeg` and `.png` files under `corpus/public/`; an image in any other format needs a new pattern in the same pull request.

## Entries

## `expected.json` schema

The schema test in `tools/ocr-eval`, tagged `R-QA-1`, validates every `corpus/public/*/expected.json` with `cargo xtask check`. The hand-authored entries use schema version 1 or 2; recognition output must never regenerate them, and `verified` remains `false` until a human independently checks the transcription.

Both versions require an integer `schemaVersion` (1 or 2), string `id`, `transcription` and `theme`, boolean `verified`, object `source` and `location`, and a non-empty `captures` object. Each capture requires `side` (`BUY` or `SELL`) and a `cards` array. Each card has a numeric `screenOrder`. A complete card has `name`, `status`, `scu`, `pricePerScu` and `containerSizes`; `scu`, `pricePerScu` and `containerSizes` may be `null` when unknown. A partial card may omit those complete-card fields and carries `partial: true`.

Version 1 stores the capture's `section`. Version 2 stores `capturedAt`, `conditions` and `sectionHeaders` per capture, `measured` per entry, and `section` per card. `expectedTerminalId` is an optional integer or null entry field. Additional fields documented in [07 §4](../docs/plan/07-ocr-concept.md#4-measurement-and-test-concept) retain their documented types; unknown measured and condition values are represented as `null`, never guessed.

| Entry | Content | Status |
|---|---|---|
| `pyro-gateway-stanton-01` | Pyro Gateway (Stanton), blue theme: 4× Buy (scrolled, without overlap), 1× Sell | owner-verified 2026-11-04 |
| `patch-city-01` | Patch City (Pyro), orange theme, 3840×2160 originals, debug overlay on, no location text: 1× Buy, 10× Sell (scrolled with overlap) | owner-verified 2026-11-04 (schema version 2) |
| `ruin-station-01` | Ruin Station (Pyro), orange theme, 3840×2160 originals, debug overlay on: 5× Buy, 22× Sell (scrolled with overlap; Pitambu never complete) | owner-verified 2026-11-04 (schema version 2) |
| `seraphim-station-01` | Seraphim Station (Stanton), blue theme, 3840×2160 originals: the same pages without a selected inventory (`unselected-*`, 5× Buy, 7× Sell, debug overlay on) and with Seraphim Station selected (7× Buy, 7× Sell) | owner-verified 2026-11-04 (schema version 2) |
| `levski-01` | Levski (Nyx), grey theme, 3840×2160 originals from three positions: front (`v1-*`), closer front (`v2-*`) and oblique (`v3-*`), each 5× Buy and 22 to 26× Sell | owner-verified 2026-11-04 (schema version 2) |
| `mic-l2-long-forest-station-01` | MIC-L2 Long Forest Station, blue theme, 3840×2160 JPEG originals decoded once to balance-redacted PNG: 2× Buy and 2× Local Market Value (`SELL`), with clipped top/bottom cards | owner-verified 2026-11-04 (schema version 2; unknown conditions and IDs remain null) |
| `orbituary-01` | Orbituary, orange theme, 3840×2160 JPEG originals decoded once to balance-redacted PNG: 5× Buy and 10× Local Market Value (`SELL`), including long names, warning markers and clipped cards | owner-verified 2026-11-04 (schema version 2; conservative re-encoded fidelity) |

The originals of `patch-city-01`, `ruin-station-01`, `seraphim-station-01` and `levski-01` and the game's language file `global.ini` in use when they were taken belong in the private corpus; each `expected.json` records their SHA-256.

### Human-confirmed completeness corrections

Corrected 2026-11-01: the project owner confirmed that the container boxes and cards listed below are clipped at the bottom of the Levski list viewport. Each card now carries `partial: true` and `partialEdge: "bottom"`, as required by [07 §2.3](../docs/plan/07-ocr-concept.md#23-layout). The existing names, statuses, quantities, prices, container-size values and commodity IDs are retained as transcription evidence; these partial cards must produce no row and supply no field values for stitching or conflict resolution.

| Capture in `levski-01` | Material | `screenOrder` |
|---|---|---|
| `v2-sell-10.jpg` | HEART OF THE WOODS | 5 |
| `v2-sell-11.jpg` | HYDROGEN | 5 |
| `v2-sell-12.jpg` | KOPION HORN | 5 |
| `v2-sell-13.jpg` | MEDICAL SUPPLIES | 5 |
| `v3-sell-5.jpg` | BORASE | 5 |
| `v3-sell-12.jpg` | HEART OF THE WOODS | 5 |
| `v3-sell-13.jpg` | HYDROGEN | 5 |
| `v3-sell-14.jpg` | KOPION HORN | 5 |
| `v3-sell-15.jpg` | MEDICAL SUPPLIES | 5 |

This 2026-11-01 confirmation covered these nine completeness corrections only. The separate owner verification decision below covers the existing transcription without changing these partial-card boundaries.

## Offline inspection

Corrected 2026-11-01: O-50 and UEX API notes §Terms of use permit small anonymised public reference fixtures for the entry's own terminals and commodities; full price dumps remain forbidden. Earlier statements above that every reference recording must stay private describe the previous default. Larger snapshots, originals and localisation files remain private. The structural scope checks do not replace human anonymisation and fixture-size review.

Use `cargo run -p ocr-eval --locked -- inspect` for schema-v2 metadata, conditions, expected values, frozen reference metadata and sample readiness. See [the capture protocol](../docs/user/corpus-capture.md) for private `SHA256SUMS`, `reference/metadata.json`, missing inputs, LFS failures and separate annotation proposals. This metadata report does not run recognition or validate accuracy. Corrected 2026-11-04: the project owner instructed that all seven existing public corpus transcriptions be marked `verified: true`. This records the verification decision of the owner, not a new agent pixel audit. Future entries and annotation proposals remain unverified until separately confirmed. No transcription, ID, capture time, condition, completeness flag or image changed; missing metadata, frozen references and recognition measurements remain missing.

The MIC-L2 entry records the JPEG-to-PNG route as `sourceFidelity: reencoded-unknown`. Its redaction receipts preserve source hashes and exterior RGB hashes; owner verification does not establish a capture-method, renderer, HDR or validated game-version class. In this entry an empty card `section` means its header is outside the capture, not a section inferred across captures. The displayed `LOCAL MARKET VALUE` tab maps to the existing `SELL` corpus side without asserting sellable-cargo eligibility.

The `orbituary-01` intake uses the same conservative `reencoded-unknown` fidelity class and explicit per-capture source/redaction proof. Its privacy rectangle is specific to Orbituary's lower balance position. The literal `[!]` marker stays in displayed commodity names; no warning marker supplies a hidden identity or eligibility decision. All top/bottom fragments remain partial, including cards whose container digits are visible but whose lower border is clipped.

Corrected 2026-11-02: the project owner confirmed that all four MIC-L2 and fifteen Orbituary original filenames are unchanged game-generated local names in MEZ (UTC+01:00), with no clock change. Their `capturedAt` values therefore convert the filename date/time to UTC at second precision; the opaque hexadecimal suffix supplies no millisecond precision. That confirmation concerned capture times only and did not change transcription verification, fidelity or capture classes. The separate 2026-11-04 owner verification decision does not infer a game version or Game.log correlation.
