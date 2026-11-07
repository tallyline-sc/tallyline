# Local 2B vision experiments: field roles and structural failures

> **Doc type:** Historical record — completed development experiments; no product backend selected. Last reviewed: 2026-11-02.

Corrected 2026-11-04: verification states, counts and hashes below describe the recorded run, whose references were unverified at that time. The owner has since marked all seven existing public entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Historical artefacts and expected-file hashes are unchanged and do not bind the current annotations; this decision supplies no retrospective accuracy measurement.

## Scope and identity

Two separate conditions used the author's Qwen3-VL-2B-Instruct Q4_K_M GGUF and matching F16 projector at revision [`52d6c8ffea26cc873ac5ad116f8631268d7eb503`](https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/tree/52d6c8ffea26cc873ac5ad116f8631268d7eb503), under Apache-2.0. The language file is 1,107,409,952 bytes, SHA-256 `089d75c52f4b7ffc56ba998ffc50aae89fcafc755f9e7208aacca281dca6c2ae`; the projector is 819,394,848 bytes, SHA-256 `c3d5afbef5287953acd57b4043d2269456e5761a4eaccb3b71b062996970aea5`. Source, private copies, manifest, configuration and the actual vision/template/runner binding were checked before execution. No model conversion was performed or claimed reproducible.

The five development captures are Levski `v3-sell-5.jpg`, Patch City `buy-1.jpg`, Pyro Gateway `buy-1.jpg`, Ruin Station `sell-1.jpg` and Seraphim `unselected-buy-1.jpg`. They contain 21 reference-complete and four partial cards at reference revision `073ec825133faca4c62162588c012548adcf67ae`. All entries retain `verified: false`. These already inspected captures are neither a held-out set nor weak-PC evidence.

The unchanged existing Ollama 0.40.13 installation was a research instrument, not a selected dependency or a user prerequisite. Execution used a private profile/store, owned literal IPv4 loopback, disabled proxy/redirect/cloud routes and no remote fallback. No catalogue, reference value, previous answer, balance or player-inventory image entered a request. The strong Windows host and concurrency limits are described in the [first campaign](local-vision-first-campaign.md); mapped GPU driver identity and child peak RAM were not established.

The first import-only instrumentation failure is retained: data import succeeded, then the controller stopped before images because it expected a flat show response and legacy manifest path. The corrected controller validated the actual single-manifest envelope and direct v2 manifest without reimporting. Import-verification and execution-controller identities remain distinct in later evidence.

## Original-prompt CPU and GPU smoke

Five CPU-t4 and five GPU-auto-t4 responses completed with the original first-campaign prompt/schema. All ten were parseable, fully schema-valid and untruncated. Logs establish CPU vision/no GPU offload for the CPU condition and CUDA vision/full 29-layer offload for the GPU condition. Four generation threads on this host do not simulate a small PC or cap whole-process RAM.

Each profile returned 24 objects, of which 23 were labelled top edges and one bottom; none was labelled complete. The frozen structural comparison therefore assigns **0/21** complete reference cards per profile, with no eligible paired main denominator. A separately labelled unique-name diagnostic assigns all 21 reference-complete cards: 14 prices, 18 quantities and 21 statuses agree with references per profile. The other six prices and three quantities have unsupported units; one price contains an inconsistent decimal separator. Unknowns remain explicit. All 63 numeric/status raw strings are identical between CPU and GPU; these executions are not independent readers.

CPU request times were 32.29–41.45 seconds. GPU times were 62.66 seconds for the first request and 4.52–6.35 seconds for the others; the first outlier's cause is unestablished. Concurrent builds and uncontrolled caches prevent production latency or cold-start conclusions. A parent-only memory sample is not model peak RAM.

## New literal-field development condition

The original prompt wrongly separated SHOP QUANTITY from the per-card SCU quantity, although that caption labels the same stock count. A new dated condition corrects those field roles, distinguishes the active tab from the panel title and asks for geometric clipping rather than list position. It does not modify earlier prompts, answers, annotations or parser rules.

The new prompt/schema/profile/controller were fixed before the five GPU-only requests. Generation controls stayed at temperature 0, seed 42, context 8192, output limit 3072, four threads, non-streaming, `think: false` and `keep_alive: 0`. The 16 GiB available-RAM guard and bounded game/timeout/cancellation checks remained. The controller checked the complete response schema; one valid and four invalid schema fixtures verified that boundary before inference.

The declared primary development diagnostic attaches only exact normalised names unique across all reference and reader cards in the same capture. Normalisation is limited to NFC, whitespace and ASCII case. It uses no section, row-order, field-value or fuzzy rescue. All 21 complete reference cards remain in every field denominator; partial cards, unknowns and unassigned extra objects are recorded separately. This diagnostic does not satisfy production layout, fusion or sending gates.

| Field on 21 complete reference cards | Original 2B GPU prompt | New literal-field prompt | Remaining new-condition outcomes |
|---|---:|---:|---|
| Price concordance | 14 | 20 | One inconsistent `41.6/SCU` against 416 |
| Stock concordance | 18 | 18 | Three price/rate strings in stock fields |
| Status concordance | 21 | 21 | None in this selected set |

All five responses satisfy the full schema and are untruncated. Six price fields become reference-concordant: BERADOM 120000, BEXALITE 27000, BIOPLASTIC 8500, Waste 450, Fluorine 1200 and Agricultural Supplies 1300. No previously concordant numeric/status field loses concordance. This is a comparison of two development prompts on the same inspected set, not a general accuracy improvement or independent-reader agreement.

Remaining review examples preserve the exact raw strings:

| Capture and material | Field | Model raw string | Reference |
|---|---|---|---|
| Levski `v3-sell-5.jpg`, BERADOM | Stock | `120,000/SCU` | 0 SCU |
| Same capture, BEXALITE | Stock | `27,000/SCU` | 0 SCU |
| Same capture, BIOPLASTIC | Stock | `8,500/SCU` | 0 SCU |
| Patch City `buy-1.jpg`, Human Food Bars | Price | `41.6/SCU` | 416 per SCU |
| Levski `v3-sell-5.jpg`, partial BORASE | Stock, excluded from complete-card denominator | `24,000/SCU` | 0 SCU |

A static visual spot check confirms these particular reference values. The fixed parser refuses the rate strings as quantities and the inconsistent decimal form; it never repairs a role, decimal point, unit or missing zero.

Structure still fails. Seven complete cards are labelled bottom-clipped: three in Levski and four in Pyro Gateway. Levski's actual bottom-clipped BORASE is retained, but its unnamed upper fragment is omitted. Patch City's OUT OF STOCK heading becomes a spurious fifth commodity object. Ruin Station's partial `[!] Altruciatoxin` becomes `[]! Altruciatoxin`, labelled complete and left unassigned by the strict name policy. Panel/tab titles are supplied as section headings; Levski's active tab is incorrectly read as BUY. All 24 returned objects and four partial reference cards remain in the evidence. No structural acceptance or automatic-send claim follows from the numeric table.

## Execution and cleanup

The new five requests took 4.86, 4.63, 4.64, 5.20 and 5.33 seconds. Logs show five CUDA vision backends and 29/29 language layers offloaded for each request. Actual peak RAM/VRAM, weak hardware, Linux inference and a packaged application were not measured.

The original owned process is absent and all six recorded API/backend ports were independently checked closed. The first backend cleanup check failed because Windows temporarily retained one listener entry with PID 0. No closed-port result was claimed then; a later connection was refused and the entry disappeared without killing a process. Both conditions preserve their original cleanup history. All 982 instrument files were rehashed unchanged after each run. No desktop window, computer-control session, credential access or foreign-process change was involved.

## Evidence identities and next work

| Evidence | SHA-256 |
|---|---|
| Original-prompt ten-cell result file | `fe28c28a6d425901129bd53e310b630617fbaf453ea221ec1a6175eb999eb66b` |
| Original-prompt analysis manifest | `71864b03e77c04ff4c268b21dad24286252294e6cd4a94895c6e64e66ac5b606` |
| New literal-field prompt | `d1a43be5596ee6de697111dde20ed7dfda7d4c1b4e5107b0418b9f48b789384a` |
| New response schema | `d3182eee405aaad2459b0796cf23432a5ac52849b6868d1af4474582056ecfa2` |
| New condition profile | `62e41b1a581c5305ab0d4e6ebc06e3966f8dbf9a5bc0700b49607764a7f52f1c` |
| Execution controller v3 | `076abcb57bbe1ad4f773df6de59a138d9351294eec898b78c1eee98fee4b8f9a` |
| New five-cell result file | `8fd4fdbff8d1cd909eb1d6dd1a4dd73c5d753c825843164b665598cc41c01e1e` |
| New five-cell completion | `c2d41dc43bf53fa3b4c11c387fc085c2a0d179269a00255038dff3278efa0d18` |
| New offline analysis manifest | `b129dd068df3e05ef7de067ee1550639b88990f45ef6e6a2f7ca77a2d877b3da` |

The private human-readable reports retain each material, capture, literal reading and reference value, plus partial/unassigned/geometry evidence. This repository record omits private paths and raw response bodies. Broader models, independently evaluated OCR, real weak-PC profiles and disagreement-based combinations remain planned in the [runtime and complementary-reader research](local-vision-runtime-research.md). Production layout remains responsible for card geometry; repeated wrong readings from related models cannot certify a value.
