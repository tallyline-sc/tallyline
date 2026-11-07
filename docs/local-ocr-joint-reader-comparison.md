# Existing local OCR readers: joint literal agreement record

> **Doc type:** Historical record — completed reconciliation of retained development outputs; accuracy and production combinations remain unmeasured. Last reviewed: 2026-11-02.

Corrected 2026-11-04: verification states, counts and hashes below describe the recorded run, whose references were unverified at that time. The owner has since marked all seven existing public entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Historical artefacts and expected-file hashes are unchanged and do not bind the current annotations; this decision supplies no retrospective accuracy measurement.

## Outcome and scope

Reusing the completed [Windows PP-OCRv6 nine-pair experiment](local-ocr-nine-pair-experiment.md) and [Linux Tesseract best/fast PSM 11 experiment](local-ocr-tesseract-experiment.md) yields eleven reader profiles over the same five development crops. This data-only comparison covers 21 complete cards, 63 manually associated fields and all 55 reader pairs. It adds no model execution, native build, model download or product configuration.

There are 693 profile/field slots: 691 contain associated text and two omit it. All eleven readers have the same nonempty normalised literal on 23 fields: zero prices, eleven quantities and twelve statuses. Small/Small and Tesseract best have 25 equal, 37 unequal and one unavailable pair of literals; Small/Small and fast have 33 equal, 29 unequal and one unavailable. These are agreement counts. Literal differences can be harmless formatting differences, and matching literals can retain the same error.

All five references remain `verified:false`. This development set was already inspected and used for earlier experiments; no development/calibration/held-out split or human-verified accuracy result is created by combining its outputs. The [strictly local delivery constraints](adr/0012-strictly-local-recognition-delivery.md), Small production default and pending production recognition pipeline remain unchanged.

## Bound evidence and references

The combined packet was recorded at `2026-11-02T14:24:51.124Z`. Its inputs retain their original execution identities: PP-OCR source commit `7b017900aeb7fc33953b2d588845654b241df458`, tree `a88845597242577e545fbe7a5a46e173a451f007`; Tesseract source commit `b019d6d936afe858f05b1cd0921d033821647697`, tree `2860b5c47e9c389b39f31df3fc787ee9f8759982`. Combining those outputs does not rebind either native campaign to a newer integration. Model, crop, worker and resource identities remain in the two campaign records above.

| Retained evidence | SHA-256 |
|---|---|
| PP-OCR 567-slot source field packet | `45d3b50bd62958b6d09fc5f99c18aa359ee6de4238b4074c3bf5ebed213a3155` |
| Tesseract 252-slot source field/condition packet | `50bb6c82c93f73e0eb47494be12c6839d207d4277307ed3bea6a3a816fcb7782` |
| Combined `literal-agreement-v2.json` | `bda31d319cac456070216ae7407cd8c2ec2f4400a6fa3df82a1c010301a65341` |
| Combined 63-field human-readable inventory | `114dda2b734816dca774b1f4728ed2a2dc49d921ca65ea6209d04e670251399b` |
| Independent read-only reconciliation result | `a5c7527a7b62b9916901a2957380e2573959513ab9f4e4d3e77815aef45d8082` |
| Reconciliation script | `03502ba1836c53166287d48d1bb73838585cc8c41bca5e98f73bbcd8235e39ed` |

The source packets, raw outputs, local scripts and detailed comparison remain private retained research evidence; they are not committed or shipped. This public record contains source identities and derived inventories. Reproducing the complete field audit requires that retained packet, rather than the public tree alone.

The reconciliation checked source-packet hashes, all 45 PP raw-file hashes and associated line text/geometry, all twenty Tesseract receipt/TSV/word-packet hashes, direct TSV reparsing, every retained word receipt/index, the five shared PNG identities and corresponding BMP identities, all 693 slots, exact normalisation/clusters and all 55 pair counts. It did not reassess pixels or establish correct field association, automatic layout or human verification. The two omitted normalised literals are represented as `null`; they are excluded from equal/unequal nonempty counts.

The following are the packet's frozen reference identities. At integration commit `d75dd800d17403a1eccc027d907b3cc47510ac6e`, all five files still matched these bytes, all 63 referenced values matched and every verification flag was false. Future corpus edits do not silently update this result.

| Source capture | Complete cards / fields | Frozen `expected.json` SHA-256 |
|---|---:|---|
| `corpus/public/levski-01/v3-sell-5.jpg` | 3 / 9 | `a10e59b84fb41b1cfa6cabd16e923e278436063ca5bea3ff9e433d4aa6a00c78` |
| `corpus/public/patch-city-01/buy-1.jpg` | 4 / 12 | `97492535338b24862bda55be0343f4988225310cf21de6a7fca258729a7d34f7` |
| `corpus/public/pyro-gateway-stanton-01/buy-1.jpg` | 4 / 12 | `e9638c8e292e963904afaad01c9e58bcefd6c526a2280ee3efe6be93d450957e` |
| `corpus/public/ruin-station-01/sell-1.jpg` | 3 / 9 | `d1ad91b16d608113983fd57aab3e7a6d48004104dcba6d50c4e8de20879e8150` |
| `corpus/public/seraphim-station-01/unselected-buy-1.jpg` | 7 / 21 | `5e9d1cfc21bcd2f77a5d360e1d831617352d83a426eb3e914d42fc63f1eff1df` |

Partial cards remain excluded. The four MIC-L2 and fifteen Orbituary captures added to the later public corpus have no recognition measurements in this packet. Their transcriptions and the existing five references are not regenerated from reader answers, and agent sight review does not change human verification status.

Corrected 2026-11-02: the retained initial combined packet labelled full Tesseract word objects as `word_indices`. Version 2 distinguishes the integer `word_indices` from the full `words` receipts. The initial packet remains separately retained with SHA-256 `6245a38add11c0cbabcac2aa30e661583c07443a1610a0c8a45dbe1875c80844`; it is not the clean metadata source for this record. The correction changes metadata labels without changing OCR literals or comparisons.

## Method and reader identities

Each field uses the earlier post-hoc manually reviewed anchor neighbourhood. PP-OCR associates whole detected lines; Tesseract associates words. Both source diagnostics use the recorded centre/overlap conditions, but these distinct units can include different surrounding material. The comparison therefore concerns associated strings, rather than a common frozen production field parser. Names, side, container sizes, completeness detection, vocabulary resolution and submission are outside its 63-field scope.

For nonempty comparisons, trim and collapse only ASCII whitespace and uppercase only ASCII `a`–`z`. Preserve digits, currency glyphs, separators, punctuation, unit letters and all other characters. No `O`/`0` substitution, numeric extraction, glyph stripping, locale repair, reference-based choice or confidence weighting is applied. For example, `2,850/SCU` and `2, 850/SCU` remain different, as do `OSCU` and `0SCu` after ASCII case conversion.

| ID | Reader profile | Retained slots / nonempty literals |
|---|---|---:|
| TT | PP-OCRv6 Tiny detector / Tiny recogniser | 63 / 63 |
| SS | PP-OCRv6 Small detector / Small recogniser | 63 / 63 |
| MM | PP-OCRv6 Medium detector / Medium recogniser | 63 / 63 |
| TS | PP-OCRv6 Tiny detector / Small recogniser | 63 / 63 |
| TM | PP-OCRv6 Tiny detector / Medium recogniser | 63 / 63 |
| ST | PP-OCRv6 Small detector / Tiny recogniser | 63 / 63 |
| SM | PP-OCRv6 Small detector / Medium recogniser | 63 / 63 |
| MT | PP-OCRv6 Medium detector / Tiny recogniser | 63 / 63 |
| MS | PP-OCRv6 Medium detector / Small recogniser | 63 / 63 |
| TB | Tesseract English best, PSM 11 | 63 / 62 |
| TF | Tesseract English fast, PSM 11 | 63 / 62 |

The nine PP profiles share one model family and only three recogniser weight sets. Best/fast share the Tesseract architecture and training lineage. All ten same-crop/model 4-/8-GiB Tesseract TSV pairs are byte-identical; each model contributes one profile. These eleven profiles do not constitute eleven independent votes. A changed runtime over the same weights would add runtime diversity without establishing an independent OCR model.

## Complete pairwise literal inventory

Each cell is **equal / unequal / unavailable**, over the same 63 manually associated fields. An unavailable field has no associated text from at least one reader. The only such field is Patch City's DynaFlex quantity for TB and TF; their mutual cell counts that field once as unavailable. Each upper-triangle cell is one of the 55 unordered pairs. The lower triangle repeats no information.

| Reader | TT | SS | MM | TS | TM | ST | SM | MT | MS | TB | TF |
|---|---|---|---|---|---|---|---|---|---|---|---|
| TT | — | 45/18/0 | 40/23/0 | 45/18/0 | 41/22/0 | 51/12/0 | 43/20/0 | 48/15/0 | 45/18/0 | 26/36/1 | 36/26/1 |
| SS | — | — | 46/17/0 | 60/3/0 | 47/16/0 | 43/20/0 | 47/16/0 | 42/21/0 | 60/3/0 | 25/37/1 | 33/29/1 |
| MM | — | — | — | 46/17/0 | 56/7/0 | 40/23/0 | 57/6/0 | 37/26/0 | 46/17/0 | 25/37/1 | 33/29/1 |
| TS | — | — | — | — | 47/16/0 | 43/20/0 | 47/16/0 | 42/21/0 | 63/0/0 | 25/37/1 | 33/29/1 |
| TM | — | — | — | — | — | 41/22/0 | 57/6/0 | 39/24/0 | 47/16/0 | 25/37/1 | 34/28/1 |
| ST | — | — | — | — | — | — | 41/22/0 | 52/11/0 | 43/20/0 | 26/36/1 | 35/27/1 |
| SM | — | — | — | — | — | — | — | 39/24/0 | 47/16/0 | 25/37/1 | 34/28/1 |
| MT | — | — | — | — | — | — | — | — | 42/21/0 | 24/38/1 | 33/29/1 |
| MS | — | — | — | — | — | — | — | — | — | 25/37/1 | 33/29/1 |
| TB | — | — | — | — | — | — | — | — | — | — | 31/31/1 |
| TF | — | — | — | — | — | — | — | — | — | — | — |

TS and MS produce equal normalised literals on all 63 fields despite using different detectors. That observation concerns this development set and this association method; it does not prove equivalent geometry, identical raw output, independent agreement or equal accuracy on other captures. TB and TF agree on 31 of their 62 jointly nonempty fields.

| Field | Associated fields | Same nonempty literal across all eleven |
|---|---:|---:|
| Price per SCU | 21 | 0 |
| Quantity SCU | 21 | 11 |
| Status | 21 | 12 |
| Total | 63 | 23 |

Zero universal price agreement does not mean every price is incorrect: the literal method deliberately retains currency, unit and formatting differences. Conversely, universal agreement is not a correctness check.

## Complementarity and shared-error examples

These selected raw literals are already documented in the two campaign records; their unverified references identify the field without contributing a reader vote. The full retained inventory includes every field and all mixed PP profiles.

| Capture / material / field | Existing reference | Observed associated literals | Interpretation limit |
|---|---|---|---|
| Levski / BEXALITE / quantity | 0 | All nine PP profiles and TF: `OSCU`; TB: `0SCu` | Ten agreeing profiles preserve letter `O`; a majority would discard the distinct zero observation. |
| Patch City / Omnapoxy / quantity | 601 | PP profiles: `601 SCU`; TB: `501, SCI`; TF: `601 SCU` | The larger Tesseract model changes a numeric digit in this example. |
| Patch City / DynaFlex / quantity | 45 | PP profiles: `45 SCU`; TB and TF: no associated text | An omission remains an omission; agreement on absence cannot supply the field. |
| Seraphim / HELIUM / price | 772 | SS: `772/SCU`; MM: `¤772/SCU`; TB and TF: `n772/SCU` | Same-family agreement can retain the same false currency reading. |
| Seraphim / FLUORINE / price | 1090 | SS: `1,090/SCU`; MM: `81,090/SCU`; TB: `r1,080/SCU`; TF: `41,090/SCU` | Digit changes and false leading currency digits differ by model; neither largest-model nor minority selection supplies a general rule. |

These examples motivate measuring added error detection, joint wrong agreement, false corrections and review burden. They do not establish a useful fusion combination, an automatic repair, an independent-witness decision, calibrated confidence or increased sendability. The experiment supplies observations for designing that evaluation.

## Resource and acceptance boundary

The earlier resource observations remain separate: Windows PP timings include Cargo/process/model-load overhead; Linux Tesseract timings include container lifecycle, admission, loading and cleanup. Their counters cover different process boundaries, have uncontrolled concurrent development/cache state and do not establish whole-application peaks. Summing those timings or memory observations would not measure a combined deployment.

Only one physical development PC was available. Tesseract's 4-/8-GiB worker limits restrict resources inside that PC's VM; they do not simulate a representative weak CPU, memory subsystem or complete weak computer. No weak-hardware performance, game-impact, whole-process-tree peak or cross-platform performance ranking follows. No clean installation, bundled offline lifecycle or removal of the existing Windows OCR prerequisite was demonstrated by this comparison.

The next accuracy step remains human verification and a frozen split by capture session/order and terminal, followed by blind single-reader evaluation and deterministic combination replay on a shared frozen parser. Retain all raw strings and report paired joint errors, missed fields, false corrections and review burden alongside resource outcomes; do not rank pairs by literal agreement alone. The [runtime research evaluation plan](local-vision-runtime-research.md#10-concrete-evaluation-program) describes those remaining stages. Further native/runtime/model experiments need their own exact admitted source, licence, feature, lifecycle and execution scope; this data reconciliation grants no new approval.
