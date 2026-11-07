# Local Tesseract best and fast: Linux development experiment

> **Doc type:** Historical record — completed Linux PSM 11 development experiment; no product backend selected. Last reviewed: 2026-11-02.

Corrected 2026-11-04: verification states, counts and hashes below describe the recorded run, whose references were unverified at that time. The owner has since marked all seven existing public entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Historical artefacts and expected-file hashes are unchanged and do not bind the current annotations; this decision supplies no retrospective accuracy measurement.

## Scope and execution identity

The root-owned headless experiment completed the owner's exact Linux-v1 research approval: Tesseract 5.5.4, Leptonica 1.87.0, the approved OpenMP patch, CMake 4.4.4 and the unchanged existing GCC 12.2 research image. The [controller record](tesseract-linux-research-controller.md) describes source, licence, process and data boundaries. Approval scope SHA-256 is `1c6180099f49e239624c4b85fb427ef10b73093d7635cadc9abe5d7a9857a2ee`; the fixed twenty-condition manifest is `5adbdf2b8fbfa0ab9e711091b025e0cb3fb4006701d7cf70d64468e58d840c31`. No Windows native execution, own FFI, product distribution or additional dependency was adopted.

Execution is bound to source commit `b019d6d936afe858f05b1cd0921d033821647697`, tree `2860b5c47e9c389b39f31df3fc787ee9f8759982`. Its frozen host controller SHA-256 is `0e0c717cb3d71417315355ada41e3fe03f5df45484f62935f3863037fb25323d`; the independently source-compiled Linux worker is `10d92cf10a8618845f52fb86b60400be48fae09a151ff64ae8679b9e7801a95c`. The executed host binary is retained as data beside the local campaign, so subsequent normal Cargo builds do not replace that evidence. This is not an inference rerun of a newer integration.

| Model | Bytes | SHA-256 |
|---|---:|---|
| English best | 15400601 | `8280aed0782fe27257a68ea10fe7ef324ca0f8d85bd2fd145d1c2b560bcb66ba` |
| English fast | 4113088 | `7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2` |

These are two related Tesseract English weight sets. Repeating them under two memory budgets does not produce four independent witnesses. The previous [PP-OCRv6 experiment](local-ocr-nine-pair-experiment.md) remains separately bound to its own Windows source and binary.

## Gates and completion

The first native campaign remains failed and incomplete. Its five fixed compilation steps, source-set and ELF gates passed, then host preprocessing refused the actual Leptonica compile directory before preprocessing or native model execution. The exact cwd correction was regression-tested and independently reviewed. The second campaign uses a fresh prepared build directory and the same approved native closure, image, flags, budgets and total 1800-second build deadline; no compiled native outputs were copied from the first campaign.

The corrected public safe Rust probe passed eight fixed conditions, both actual 26-byte kernel/running-state input-admission proofs and removal of all fourteen owned probe containers. The actual native build then validated the 4,140-file compiler/SDK closure and eight bootstrap identities, completed all five CMake steps and preprocessed all 182 Leptonica plus 152 Tesseract translation units. The retained preprocessed outputs total 487,237,913 bytes. Every unit identity, byte count and digest and all seventeen generated evidence artefacts were checked before the operator accepted the exact build evidence. The build used 1,756,813 milliseconds of recorded container time, within the unchanged total deadline including its final host checks.

The ELF is x86-64 with `/lib64/ld-linux-x86-64.so.2` and only `libstdc++.so.6`, `libm.so.6`, `libgcc_s.so.1` and `libc.so.6` dynamic dependencies. Generated/preprocessed review confirmed the approved feature and link scope. Leptonica's compiled debug utility retains a guarded `system(cmd)` call; the fixed BMP-memory route does not enable its debug flag. Read-only mounts, one PID and no network enforce the runtime boundary. This is source/reachability and boundary evidence, not a syscall trace or a claim that all utility code was removed.

Four native blank-image gates, best/fast at 4/8 GiB, each exited 0 with valid empty-word TSV, no reported OOM and complete cleanup. Only afterwards were the five frozen development crops delivered through stdin. All twenty fixed reads exited 0, produced valid bounded TSV and reported no OOM. No automatic retry occurred. Every receipt binds the exact condition and container; independent reparsing checked raw TSV against every stored word, geometry, input byte count and digest. The recorded running-state/kernel admission proofs passed before input delivery. A separate normal-host inspection confirmed all twenty exact returned container IDs absent. No computer-control session was used.

| Evidence | SHA-256 |
|---|---|
| Accepted build evidence | `3a0d22735b9d12996c48c3ee0a069a373a17bd5a9defe665f8b6f5d3c88af0f8` |
| Twenty-condition completion marker | `20287f5f637c745534851bc14c7d8a4126b78d98f49c699e27e9066ab4ed4301` |
| Independent completed-run audit, including removal inspection identity | `673a7773cb0fb7b2191a3ea196d94a8ba2183fc4e7edf1f561afb9d6a26e2bc1` |

## Development images and literal field comparison

The inputs are the same five balance-free development PNGs as the previous PP-OCR and small-VLM experiments, converted without scaling to canonical 24-bit BMP. Their PNG identities remain in the [nine-pair record](local-ocr-nine-pair-experiment.md#fixed-development-images-and-literal-diagnostics).

| Crop | Dimensions | BMP SHA-256 |
|---|---|---|
| levski-01/v3-sell-5.jpg | 630 × 1220 | `1f7a726b48b19e8ec30b36c67c1da2a8b82770c7d37104b81c884aaf95a8ebb4` |
| patch-city-01/buy-1.jpg | 1030 × 1500 | `be1011c4fa16d6ee844b2989ec1a989aa11e8971c2f9653360ad49af52071c8c` |
| pyro-gateway-stanton-01/buy-1.jpg | 450 × 775 | `9526a92dbd2820655eebebfb605d1b7fc5adf5cbc8cdc2acdc0f9ac5e1ae1e03` |
| ruin-station-01/sell-1.jpg | 1010 × 1470 | `1ffc9787e275313d0714e9d9dbfc110ec96430117b38506da596c537ae6f8992` |
| seraphim-station-01/unselected-buy-1.jpg | 910 × 1510 | `fa4040ada61620a120d4664f80971f1c92a96947e7c79d162c9faedfb7a602bc` |

All ten same-crop/model 4-/8-GiB raw TSV pairs are byte-identical. A separately labelled post-hoc comparison covers price, quantity and status of the same 21 complete cards: 63 fields and 252 field/condition observations. All five references remain `verified:false`; partial cards, including the human-confirmed clipped cargo-size groups, stay excluded. No reference, pixel, verification flag or golden value changed.

The comparison reuses the earlier manually reviewed anchor rectangles. It retains words with centre y within 0.8 anchor height and horizontal overlap of at least 0.2 of the smaller width after expanding the anchor by half its height. All retained word indices, boxes, native confidence strings and raw text are checked against the TSV in original order. The earlier PP-OCR diagnostic associated lines; this one associates words. Neither is a production layout rule or a frozen field metric. All five crops were viewed statically, and the erroneous quantities/prices below were checked against their pixels and surrounding raw boxes.

A separate root sight comparison also transcribed all 63 visible reference fields from the five crops and found no discrepancy against their existing values. That post-hoc agent review is not human golden verification; every `verified:false` flag remains unchanged.

| Capture / material | Field | Reference | best literal, both budgets | fast literal, both budgets |
|---|---|---:|---|---|
| Levski / BERADOM | Stock SCU | 0 | `osu` | `OSCU` |
| Levski / BEXALITE | Stock SCU | 0 | `0SCu` | `OSCU` |
| Patch City / Omnapoxy | Stock SCU | 601 | `501, SCI` | `601 SCU` |
| Patch City / Human Food Bars | Price/SCU | 416 | `H416/SCU` | `Q416/SCU` |
| Patch City / DynaFlex | Stock SCU | 45 | No word in the quantity neighbourhood | No word in the quantity neighbourhood |
| Pyro Gateway / HEPHAESTANITE | Price/SCU | 4167 | `14,167/5C0` | `4,167/SCU` |
| Ruin / Waste | Stock SCU | 0 | `0 stu` | `0 SCU` |
| Ruin / Fluorine | Price/SCU | 1200 | `11,200/SCU` | `S1,2c00/SCU` |
| Seraphim / FLUORINE | Price/SCU | 1090 | `r1,080/SCU` | `41,090/SCU` |
| Seraphim / WASTE | Price/SCU | 198 | `1198/SCU` | `5198/SCU` |

The larger best model changes Omnapoxy's first stock digit from 6 to 5 and Seraphim Fluorine's price digit from 9 to 8. Fast preserves those numeric digits but still attaches a false currency digit to the latter price. Both omit DynaFlex's visible 45 SCU in its quantity neighbourhood; surrounding raw boxes confirm that the association did not simply drop a nearby correctly read quantity. These are observed PSM 11 failures, not evidence about separately untested segmentation modes.

Best preserves Bexalite's zero where all nine previous PP-OCR pairs read letter O. Both Tesseract models preserve Ruin's zero digits where Small/Medium PP-OCR recognisers used O; Tiny already preserved those zeros. Different families therefore supply useful disagreement examples, while currency-glyph errors and missing quantities persist. No size or simple majority is a universal winner. No O-to-zero substitution, glyph stripping, numeric repair, automatic fusion, confidence uplift or verified-field status follows from these observations.

The local `target/tesseract-linux-evaluation-20261102-2/all-field-comparison/` packet retains the complete human-readable field list and 252-row CSV, not only these examples. Its JSON SHA-256 is `50bb6c82c93f73e0eb47494be12c6839d207d4277307ed3bea6a3a816fcb7782`; CSV is `176f3db5d0dcf9b34df2dc1242c27985327bc16ae84f59cee36d3d23775d534b`. Raw private evidence and model/build outputs are not committed. This report does not measure names, side, container sizes, completeness detection, automatic anchoring, vocabulary resolution or sending.

## Resource observations and remaining acceptance

Each fixed worker had two CPU equivalents, one PID, no swap/GPU/network, read-only filesystems and either a 4- or 8-GiB cgroup budget within the same existing VM. This is one physical development PC, not measured weak hardware. The VM, Windows controller and complete application overhead are outside these worker measurements.

| Model | Observed live memory.peak bytes | Docker lifecycle wall milliseconds per crop |
|---|---:|---:|
| best | 49041408–49999872 | 8064–16174 |
| fast | 16379904–17084416 | 8027–16841 |

Live cgroup peaks are lower bounds. The local engine deletes the group before a final read, so every whole-attempt peak remains unknown. Final exit/OOM evidence is recorded separately. Lifecycle wall time includes Docker creation, policy/admission checks, model loading and cleanup; it is not inference-only latency and is not directly comparable to the earlier Windows PP-OCR Cargo timings. Concurrent development and warmed caches are uncontrolled.

This completes only the approved Linux PSM 11 pilot. PSM 7 field geometry is not frozen or implemented. Freeze future line/layout conditions separately, evaluate different admissible OCR families and conservative paired error detection, then measure representative held-out classes and calibrated silently-wrong rates. Production scheduling, whole-app resources, Windows/native distribution duties, clean installation and the no-additional-program goal remain open. The standard PP-OCR reader and its default are unchanged.

Added 2026-11-02: the [joint eleven-profile literal record](local-ocr-joint-reader-comparison.md) reconciles these retained best/fast outputs with all nine PP profiles on the same 63 fields. Identical budget replicas count once per model; its 23 universally equal literals and 55 pair inventories remain string agreement, without accuracy, independence or fusion claims.
