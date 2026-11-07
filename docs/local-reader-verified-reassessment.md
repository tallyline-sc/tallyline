# Verified-reference reassessment of retained local readers

> **Doc type:** Historical record — completed post-hoc development replay; no held-out accuracy, production calibration or profile acceptance. Last reviewed: 2026-11-04.

## Result and scope

Small/Small OCR (SS) and the corrected-prompt Qwen3-VL 2B diagnostic (Q2) agree on **52 correct values, zero wrong values and 11 abstentions out of 63 fields**. The parser-only SS baseline emits 52 correct and five wrong values and already abstains on six invalid strings. The cross-check detects five additional parsed errors on this inspected development set. Adding Tesseract fast (TF) reduces unanimous output to 31 correct values without removing another emitted error. This supports a bounded future comparison; it does not establish a deployable fusion rule.

The original larger-VLM main comparison remains structurally unusable for paired accuracy: 8B has **0/1680 assigned fields**, and 9B has **45/1680**, comprising 37 correct and eight invalid fields. All 1680 paired fields abstain. Zero wrong emissions with zero coverage supplies no accepted accuracy result.

This record uses the owner-verified annotations introduced by `19facb1` at source parent `a63dad9737ac7de8087d66ae603f6dcaad6b9625`. It reuses immutable first answers; it runs no recognition, model download, new native dependency compilation, app launch, Docker, WSL, UEX call or external write. The [original OCR](local-ocr-joint-reader-comparison.md) and [VLM](local-vision-first-campaign.md) reports retain their historical unverified status and results. This is a new post-hoc assessment against verified references, not a retrospective claim that those runs were blind Golden evaluations.

The current corpus contains seven verified entries, 174 captures, 622 complete and 236 partial card observations. The old full VLM campaign covers the original 155 captures, 560 complete and 209 partial observations; the OCR and small-VLM diagnostics cover five already inspected crops, 21 complete cards and three fields per card. Four partial cards in those crops stay excluded. The four MIC-L2 and fifteen Orbituary additions have no retained recognition outputs. Their 62 complete and 27 partial observations receive no invented score.

## Frozen inputs and scoring policy

Before any score, `protocol-freeze.json` fixed all 63 OCR field identities, all 55 OCR pairs, all 165 OCR triples, the original VLM main/name diagnostic distinction, a fixed SS/TF/Q2 triple and its pair ablations, and SS/M3. `field-identities-freeze.json` binds each original OCR anchor to its existing capture, `screenOrder` and field. The later retained-input freeze binds the existing VLM field ledgers and original eligibility before scoring. No predicted name, side, price, section, row order, new matching rule or hand-selected easy subset reassigns a field.

All seven current expected files differ from their pre-verification blobs only at the single `verified` token. Replacing that token with `false` reproduces the exact original file bytes and SHA-256 below; the five historical VLM reference copies matched those same original digests at the pre-score freeze. The 63 old OCR values and every retained VLM field reference were separately checked against current Golden fields. No image, completeness boundary, transcription, ID, condition or timestamp changed.

| Entry | Current verified expected SHA-256 | Restoring only `verified:false`: original SHA-256 |
|---|---|---|
| `levski-01` | `200ebc0a9bbddf374d59d7d7407d10133ded0332bb84768bffdd2bece3626a35` | `a10e59b84fb41b1cfa6cabd16e923e278436063ca5bea3ff9e433d4aa6a00c78` |
| `mic-l2-long-forest-station-01` | `5e77f7c4ed920364449681360f73280623628904da99c09bfaff8ccec70a3dee` | `0f4ea5e557ebf602808b45d2c7696d6d5a15351067172e5fddb124f62d4b92ff` |
| `orbituary-01` | `1aecd24d297c961c33af5bb42ad398cc784e0cf9a6cabac99e7e4bd38791464a` | `3833b3d44d50c764e3af7644f19edc5a56d4f4d1bcbe28a7f8782033aae67817` |
| `patch-city-01` | `39ece6d8b476ccd135d637f186ea40f639bc2274e02ea7e85c2f92df8d217f80` | `97492535338b24862bda55be0343f4988225310cf21de6a7fca258729a7d34f7` |
| `pyro-gateway-stanton-01` | `246e11ee6a4152e446408c878d7bda9fa4f72a30ab82f5981644a42246225d6d` | `e9638c8e292e963904afaad01c9e58bcefd6c526a2280ee3efe6be93d450957e` |
| `ruin-station-01` | `ca8e91582564121d44007c9eefe597f3701e4338ac29d9a3287722d333b5e42d` | `d1ad91b16d608113983fd57aab3e7a6d48004104dcba6d50c4e8de20879e8150` |
| `seraphim-station-01` | `68476e7e3e3536d0067557c0b9ed1815b183abed2f4a5721510b04c98443cb08` | `5e9d1cfc21bcd2f77a5d360e1d831617352d83a426eb3e914d42fc63f1eff1df` |

The unchanged original `comparison-parser.ps1` has SHA-256 `265d01d37f3de9bc3b942aa9631550fd156a0e53236ee13359767f62f2aed3fd`; all 27 original fixtures pass. Its earlier policy is `6897812e6f62707487b4f2ee164dd2febb1ab6eb2eba96d71558ab43cb17775d`, and the original main alignment policy is `9919e3a170e3c42e5c713eae53fe12b7f3a9cf4e58dad0542b2b2fb5521e356c`. The same numeric grammar is applied to OCR associated literals as a separately dated development condition. It accepts consistent thousands grouping, the originally admitted currency glyph and exact supported SCU/rate units; it refuses letter/digit substitution, unsupported symbols/units and inconsistent decimal forms. K/M expansion uses exact integer/rational arithmetic and the original unsigned-64-bit limit. No float, rounding, prior, Golden value or second answer repairs a literal. Canonical equivalence does not measure raw glyph accuracy.

| Outcome | Definition and denominator treatment |
|---|---|
| Correct | Supported parsed value equals the exact Golden value; status uses the original NFC/whitespace/ASCII-case literal policy. |
| Wrong | Supported value differs, or a nonempty numeric output fails the frozen grammar. `Invalid` is a reported subset of wrong. Nonmatching status literals, including literal `NULL`, remain wrong under the original policy; this is not proof that each was an intended status reading. |
| Missing | Omitted or explicit null/empty output. The slot remains in the denominator. |
| Unscorable | Original alignment failure or unknown ground truth, with reason retained. Malformed or difficult output is not relabelled unscorable. |

The original OCR inventory retains all 693 reader/field slots: 691 nonempty strings and two omissions. Its 23 universal nonempty literals were agreement counts, not accuracy. The existing post-hoc OCR anchors associate whole PP lines and Tesseract words, so their strings can include different surrounding material. Q2/M3 retain their original predeclared unique-name development diagnostics; they do not acquire the larger campaign's stricter main alignment or full-pipeline status. The CLI trusts the frozen adapter's canonical observations and outcome reasons. It checks the packet hash, current verified corpus hash and exact field identity, then measures them; it does not independently certify the adapter's raw-string parsing. The retained adapter reuses and checks the original parses. Raw literals and their original normalisations remain private beside every canonical observation.

## Singles on the same 63 development fields

TT/SS/MM are Tiny/Small/Medium detector and recogniser pairs; TS, TM, ST, SM, MT and MS name the crossed pair. TB/TF are Tesseract English best/fast, PSM 11. Q2 is corrected-prompt Qwen3-VL 2B Q4_K_M/F16 projector, and M3 is Ministral 3 3B Q4_K_M/BF16 projector. Exact model/runtime/source identities remain in the [nine-pair](local-ocr-nine-pair-experiment.md), [Tesseract](local-ocr-tesseract-experiment.md), [2B](local-vision-small-model-campaign.md) and [3B](local-vision-ministral3b-experiment.md) owning records. No new version is selected.

| Profile | Correct price / SCU / status (each /21) | Correct /63 | Wrong /63 | Of wrong: invalid | Missing /63 |
|---|---:|---:|---:|---:|---:|
| TT | 9 / 18 / 21 | 48 | 15 | 13 | 0 |
| SS | 16 / 15 / 21 | 52 | 11 | 6 | 0 |
| MM | 9 / 15 / 21 | 45 | 18 | 17 | 0 |
| TS | 16 / 15 / 21 | 52 | 11 | 6 | 0 |
| TM | 10 / 15 / 21 | 46 | 17 | 16 | 0 |
| ST | 7 / 18 / 21 | 46 | 17 | 16 | 0 |
| SM | 11 / 15 / 21 | 47 | 16 | 15 | 0 |
| MT | 6 / 18 / 20 | 44 | 19 | 18 | 0 |
| MS | 16 / 15 / 21 | 52 | 11 | 6 | 0 |
| TB | 0 / 14 / 13 | 27 | 35 | 23 | 1 |
| TF | 3 / 16 / 15 | 34 | 28 | 17 | 1 |
| Q2 | 20 / 18 / 21 | 59 | 4 | 4 | 0 |
| M3 | 12 / 18 / 21 | 51 | 9 | 8 | 3 |

All profiles in this table have zero unscorable fields. Wrong includes invalid; the four top-level outcomes always sum to 63. Both Tesseract omissions concern DynaFlex stock. Larger readers are not uniformly more correct under this grammar, and these counts are not a ranking of general digit recognition.

## Pair complementarity and unanimous abstention replay

Every pair accepts only two supported equal values; every other slot abstains. `Neither correct` includes wrong, missing and unscorable outcomes. Parsed disagreement requires two supported differing values. Matching invalid literals are shared failure forms and are never emitted. Only-A/only-B counts show potential complementarity; they do not permit choosing the correct reader using the reference.

| Pair A + B | Both correct / emitted correct | Only A correct | Only B correct | Neither correct | Parsed disagreement | Same wrong / emitted wrong | Matching invalid literals | Abstain /63 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| SS + TS | 51 | 1 | 1 | 10 | 2 | 4 | 6 | 8 |
| SS + TB | 25 | 27 | 2 | 9 | 12 | 0 | 0 | 38 |
| SS + TF | 31 | 21 | 3 | 8 | 11 | 0 | 3 | 32 |
| TB + TF | 26 | 1 | 8 | 28 | 6 | 3 | 2 | 34 |
| SS + Q2 | 52 | 0 | 7 | 4 | 4 | 0 | 0 | 11 |
| TF + Q2 | 34 | 0 | 25 | 4 | 11 | 0 | 0 | 29 |
| SS + M3 | 45 | 7 | 6 | 5 | 4 | 0 | 0 | 18 |
| Q2 + M3 | 51 | 8 | 0 | 4 | 0 | 0 | 0 | 12 |

For SS + Q2, all 11 wrong SS outputs are withheld, but six were already parser-invalid. The **incremental** benefit is withholding all five parsed wrong SS prices, with no additional flags among its 52 correct fields. SS + TF also withholds those five parsed errors, while flagging 21 correct SS fields. SS + M3 flags seven correct SS fields. Q2 + M3 withholds four already-invalid Q2 fields and eight correct Q2 fields; it adds no emitted-error detection over parser-only Q2 here. These policies never replace a disagreeing value, so they introduce no false correction; false review flags and retained wrong agreement remain visible.

The complete 55-pair matrix gives **emitted correct / emitted wrong / abstained**, always over 63 fields. Seven pairs emit a common wrong value; the other 48 have zero observed wrong emissions at varying coverage. No pair is selected by maximising this table.

| Reader | TT | SS | MM | TS | TM | ST | SM | MT | MS | TB | TF |
|---|---|---|---|---|---|---|---|---|---|---|---|
| TT | — | 44/0/19 | 38/0/25 | 44/0/19 | 39/0/24 | 43/0/20 | 41/0/22 | 41/0/22 | 44/0/19 | 26/0/37 | 33/0/30 |
| SS | — | — | 43/0/20 | 51/4/8 | 45/0/18 | 43/0/20 | 46/0/17 | 41/0/22 | 51/4/8 | 25/0/38 | 31/0/32 |
| MM | — | — | — | 43/0/20 | 44/1/18 | 39/0/24 | 44/1/18 | 37/0/26 | 43/0/20 | 25/0/38 | 30/0/33 |
| TS | — | — | — | — | 45/0/18 | 43/0/20 | 46/0/17 | 41/0/22 | 52/5/6 | 25/0/38 | 31/0/32 |
| TM | — | — | — | — | — | 40/0/23 | 45/1/17 | 38/0/25 | 45/0/18 | 25/0/38 | 31/0/32 |
| ST | — | — | — | — | — | — | 42/0/21 | 43/0/20 | 43/0/20 | 26/0/37 | 32/0/31 |
| SM | — | — | — | — | — | — | — | 39/0/24 | 46/0/17 | 25/0/38 | 31/0/32 |
| MT | — | — | — | — | — | — | — | — | 41/0/22 | 25/0/38 | 31/0/32 |
| MS | — | — | — | — | — | — | — | — | — | 25/0/38 | 31/0/32 |
| TB | — | — | — | — | — | — | — | — | — | — | 26/3/34 |
| TF | — | — | — | — | — | — | — | — | — | — | — |

## Triples, ablation and escalation control

All 165 OCR triples were frozen before scoring and use unanimity only. Correct emissions range from 24 to 51 of 63. Of these triples, 163 have no observed wrong emission; SS + TS + MS emits **51 correct, four wrong and eight abstentions**, and MM + TM + SM emits **44 correct, one wrong and 18 abstentions**. Both failure groups reuse one recogniser weight set across detector choices. Their full per-field/per-capture counters remain in the hashed result packet. Multiple comparison on five inspected crops supplies no validation of the apparently clean groups.

| Fixed control / ablation | Emitted correct | Emitted wrong | Abstained /63 |
|---|---:|---:|---:|
| SS supported-value baseline | 52 | 5 | 6 |
| SS + TF, remove Q2 | 31 | 0 | 32 |
| SS + Q2, remove TF | 52 | 0 | 11 |
| TF + Q2, remove SS | 34 | 0 | 29 |
| SS + TF + Q2, always inspect all three | 31 | 0 | 32 |
| Availability-triggered control | 55 | 5 | 3 |

The predeclared availability control keeps any supported SS value; only for its six unavailable/invalid fields does it require TF/Q2 agreement. It makes 63 baseline and 12 auxiliary **field lookups**, versus 189 for always inspecting all three. It recovers three correct quantities but leaves five parsed wrong SS prices unexamined. These are data-replay lookup counts, not measured model requests, latency, CPU savings or a scheduling implementation: inference originally ran on whole crops. No confidence scores or justified production escalation thresholds exist here.

| Capture | Fields | SS correct | Q2 correct | SS + Q2 emitted correct / wrong / abstain | SS + TF + Q2 emitted correct / wrong / abstain |
|---|---:|---:|---:|---:|---:|
| `levski-01/v3-sell-5.jpg` | 9 | 6 | 6 | 6/0/3 | 0/0/9 |
| `patch-city-01/buy-1.jpg` | 12 | 9 | 11 | 9/0/3 | 6/0/6 |
| `pyro-gateway-stanton-01/buy-1.jpg` | 12 | 12 | 12 | 12/0/0 | 9/0/3 |
| `ruin-station-01/sell-1.jpg` | 9 | 4 | 9 | 4/0/5 | 3/0/6 |
| `seraphim-station-01/unselected-buy-1.jpg` | 21 | 21 | 21 | 21/0/0 | 13/0/8 |

These are five captures from five terminal entries, not five proven independent sessions. Benefits and abstentions cluster by capture: SS/Q2 emits no Levski stock, and TF makes the triple abstain on every Levski field. Treating all 63 fields as independent trials would conceal that structure.

## Larger and original-prompt VLM conditions

| Frozen condition | Reader | Assigned field coverage | Correct | Wrong (invalid subset) | Missing | Alignment-unscorable | Total |
|---|---|---:|---:|---:|---:|---:|---:|
| Original MAIN | Qwen3-VL 8B | 0/1680 | 0 | 0 (0) | 0 | 1680 | 1680 |
| Original MAIN | Qwen3.5 9B | 45/1680 | 37 | 8 (8) | 0 | 1635 | 1680 |
| Original POSTHOC-NAME | 8B | 1659/1680 | 1126 | 493 (492) | 40 | 21 | 1680 |
| Original POSTHOC-NAME | 9B | 1671/1680 | 1105 | 463 (459) | 103 | 9 | 1680 |
| Original-prompt 2B MAIN, each CPU/GPU | 2B | 0/63 | 0 | 0 (0) | 0 | 63 | 63 |
| Original-prompt 2B POSTHOC-NAME, each CPU/GPU | 2B | 63/63 | 53 | 10 (10) | 0 | 0 | 63 |

Assigned coverage includes wrong/invalid/missing values on an aligned card; it does not mean valid numeric output. The 9B main assignment covers only 15 of 560 complete reference cards. Its eight invalid fields are four unsupported-unit prices and four quantities. Paired main coverage is zero, so paired correctness and accepted accuracy rates remain undefined. Original 2B main likewise has no eligible complete cards.

The separately labelled larger-reader name diagnostic emits 867 correct, zero wrong and 813 abstentions over all 1680 fields. Its both-correct/only-8B/only-9B/neither counts are 867/259/238/316; five supported values disagree, 244 invalid literals match, 135 fields involve an omission and 30 involve an assignment failure. Those latter categories can overlap. The original CPU/GPU 2B diagnostics have identical literals, emit the same 53 correct values and share ten invalid literals. They add no independent witness. Neither diagnostic repairs the main result or becomes held-out evidence.

## Concrete public-corpus discrepancies

These selected illustrations come from the complete measured inventories, not a scoring subset. Links identify unchanged current Golden entries. Prices are aUEC/SCU; stock is SCU. Parser outcome, field role and exact literal stay distinct.

| Capture / commodity / field | Current Golden | Retained prediction | Consequence |
|---|---:|---|---|
| [Patch City](../corpus/public/patch-city-01/expected.json), `buy-1.jpg`, Omnapoxy price | 3237 | SS, TS and MS: `83.237/SCU` | All parse as 83237; their unanimous triple emits the same wrong price. |
| Same capture, Human Food Bars price | 416 | SS: `8416/SCU`; Q2: `41.6/SCU`; M3: `4.116/SCU` | SS/M3 parse wrong values; Q2 is invalid under the unchanged grouping grammar. No reference-based decimal repair. |
| [Levski](../corpus/public/levski-01/expected.json), `v3-sell-5.jpg`, BEXALITE stock | 0 | All nine PP profiles and TF: `OSCU`; TB: `0SCu` | Ten matching letter-O readings remain invalid; best's zero is a distinct correct observation. |
| Same capture, BERADOM stock | 0 | SS: `OSCU`; Q2: `120,000/SCU`; M3: JSON null | Invalid letter/digit and field-role readings differ from an explicit omission; agreement cannot supply zero. |
| [Patch City](../corpus/public/patch-city-01/expected.json), `buy-1.jpg`, DynaFlex stock | 45 | SS: `45 SCU`; TB/TF: empty associated text | Both omissions stay in their 63-field denominators. |
| [Seraphim](../corpus/public/seraphim-station-01/expected.json), `unselected-buy-1.jpg`, FLUORINE price | 1090 | SS: `1,090/SCU`; MM: `81,090/SCU`; TF: `41,090/SCU` | Parsed prices 1090, 81090 and 41090 show distinct false leading digits. |

Numeric/status agreement says nothing about the shared wrong Levski BUY tab in Q2/M3, clipped complete-card labels, spurious commodity objects or free-form container fields in the original records. Names/IDs, terminal, side, section, container sets, locate/layout, completeness, stitching, confirmed fields, confidence and sendability are outside this three-field replay.

## Uncertainty, readiness and delivery limits

No calibration or untouched holdout is created. The OCR/small-VLM comparison has five previously inspected captures; neighbouring scroll captures and repeated cards in the 155-capture campaign are correlated, and independent session membership is not established. Per-capture results replace an unjustified narrow field-level interval. Assuming independent accepted fields would give optimistic one-sided 95% zero-error upper bounds of **5.60% for 0/52** and **9.21% for 0/31**, using `1 - 0.05^(1/n)`. Even treating the five captures as independent gives a 45.07% bound for a capture-error event; that assumption is also unproved. These are illustrations, not valid production confidence bounds. All-abstain never wins through a zero-error percentage.

The unchanged `inspect` command separately reports seven entries and 41 metadata classes: 15 `gated`, ten `insufficient` and sixteen `missing`, with recognition explicitly unevaluated. **Metadata readiness is not measured accuracy.** Missing references, unknown conditions/IDs, null text-height bands, fidelity restrictions and unsupported classes remain unchanged. New captures gain no recognition coverage from verification alone.

Nine PP profiles contain only three recogniser weight sets. Tesseract best/fast share an architecture and training lineage; their byte-identical 4-/8-GiB replicas are not additional votes. CPU/GPU copies and backend/quantisation changes do not establish independent models. Distinct OCR/VLM families can still share errors or field-role confusion.

This replay adds no hardware or lifecycle measurement. The earlier RTX 5090/32-GiB-VRAM/approximately 96-GiB-RAM host is strong hardware; two-CPU and 4-/8-GiB VM conditions are stress limits on it. Physical weak-PC behaviour, peak process-tree RAM/VRAM, p95, cancellation/unload, game coexistence and clean offline installation remain unproved. [ADR-0012](adr/0012-strictly-local-recognition-delivery.md) still requires local inference without fees or a separately installed programme. Historical Ollama was an instrument; the [Windows CRT prerequisite](adr/0009-visual-cpp-runtime-as-prerequisite.md) remains an unresolved delivery blocker. This record selects no runtime/model/profile and changes no product acceptance or approval.

## Replay, private evidence and validation

Raw responses, captures, request metadata and model files are not published here. The prepared packets retain associated field literals privately. Only the known public-corpus examples above are reproduced. Exact retained roots are:

- Current protocols, preparation, tests, logs and results: `D:/Coding/Agents/Temp/codex/tallyline-reader-verified-reassessment-20261104`.
- Original joint/PP/Tesseract evidence: `D:/Coding/codex-worktrees/tallyline-native-guard/UEX-Datarunner-Client/target`; `local-ocr-joint-reader-comparison-20261102/literal-agreement-v2.json`, SHA-256 `bda31d319cac456070216ae7407cd8c2ec2f4400a6fa3df82a1c010301a65341`, retains exact upstream pointers.
- Original VLM root: `D:/Coding/claude-worktrees/tallyline-implementation/local-model-benchmark`; `offline-analysis/`, `qwen3vl-2b-source-bound-plan/offline-analysis-smoke-v2/`, `development-transcription-v2/offline-analysis-v3/` and `ministral3-3b-source-review/offline-analysis-smoke-v1/` retain raw/source manifests and original ledgers.

| Frozen artefact | SHA-256 |
|---|---|
| `protocol-freeze.json`, before metric implementation/scoring | `73de64578722d007d6652ebf2c28d460c188a40c509524dece2c7e509ecad037` |
| `field-identities-freeze.json` | `41657c15cda2801278ef692b80a8e96c06202774625354c1002ee21113b2404c` |
| `v3/retained-inputs-freeze.json`, 628 rehashed retained files | `fd2edc2bc4a99f9742a3967ea9b5764c71aa6febca7c618b90fbc575682fa3f8` |
| `v3/prepared-packets-freeze.json`, before any score | `016fdd1f2bc83a120cbdf34305e2b51c3184b752f776fdd6512629c7838172fb` |
| `v3/final-result-identities.json`, eight byte-identical replays | `98d38989f85426b2df887588272336104e2bd5d400e322c4c1353cfc568d0e94` |

| Packet in `v3/` | Input SHA-256 | Result SHA-256 in `v3/final-results/` |
|---|---|---|
| `cross-ss-ministral.json` | `7702b6f1add9c72921f9ce67580708bc38f1d922c58c4f8697b48816cb1897ff` | `d6eab6349c3f946a7b9f844b84581333660273e18f23c526efecb99e8c681623` |
| `cross-ss-tf-qwen.json` | `714955fc5a3abef510c3bb152dbb11cfade7c07249097dd024216f04bf06d1f2` | `70a70f1771445a3307d8cc96fa9df706951811e20a987c5ab85b557192af91fa` |
| `ocr-eleven.json` | `53d3801b720d17a23c4e6bdc000bfc7d33e0fdfb24fcc54117af7c2080d6399a` | `aa2a2ebc5aeca14929920b7f6c3e67de63866abc9aa562004f5cfbcb40e502ca` |
| `vlm-corrected-small.json` | `c657851364aa391bbc696a1e259b1f53ccd46d5ad40c5284d31353b1eb704923` | `bb1b957b73dc9504277cd10b19747c50dcfdd2d0f5a6bffc8caef3f317ca0837` |
| `vlm-large-main.json` | `c7897b9c51e04e0d1de982a91e26098abb55e2830c06366ab00ee23e1f9a1fcd` | `396e7b9748de60ce939c570be31d5126bdece5dc5f6e70c75e1fcd3c9ef2f9a3` |
| `vlm-large-posthoc-name.json` | `8184eae382e08d440d1bb4e14521b44a7b961bc294def0de02cca12fab6294a9` | `3f7739325bf0bded526d3c0e910ff1b0fdfb9e28c0ffd34db0f09eda6b3a7454` |
| `vlm-original2b-main.json` | `c8c49f2b755e726760b37ac8c6021dd39a18585d6383af1ce2874eded891439e` | `773c5be6bb69f65b8f4145432c81b509c20449bcb1ce88cbc3cc4b86bd3ff69f` |
| `vlm-original2b-posthoc-name.json` | `3c0d3edb8e07749f2060f489acdca66affe0f6edf4ad5955c1fdedb78fcfbac2` | `896a1e78eae45c02c8f01b0cf1383e8229224353385e5c80f36a0ea096e53e86` |

Availability checked after scoring: the two historical worktree roots above are unavailable at their original paths. Of 628 files in the VLM retained-input freeze, 627 original input paths are absent; only the current adapter script remains at its recorded path. They were successfully rehashed before scoring. The owner confirmed that these legacy worktree roots were deleted because they were outside the required Agents directory; no relocation or archive is expected. The eight prepared packets, their raw associated field literals, current Golden files, input/hash freezes and both byte-identical result sets remain available under the current task root. Replaying those packets is reproducible; complete original-response re-audit or raw re-extraction requires recovery of the hash-bound originals. The dated path-by-path check is retained as `retained-input-final-availability.json`.

The public [offline CLI](../tools/ocr-eval/README.md#retained-reader-replay) measures a prepared packet. Reproducing extraction also requires the private evidence and `prepare-replay-v3.ps1`, whose identity is in the input freeze. Preparation uses exclusive creates; earlier failed/incomplete scratch attempts remain separately retained and were not scored. No earlier artefact is overwritten.

Execution used PowerShell 7, Rust 1.99.0, two Cargo jobs, an exclusive external Cargo home/build root, explicit external TEMP/TMP, cleared wrappers/flags/aliases and an unset private-corpus variable. HTTP/HTTPS/all proxies point to checked-closed `127.0.0.1:9`; Cargo work uses `--locked --offline`. Replay the already frozen packets into a fresh permitted result directory:

```powershell
foreach ($setting in @(Get-ChildItem Env: | Where-Object Name -Match '^(RUSTC_WRAPPER|RUSTC_WORKSPACE_WRAPPER|RUSTFLAGS|RUSTDOCFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_TARGET|CARGO_BUILD_TARGET_DIR|CARGO_BUILD_BUILD_DIR|CARGO_NET_OFFLINE|CARGO_ALIAS_.+|CARGO_TARGET_.+)$')) {
    Remove-Item -LiteralPath "Env:$($setting.Name)"
}
$env:CARGO_HOME = 'D:/Coding/Agents/Temp/codex/tallyline-merge-external-paths/cargo-home'
$env:CARGO_TARGET_DIR = 'D:/Coding/Agents/Temp/codex/tallyline-merge-external-paths/build'
$env:CARGO_BUILD_JOBS = '2'
$env:RUSTUP_TOOLCHAIN = '1.99.0'
$env:TEMP = 'D:/Coding/Agents/Temp/codex/rv4/tmp'
$env:TMP = $env:TEMP
$env:HTTP_PROXY = 'http://127.0.0.1:9'
$env:HTTPS_PROXY = $env:HTTP_PROXY
$env:ALL_PROXY = $env:HTTP_PROXY
$env:NO_PROXY = 'localhost,127.0.0.1,::1'
Remove-Item -LiteralPath Env:TALLYLINE_CORPUS_DIR -ErrorAction SilentlyContinue
$evidenceRoot = 'D:/Coding/Agents/Temp/codex/tallyline-reader-verified-reassessment-20261104'
$packetRoot = Join-Path $evidenceRoot 'v3'
$freeze = Get-Content -Raw -LiteralPath (Join-Path $packetRoot 'prepared-packets-freeze.json') | ConvertFrom-Json
$resultRoot = Join-Path $evidenceRoot 'replay-new'
New-Item -ItemType Directory -Path $resultRoot -ErrorAction Stop | Out-Null
foreach ($packet in $freeze.packets) {
    $name = [IO.Path]::GetFileNameWithoutExtension($packet.path)
    cargo run --quiet -p ocr-eval --locked --offline -- reader-replay --public corpus/public --packet $packet.path --sha256 $packet.sha256 1> (Join-Path $resultRoot "$name-result.json")
    if ($LASTEXITCODE -ne 0) { throw 'Replay failed' }
}
```

Inspection and all eight initial/final replays exited 0. Results were byte-identical after structural module separation; no score, parser, label or normalisation changed after viewing results. At the initial checkpoint, package tests (`cargo test -p ocr-eval --locked --offline`) passed **35 tests**, including two public CLI contracts for known literal wrong/invalid/omitted/alignment cases, exact decimals beyond float precision, shared wrong agreement and hash/slot/eligibility refusals. The first CLI test failed with exit 101 before implementation.

After the typed replay-error and type-import fixes, final package tests pass **36 tests**, including the public typed-failure contract for unavailable inputs, malformed packets and unsupported schemas. Final `cargo fmt -p ocr-eval -- --check` and `cargo clippy -p ocr-eval --all-targets --locked --offline -- -D warnings` pass, and all eight replays remain byte-identical to the frozen results. No canonical workspace, mutation, native or fresh inference acceptance is claimed; integration checks remain separate.
