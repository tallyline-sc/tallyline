# First local VLM campaign: coverage before confidence

> **Doc type:** Historical record — completed exploratory experiment; no product backend selected. Last reviewed: 2026-11-02.

Corrected 2026-11-04: verification states, counts and hashes below describe the recorded run, whose references were unverified at that time. The owner has since marked all seven existing public entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Historical artefacts and expected-file hashes are unchanged and do not bind the current annotations; this decision supplies no retrospective accuracy measurement.

## Result

The two existing local readers returned 310 parseable, schema-valid responses across the same 155 shop-only crops. Their structural output is not suitable for estimating complete-pipeline accuracy. Under the comparison policy frozen before scoring, Qwen3-VL 8B supplies no complete candidates and Qwen3.5 9B supplies only 15 assignable complete reference cards out of 560. There are no jointly eligible cards, so the paired main comparison rate is **undefined**, not a zero-error result.

The reference annotations are also unverified: all five entries retain `verified: false`. The owner's nine confirmed bottom-clipped cards establish those geometry corrections, not verification of every price, quantity or status. This record reports reference concordance and diagnostic differences, never validated accuracy or submission safety.

## Bound inputs and method

The reference revision is `073ec825133faca4c62162588c012548adcf67ae`: 155 captures, 769 annotated cards, comprising 560 complete and 209 partial cards. Each reader received a fresh single-message request containing the same audited shop-only crop and generic literal-transcription prompt. No reference values, commodity catalogue, previous answer, balance, terminal header or player-inventory screenshot entered the request.

The existing Ollama installation was an experimental instrument, not a product prerequisite or dependency adoption. Its reported version was 0.40.13 and source VCS revision `04088e7c9d41cba5d33bafbf788d87a465c79d5b`. Its release is inside the restored seven-day cooldown; this experiment grants no production exception. A private model store, profile and temporary directory kept the user's configuration and credentials outside the experiment. Only the owned literal IPv4 loopback endpoint was used, with proxy/redirect/cloud routes disabled and no remote fallback. This is not a claim of an independently enforced network sandbox.

| Reader | Exact private model manifest SHA-256 |
|---|---|
| `qwen3-vl:8b-instruct` | `367f8577c5bed4ca5fd016207ce7b069010734f599fd717d8833444373a9a318` |
| `qwen3.5:9b` | `85879493124988ce76ff36a6ac583504cc8d893727db6a0c4f9992a079e6ca49` |

Manifest identity is not proof of an author-reproducible GGUF conversion. No new model or native engine is approved for redistribution by these measurements.

Settings were fixed: temperature 0, seed 42, context 8192, output limit 3072, `think: false`, non-streaming responses and `keep_alive: 0`. The readers ran sequentially; bounded checks observed no resident model between cells. Empty `/api/ps` is not independent proof of VRAM release. The strong Windows x64 host had an RTX 5090 and roughly 96 GiB system RAM. Concurrent builds, operating-system caches and driver caches prevent production latency or cold-start claims; no weak-PC or Linux inference was measured.

Names used NFC, whitespace normalisation and explicitly limited ASCII case folding. Main alignment required a unique name plus an explicit recognised section within the same capture, a reference-complete card and a model-complete candidate. No fuzzy, row-order or price-assisted assignment was allowed. The conservative numeric parser retained unknown, unsupported-unit, malformed and overflow outcomes, with exact integer/rational arithmetic. Containers remained free-form text and were not scored as enabled-size sets. Active `LOCAL MARKET VALUE` labels remained unknown to the frozen BUY/SELL side parser rather than being silently relabelled.

## Frozen main results

| Measure | Qwen3-VL 8B | Qwen3.5 9B |
|---|---:|---:|
| Responses / schema failures | 155 / 0 | 155 / 0 |
| Returned cards | 667 | 672 |
| Cards declared complete by the reader | 0 | 662 |
| Assignable complete reference cards | 0 / 560 | 15 / 560 |

All 667 cards from the 8B reader were labelled as top or bottom edges, including visibly complete cards. The 9B reader supplied no recognised explicit section for 603 of its 672 cards: 445 are missing or empty and the remainder use labels outside the frozen grammar. Its 15 aligned cards contain 11 parseable concordant prices and quantities and four unsupported-unit outcomes; their statuses are concordant. These are structural failures of this prompt/reader condition, not evidence that every unassigned digit is wrong.

## Separately dated exploratory name diagnostic

After the main policy had been frozen and the campaign had finished, a separate name-only diagnostic attached only exact normalised names that were unique across all reference and reader cards in the same image. It retained reference-complete cards for numeric comparison, while leaving unknown sections and reader completeness visible. It did not repair either output and is not the main result, a held-out validation or a fusion rule.

| Field on 560 reference-complete cards | 8B concordant | 9B concordant | 9B different | Other outcomes |
|---|---:|---:|---:|---|
| Price | 229 | 306 | 0 | 8B: 324 unknown, malformed or unsupported, 7 unassigned; 9B: 251 unknown, malformed or unsupported, 3 unassigned |
| Quantity | 380 | 309 | 1 | 8B: 173 unknown, malformed or unsupported, 7 unassigned; 9B: 247 unknown, malformed or unsupported, 3 unassigned |
| Status | 517 | 490 | 3 literal strings | 8B: 1 different, 35 unknown, 7 unassigned; 9B: 64 unknown, 3 unassigned |

There were 550 jointly name-assigned cards. Prices were parseable and equal in 173 pairs; 377 pairs had one or both values unparseable. Quantities agreed in 243 pairs and disagreed in one, with 306 unparseable pairs. Ten further cards had only one assigned reader. Four literal status disagreements include three strings containing the word `null`, which must not be mistaken for three confirmed status-reading errors. No paired accuracy percentage follows from these selected parseable subsets.

Concrete review examples:

- Patch City `sell-3.jpg`, Pressurized Ice: the visible quantity and reference are 456 SCU; 8B reads `456 SCU`, whereas 9B reads `0 SCU`. A subsequent visual spot check confirms this particular 9B error without promoting the whole reference entry to verified.
- Levski `v1-buy-5.jpg`, Quantum Fuel: reference and 9B status are `MAX INVENTORY`; 8B returns `OUT OF STOCK`. This is a status discrepancy for review, distinct from the three literal `null` outputs.
- Levski `v1-sell-10.jpg`, Helium: both readers put `0 SCU` into the price field, while the reference price is 1100. This is shared field-role confusion; it is not silently converted into a price or a whole-pipeline joint-error statistic.

The local human-readable report preserves 813 flagged review rows, raw strings, unknowns and crop links. That count does not mean 813 wrong values or 813 wrong annotations. Private paths and response bodies are deliberately absent from this repository record.

## Completion, interruptions and evidence identities

The 310 unique cells comprise ten smoke cells and 300 further cells. One earlier child-owned process interruption lost an in-flight response; one infrastructure retry was declared. Later resumptions stopped before sending the next image when a memory probe failed or available RAM fell below the unchanged 16 GiB guard. Saved cells were carried forward byte-identically; no answer-quality retry, prompt change or reference-based correction was made. The completed campaign ended at 2026-11-01T22:37:27Z. An independent check confirmed the owned process absent and its loopback port closed. All 982 installed runtime files were rehashed afterwards: 3,242,611,913 bytes, zero changes. This does not establish the identity of every mapped GPU driver.

| Evidence | SHA-256 |
|---|---|
| Frozen crop/case manifest | `a0e87933445f39b6d75d33318c2721ba8c14590f13ff39a80ed04da1b0f486d7` |
| Frozen prompt | `e55e2384d56fdd8755a7806d23d2d0162e0a7ef8f3b6457a7e6c289256da8fa3` |
| Frozen response schema | `b3378a71d8d6f71c2a68cd73c0abcd91e482dadac3ad98ee08dbefa5369d03e5` |
| Main comparison protocol frozen before scoring | `9919e3a170e3c42e5c713eae53fe12b7f3a9cf4e58dad0542b2b2fb5521e356c` |
| Experimental parser policy frozen before scoring | `6897812e6f62707487b4f2ee164dd2febb1ab6eb2eba96d71558ab43cb17775d` |
| Completed 300-cell full-pass result file, excluding the ten smoke cells | `b64597bd6b118a493d76a9d467ec8c4e0b4b9c3a1849b7405cf48b71c1e505fa` |
| Raw-response manifest covering all 310 cells | `876638a9d0d3c4b1d5ccf341ab9e61e351c2bd5b490f4380f4a2c69cc7a3c089` |
| Offline analysis manifest, revision 2 | `383c14a075a1cc2117e04fa5a02a0bf0f10bb41bf33ddca4854d079c3c876299` |

## Consequences for the next experiment

Keep the raw first-pass results immutable. A revised prompt must have its own dated profile and separate development results, with explicit field roles, active tab, section and completeness handling; it cannot retroactively repair this condition. Layout remains the production authority for complete cards. Measure ordinary OCR independently, then test whether an additional reader reduces undetected errors while preserving coverage. CPU and GPU profiles of one model are resource comparisons, not independent votes.

The next prepared experiment uses the author's fixed Qwen3-VL 2B GGUF and matching F16 projector, with a data-only local import check before five CPU and five GPU smoke cells. The first import-only attempt created the local model, then stopped before any image request because the controller expected a different show-response envelope and legacy manifest path. The owned process exited and its port closed; the failed instrumentation evidence is retained. At this record's cutoff, CPU/GPU smoke is unstarted. Preparation and import metadata do not establish backend placement, measured accuracy or a model recommendation. See [the runtime and OCR research](local-vision-runtime-research.md), [the OCR concept](plan/07-ocr-concept.md) and [ADR-0012](adr/0012-strictly-local-recognition-delivery.md) for the remaining product and delivery constraints.

Follow-up 2026-11-02: both original-prompt 2B smoke profiles and a separate corrected literal-field development condition have since completed. Their [own dated record](local-vision-small-model-campaign.md) preserves the separate inputs, structural failures, raw discrepancies and cleanup history; it does not alter this campaign's results or the earlier cutoff.
