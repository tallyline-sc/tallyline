# Ministral 3 3B development experiment: literal fields and shared errors

> **Doc type:** Historical record — completed development experiment; no product backend selected. Last reviewed: 2026-11-02.

Corrected 2026-11-04: verification states, counts and hashes below describe the recorded run, whose references were unverified at that time. The owner has since marked all seven existing public entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Historical artefacts and expected-file hashes are unchanged and do not bind the current annotations; this decision supplies no retrospective accuracy measurement.

## Scope and source identity

Five GPU-only requests used the author's public, ungated [Ministral-3-3B-Instruct-2512-GGUF](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512-GGUF/tree/eb599d408350ea2bb60452cb86be7c7b2fc28227), revision `eb599d408350ea2bb60452cb86be7c7b2fc28227` dated 2026-01-15, under Apache-2.0. The [author API at that revision](https://huggingface.co/api/models/mistralai/Ministral-3-3B-Instruct-2512-GGUF/revision/eb599d408350ea2bb60452cb86be7c7b2fc28227?blobs=true) binds the two published files below. Both downloaded files and their private imported copies were independently rehashed against these sizes and digests; no conversion was performed or claimed reproducible.

| Author file | Bytes | SHA-256 |
|---|---:|---|
| `Ministral-3-3B-Instruct-2512-Q4_K_M.gguf` | 2,147,023,008 | `9ed150d4367e68df0ac8e1540f6ddc65b42d0ee26378329d1ecbca60f93fc5f8` |
| `Ministral-3-3B-Instruct-2512-BF16-mmproj.gguf` | 841,501,856 | `70ca26e41ecea9da997e2887462ad68b848fbf64da011ff4107bbe7353519ee0` |

A separate zero-image import was followed by a binding review of the actual private manifest/configuration, vision capability and embedded GGUF chat template before execution. The binding check accepted the real show response and refused 12 negative fixtures, including changed runner/family/quantisation/architectures, missing vision, renderer/parser overrides, changed template source/bytes, remote host and multiple manifests. The zero-image check itself did not prove tensor compatibility or authorise an image request. The subsequent five completed responses provide the execution evidence.

The unchanged existing Ollama 0.40.13 installation, source VCS revision `04088e7c9d41cba5d33bafbf788d87a465c79d5b`, was a research instrument. This experiment selects no dependency, engine or user prerequisite and grants no redistribution approval. It used a private profile/store, owned literal IPv4 loopback, disabled proxy/redirect/cloud routes and no remote fallback, rather than an independently enforced network sandbox. No authentication, reference values, catalogue, previous answers, balance or player-inventory image entered a request. Tallyline's strictly local, complete initial-installation constraints remain those of [ADR-0012](adr/0012-strictly-local-recognition-delivery.md).

## Fixed development condition

The five already inspected captures are Levski `v3-sell-5.jpg`, Patch City `buy-1.jpg`, Pyro Gateway `buy-1.jpg`, Ruin Station `sell-1.jpg` and Seraphim `unselected-buy-1.jpg`. They contain 21 complete and four partial reference cards at revision `073ec825133faca4c62162588c012548adcf67ae`. Every entry retains `verified: false`. This is neither a held-out set nor a measurement of general digit accuracy.

Requests reused the frozen generic user prompt, response schema, shop-only crops and generation controls from the [new 2B literal-field condition](local-vision-small-model-campaign.md#new-literal-field-development-condition): temperature 0, seed 42, context 8192, output limit 3072, four generation threads, GPU-auto, non-streaming, `think: false` and `keep_alive: 0`. Each model retained its separately checked GGUF template; identical user prompt and controls do not mean identical rendered tokens. The 16 GiB available-RAM guard and bounded game/timeout/cancellation checks remained. No previous model answer was supplied to the reader.

All five responses satisfy the full unchanged schema and finish without truncation, returning 23 objects. The declared development diagnostic attaches only names unique across all reference and reader cards within the same capture, using NFC, whitespace and ASCII-case normalisation. It uses no section, row-order, field-value, marker-removal or fuzzy rescue. All 21 complete reference cards remain in each field denominator; partial cards, unknowns and unassigned objects remain separate. The unchanged conservative parser retains raw strings and refuses repairs of symbols, units, decimal points or missing values.

## Field results and concrete discrepancies

| Field on 21 complete reference cards | Reference-concordant | Remaining outcomes |
|---|---:|---|
| Price | 12 | One parsed mismatch, one inconsistent decimal form, seven malformed symbol-prefixed strings |
| Stock quantity | 18 | Three JSON `null` values against visible/reference 0 SCU |
| Status | 21 | None on this selected set |

The seven symbol-prefixed price strings contain the matching reference digits. They are **not seven demonstrated digit errors**. Their extra characters fail the frozen grammar and remain in the denominator; symbols are not stripped and the prices are not retrospectively rescored as concordant.

| Capture and material | Raw price | Reference price per SCU |
|---|---|---:|
| Levski, BERADOM | `₠ 120,000/SCU` | 120000 |
| Levski, BEXALITE | `₠ 27,000/SCU` | 27000 |
| Levski, BIOPLASTIC | `₠ 8,500/SCU` | 8500 |
| Pyro Gateway, HEPHAESTANITE | `≈4.167/SCU` | 4167 |
| Pyro Gateway, IRON | `≈2.850/SCU` | 2850 |
| Pyro Gateway, ALUMINUM | `≈3.128/SCU` | 3128 |
| Pyro Gateway, ARGON | `≈316/SCU` | 316 |

Two further Patch City prices need separate treatment. Human Food Bars is read as `4.116/SCU` against 416: the fixed grouping grammar parses it as 4116, which is a mismatch; this does not establish that the model intended 4116. Fluorine is read as `9.87/SCU` against 987 and is refused as an inconsistent decimal form. For Levski BERADOM, BEXALITE and BIOPLASTIC, stock is JSON `null` against 0 SCU. Unknown is retained as unknown, never converted to zero. A static visual spot check confirms these particular Patch City prices and Levski stocks without verifying whole reference entries.

## Paired development diagnostic and geometry

The paired comparison uses the same 21 complete reference cards and the exact frozen parser; it measures parser/reference concordance, not a ranking of digit accuracy or independent correction.

| Field | Both concordant | Only 2B concordant | Only Ministral concordant | Neither concordant |
|---|---:|---:|---:|---:|
| Price | 12 | 8 | 0 | 1 |
| Stock quantity | 18 | 0 | 0 | 3 |
| Status | 21 | 0 | 0 | 0 |

Both readers miss Human Food Bars: the 2B raw price is `41.6/SCU`, while Ministral returns `4.116/SCU`. For the three Levski stocks the 2B reader supplies price/rate strings and Ministral supplies unknowns. These are different failure forms, not three corrected quantities.

Ministral labels all 21 complete reference cards `clipped_at: none`; the 2B condition wrongly labels seven of them bottom-clipped. Ministral also avoids the 2B condition's spurious OUT OF STOCK commodity object at Patch City. The actual bottom-clipped Levski BORASE is retained with `bottom`, price `₠ 24,000/SCU` against 24000 and stock JSON `null` against 0; it stays outside the complete-card denominator. The unnamed upper Levski and lower Pyro fragments are omitted. Ruin Station's partial `[!] Altruciatoxin` loses its warning marker, becomes `Altruciatoxin` with `none` and remains unassigned under the strict name policy. Free container fields include material/section text such as `BORASE` and `SELLABLE CARGO`; no enabled-size set is inferred from them. All 23 returned objects and four partial reference cards remain in the private ledgers.

Both model families incorrectly read Levski's active tab as BUY, although the visible selected tab is LOCAL MARKET VALUE. A static crop check confirms this shared error. Agreement between these readers therefore cannot certify the value. Production layout remains responsible for geometry; no fusion, confidence or automatic-submission acceptance follows from this experiment.

## Execution, cleanup and limits

Request times were 54.85, 5.64, 6.16, 5.25 and 7.14 seconds. The first outlier's cause is unestablished. The owned backend log records five CUDA vision backends, 27/27 language layers offloaded per request and four generation threads. Concurrent builds and uncontrolled caches prevent production-latency or cold-start conclusions. Peak RAM/VRAM, mapped GPU driver identity, weak-PC behaviour, Linux inference and a packaged application were not measured. The strong Windows host is described in the [first campaign](local-vision-first-campaign.md#bound-inputs-and-method); these results do not simulate weaker hardware.

Independent post-run checks established the owned process absent and all six recorded API/backend ports closed. All 982 instrument files were rehashed unchanged. No app window, computer-control session, credential access or foreign-process modification was involved. Native product OCR and additional OCR readers remain separate evaluation work.

## Evidence identities

The local analysis manifest binds the input/reference/response files, complete field/card/partial ledgers, paired comparison and separately dated source, execution and cleanup evidence. All 28 files named by its derived/source-run lists and all 25 input-manifest entries were independently checked against their recorded size and SHA-256 before this record was written. Historical pre-execution statuses and raw run fields were preserved; later verification records supply backend placement and cleanup, rather than silently rewriting the raw results.

| Evidence | SHA-256 |
|---|---|
| Frozen user prompt, shared with the new 2B condition | `d1a43be5596ee6de697111dde20ed7dfda7d4c1b4e5107b0418b9f48b789384a` |
| Frozen response schema | `d3182eee405aaad2459b0796cf23432a5ac52849b6868d1af4474582056ecfa2` |
| Fixed condition profile | `809169f387c7f37b1c4705478ead36acd0c013751248e1565e01eadfee5209f0` |
| Zero-image root review | `fd4229145121d358645d2523482e30325fe7ee194d20c2d7f7d4c0dc62a3b7eb` |
| Actual binding and 12 negative-fixture review | `0b33288792baa9960ba030213ab6b40e823c1bfa3ff7a805308f58fe5925b5da` |
| Five-cell result file | `34c691b09e5650f0673f9c9a6e43dda2d05d702e6d859decece7b0663cf7b661` |
| Five-cell completion | `dfc44dc79ab18371e3bafdcbe7c0d8ba597fbbfe87fcae6cfba5d3a506c92b3a` |
| Independent process/port completion check | `d5140a1e1797b93d1302f9b8b7cae40201fdd88a4c8bc6893bca1c50a5d26e5b` |
| Independent backend placement review | `6ed800b260c7d0ad97a31e0586499e3ec7e3ab66a3f9ba4fe2bf624e445537b9` |
| Independent unchanged-instrument rehash | `86ae4a2b51cc90b57cc3e2322c07218d11b4a9fa98d56905a95d4eb6d45f1746` |
| Offline analysis manifest | `1a4e2ecf8664bb8d3b73832eda0a3e79e2005fe0be437e37812d8ebcbfc6e3a6` |

The private human-readable report retains literal readings, reference values and review classifications. Private paths, ports, process identifiers, raw responses, screenshots and model weights are omitted from this repository checkpoint. No reference annotation or earlier result was changed. Broader held-out, complementary-OCR, resource and offline-delivery work remains described in the [runtime research](local-vision-runtime-research.md).
