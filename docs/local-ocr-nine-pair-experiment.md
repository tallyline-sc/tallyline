# Local PP-OCRv6 Tiny, Small and Medium: nine-pair development experiment

> **Doc type:** Historical record — completed Windows raw-reader development experiment; no production profile selected. Last reviewed: 2026-11-02.

Corrected 2026-11-04: verification states, counts and hashes below describe the recorded run, whose references were unverified at that time. The owner has since marked all seven existing public entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Historical artefacts and expected-file hashes are unchanged and do not bind the current annotations; this decision supplies no retrospective accuracy measurement.

## Scope and source identity

The root-owned headless experiment exercised every detector/recogniser pairing of the author-bound PP-OCRv6 Tiny, Small and Medium data through the existing approved ONNX Runtime 1.31.0 / ort 2.0.0-rc.14. No additional runtime, engine, feature or native activation was selected. Model and ordered-dictionary provenance remain in `crates/adapter-ocr/assets/research/provenance.json` and the Small provenance record; their `native_output_verified:false` fields describe the original static source-review condition, not this later execution. NOTICE and the catalogue retain the Apache-2.0 source/weight evidence.

The experiment source commit is `7b017900aeb7fc33953b2d588845654b241df458`, tree `a88845597242577e545fbe7a5a46e173a451f007`. Small remains the product default; the production capture pipeline is unfinished. No user-facing behaviour changed. The frozen development binary is 7,052,800 bytes, SHA-256 `f26fcdb1cbd5713988f7c7e45a7a59d635329fd41a7b69bb8bdccac4a4554911`. All 668 tracked source files, six program artefacts and eight model/dictionary payloads were frozen and rehashed throughout execution. Source snapshots, Cargo dep-info and mapped runtime identity are retained with the local packet.

| Artefact | Bytes | SHA-256 |
|---|---:|---|
| MediumDetector | 62032837 | `eb13b44b25bb36f89528b68720af8a61d9cf381176107f465db1757b65d086e1` |
| MediumRecognizer | 76554979 | `9c09abf0957f7968c7586464b7397b84ad2387a0497a351af40e9acc71b673ba` |
| SmallDetector | 9880512 | `d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e` |
| SmallDictionary | 74947 | `b5f2bfe2bdd9448429e3e82b51c789775d9b42f2403d082b00662eb77e401c5d` |
| SmallRecognizer | 21159378 | `5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634` |
| TinyDetector | 1780590 | `193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8` |
| TinyDictionary | 27156 | `c5cbe34ef40c29c4df07ed012bf96569cb69a2d2a01a07027e9f13cb832bd9cd` |
| TinyRecognizer | 4462639 | `9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6` |

These are three recogniser weight sets with three detector choices, not nine independent witnesses. Changing a detector is a separate crop/geometry condition. Tiny recogniser uses 6,904 canonical ordered labels plus blank and appended space (6,906 classes); Small and Medium use 18,708 plus blank and space (18,710). The canonical Tiny dictionary was checked at every index against the author configuration. No generated substitute dictionary, model conversion or unchecked model code is used.

## Conditions and completion

The actual app-owned Cargo probe prepares telemetry before runtime creation, starts one named joined CPU worker with a verified below-normal Windows calling-thread boundary, and uses one intra/inter-op thread, sequential execution, no spinning, no CPU arena and no memory pattern. The portable DB/bilinear reader is shared: mask threshold 0.2, unclip 1.4, bounded 1,000 candidates; Tiny box threshold 0.4, Small/Medium 0.45. This is not an OpenCV-parity claim or the production bounded pool/Ready/timer pipeline. The reader retains sessions lazily and the lifecycle probe exercises reuse and explicit inactivity cleanup; production scheduling remains open.

The completed second condition ran 04:41:18–04:47:19 UTC on 2026-11-02. All 60 phases exited 0 without timeout: six original Small phases, then nine pairs times one lifecycle and five image reads (ten lifecycle phases and 50 image reads in total). Every owned Cargo/native process was absent afterwards. Five legacy Small raw files match earlier native runs byte for byte; five research Small/Small line arrays match those legacy lines; five Tiny/Tiny raw files match the earlier completed Tiny condition. Research headers deliberately distinguish profile and file identities.

The first condition is retained as failed/incomplete. Twelve phases succeeded before the research Small/Small lifecycle phase legitimately returned `Detection(Complexity)` on its synthetic colour gradient. The corrected condition uses the same uniform-grey lifecycle fixture as the original Small control for every pair. Only that developer fixture and descriptive guides changed; no geometry cap, safety threshold or production reader arithmetic was weakened. The first condition has no usable resource samples because its monitor did not follow the actual Cargo/rustup process ancestry. Missing counters are unknown, not zero.

The final monitor verified each actual native process ancestry through the owned Cargo PID and collected positive samples for all 60 phases. The packet receipt SHA-256 is `81b6112d2c4877762edd4cf1afd80a55b687227ce11875888bac0897943b2190`; complete packet hashes are `3e4bbbe73e2d480012ccd36bd9ff07ecdac48e9dea5cfc995bd785499f419d9e`; controller is `cdc428f02338d6f7049d466ce2ee27f7e123f4cbd7fd3f5e4c039ee6161fd03c`; completion/compatibility proof is `713784a6363675e4fba98c7ac6dd7dbed1128f9e4737a5eeb7f8ed0e1f3b2cde`. No GUI or computer-control session was started.

## Fixed development images and literal diagnostics

The five balance-free shop crops are the same already inspected development images used in the [2B experiments](local-vision-small-model-campaign.md). They contain 21 reference-complete cards and partial fragments. All five `expected.json` files remain `verified:false`. No reference value or golden status changed. The five PNG crop identities were checked against their frozen receipts. Four original JPEGs were later present as Git LFS pointers whose object IDs match the frozen source metadata; the experiment read the staged PNGs, never a pointer as an image.

| Crop | PNG bytes | SHA-256 |
|---|---:|---|
| levski-01/v3-sell-5.jpg | 683089 | `1bc24d646c2b58a458834067939804cc59464ca390ee7f20f24bdd0b315d1e1d` |
| patch-city-01/buy-1.jpg | 1228405 | `639cdc15704ed1ee7bed288a68310f8e947441da2e6d1e263c02e2a546ac603e` |
| pyro-gateway-stanton-01/buy-1.jpg | 336142 | `802e189ec57396034fa529cbadb3404707db6fa9f2e1944037faaa1fb0cdd4b6` |
| ruin-station-01/sell-1.jpg | 1389095 | `4d30eca48b7317f6db53944d47c9324b85bb1b20ca40beaf88d3a3960418fc8d` |
| seraphim-station-01/unselected-buy-1.jpg | 806999 | `214d4f646d59e7415bb830002e35e2beec446639b4298e050827752c2016e7b1` |

A separately labelled post-hoc diagnostic manually anchors price, stock and status for all 21 complete cards: 63 fields and 567 field/pair observations. Every retained literal is checked against its actual raw line index and the five crops were visually inspected. Coordinates, per-file hashes and split text lines remain in the local JSON/CSV. Partial cards are excluded; their missing values are not invented. In particular, the partial Altruciatoxin has no reference price or stock value. This diagnostic does not measure automatic locate/layout, names, side, container sizes, completeness, vocabulary resolution or sending. It is neither a frozen field metric nor held-out accuracy.

| Capture / material | Field | Reference | Tiny / Tiny literal | Small / Small literal | Medium / Medium literal |
|---|---|---:|---|---|---|
| levski-01/v3-sell-5.jpg / BERADOM | Stock SCU | 0 | `OSCU` | `OSCU` | `OSCU` |
| patch-city-01/buy-1.jpg / Omnapoxy | Price/SCU | 3237 | `3.237/SCU` | `83.237/SCU` | `Q3.237/SCU` |
| patch-city-01/buy-1.jpg / Human Food Bars | Price/SCU | 416 | `只416/SCU` | `8416/SCU` | `0416/SCU` |
| patch-city-01/buy-1.jpg / Fluorine | Price/SCU | 987 | `只987/SCU` | `8987/SCU` | `0987/SCU` |
| ruin-station-01/sell-1.jpg / Waste | Stock SCU | 0 | `0 SCU` | `O SCU` | `O SCU` |
| ruin-station-01/sell-1.jpg / Waste | Price/SCU | 450 | `只450/SCU` | `8450/SCU` | `X450/SCU` |
| ruin-station-01/sell-1.jpg / Fluorine | Price/SCU | 1200 | `只1,200/SCU` | `81,200/SCU` | `X1.200/SCU` |
| seraphim-station-01/unselected-buy-1.jpg / HELIUM | Price/SCU | 772 | `8772/SCU` | `772/SCU` | `¤772/SCU` |
| seraphim-station-01/unselected-buy-1.jpg / WASTE | Price/SCU | 198 | `8198/SCU` | `198/SCU` | `¤198/SCU` |
| seraphim-station-01/unselected-buy-1.jpg / FLUORINE | Price/SCU | 1090 | `只1,090/SCU` | `1,090/SCU` | `81,090/SCU` |

All nine pairs read the three Levski zero quantities (BERADOM, BEXALITE, BIOPLASTIC) as letter `O`. The partial BORASE does the same and remains excluded by its human-confirmed clipping annotation. Tiny recogniser reads the three listed Ruin quantities with digit 0 across all three detectors; Small/Medium recognisers use letter O there. Matching wrong output across related models cannot certify a field.

The currency glyph becomes extra 8/0 or unsupported letters/symbols in different recognisers. Tiny preserves the intended digits for some Patch-City prices where Small adds 8, yet Tiny adds 8 at Seraphim HELIUM/WASTE where Small retains the digits. Medium has a corresponding Seraphim FLUORINE failure. No family dominates these selected examples. Crossed detection also matters: Ruin FLUORINE is `81,200/SCU` with Small/Small but `1,200/SCU` with Tiny/Small and Medium/Small. No glyph stripping, O-to-zero repair, numeric coercion, majority voting or confidence increase follows.

The original twelve-anchor discrepancy ledger remains separate, including the partial BORASE. The expanded 63-field JSON SHA-256 is `45d3b50bd62958b6d09fc5f99c18aa359ee6de4238b4074c3bf5ebed213a3155`; its 567-row CSV is `9cbb4073cef6662fb3d6c1e20af52e361ffbb91292f17ce5d387809da1ff6b4a`. These local raw outputs are not committed; this report retains examples derived from the public corpus and evidence identities.

## Observed resource cost

| Detector / recogniser | Cargo wall seconds per crop | Largest sampled private MiB | Largest OS peak working-set MiB at final sample |
|---|---:|---:|---:|
| tiny / tiny | 2.679–3.206 | 112.7 | 178.2 |
| small / small | 3.641–4.770 | 190.7 | 267.4 |
| medium / medium | 8.283–9.436 | 526.8 | 628.0 |
| tiny / small | 3.396–4.712 | 114.4 | 194.5 |
| tiny / medium | 6.366–8.643 | 274.9 | 282.4 |
| small / tiny | 2.837–4.016 | 167.3 | 250.9 |
| small / medium | 7.057–8.122 | 299.4 | 319.4 |
| medium / tiny | 4.111–4.968 | 435.0 | 558.0 |
| medium / small | 5.267–6.607 | 568.4 | 574.2 |

Each interval covers five fresh processes on the current strong Windows host. Cargo wall time includes process startup, model loading and monitoring. OS caches were uncontrolled and warmed; other Rust builds ran concurrently. Sampled private maxima can miss peaks; the OS working-set peak is the value read at the last sample. Counters cover only the actual native probe process, not the GUI or complete process tree. These are lower-bound observations, not minimum-hardware requirements, cold-load guarantees or steady-state product latency.

Tiny/Tiny is a smaller-resource candidate for subsequent real weak-PC testing; the observed failures prevent selecting it as universally more accurate. The related family disagreements justify testing conservative complementary evidence, but do not yet establish a production combination.

## Remaining acceptance

Freeze field/layout parsing and calibration separately, then run representative held-out classes and paired error-detection experiments. Evaluate different OCR families (including licence/native-reviewed Tesseract, RTen/ocrs or TrOCR where admissible), not only PP-OCR sizes. Actual weak/medium hardware, whole-app/process-tree peaks, Linux inference, packaged telemetry with positive control, interruption/unload, clean-OS runtime delivery and the no-extra-prerequisite goal remain pending. Existing VLM experiments and this raw OCR matrix do not close those acceptance gaps.

Corrected 2026-11-02: the separate approved [Linux Tesseract PSM 11 pilot](local-ocr-tesseract-experiment.md) has since completed twenty fixed development reads with best/fast and both worker budgets. It supplies a different-family diagnostic comparison, not PP-OCR Linux acceptance, a production combination, calibrated accuracy or a controlled cross-engine performance result. The remaining acceptance boundaries above still apply to this Windows nine-pair experiment.

Added 2026-11-02: the [joint literal comparison](local-ocr-joint-reader-comparison.md) reconciles the nine PP profiles with Tesseract best/fast over the same 63 fields and publishes all 55 pair counts. It reuses retained outputs, changes no reference or verification flag and measures neither accuracy nor automatic fusion.
