# SCU policy calibration evidence

> **Doc type:** Living reference — provisional starting-pair recommendation; broader empirical validation open. Last reviewed: 2026-11-02.

Corrected 2026-11-04: verification states, counts and hashes below describe the recorded run, whose references were unverified at that time. The owner has since marked all seven existing public entries verified; see [current corpus status](../corpus/README.md#offline-inspection). Historical artefacts and expected-file hashes are unchanged and do not bind the current annotations; this decision supplies no retrospective accuracy measurement.

## Binding rule and evidence boundary

[R-VAL-2](plan/01-requirements.md) and [O-140](adr/0000-open-points.md) require independently typed percentage and absolute SCU bands, both calibrated on the corpus in M1 and only tighten-able. A difference is Major only when it strictly exceeds both bands. The verified server `scu_variation` of 5000 % is diagnostic only, not a client baseline. [07 §2.6](plan/07-ocr-concept.md#26-confidence-rule-based) preserves the unlowered class for decisions, lowers only stale Minor presentation and separates USER reading confidence from deviation.

These requirements do not mandate frozen UEX priors before any exploratory parameter work. Contemporaneous priors would directly test deployment comparisons; successive known-time screen quantities can instead provide limited evidence about genuine trading movement. Neither current API values nor synthetic digit edits reconstruct historical UEX comparisons. [The API source record](plan/06-uex-api.md) establishes `scu_buy` and `scu_sell_stock` as inventory priors; forecast `scu_sell`, status labels and commodity average prices are not quantity substitutes.

## Available public sample

The read-only materialisation of 155 public images and five existing expected files has manifest SHA-256 `9acf89f16aec154a3cc5e760a6da60f51e146c70c5274beb468e8041cbbcce4a`. Its metadata inspection receipt has SHA-256 `5677b9ddcd7757d9e683689484f290ab342fb59ae4d70df00ab7a25850206cb2`. All five entries remain `verified: false`; recognition was explicitly skipped/unimplemented, private corpus unset/skipped and frozen reference directories absent. The nine owner-confirmed Levski bottom labels concern geometry only.

The external `exploratory-scu-temporal-pairs.json` receipt (SHA-256 `06e2bd8d14e41fb64896955d5c785fb8bce0eec3b3d06e126b0f9db5d648abd3`), derived from those expected files rather than OCR, contains 540 complete rows in 257 entry/side/commodity groups. It excludes 209 partial rows and 20 rows lacking commodity identities, and finds no same-time contradiction. There are 282 stable adjacent pairs and one changed pair: [Levski](../corpus/public/levski-01/expected.json), BUY Agricium (commodity 1), `v1-buy-1.jpg` 86 SCU at 2026-10-14T21:05:56+02:00 to `v2-buy-1.jpg` 70 SCU at 21:08:21+02:00, 145 seconds later. [07 §1c](plan/07-ocr-concept.md) independently records this real in-game stock change. Its exact absolute change is 16 SCU and its relative change is 1600/86 %, approximately 18.604651 %. The earlier screen reading is not an independently frozen UEX prior. Repeated stable views are correlated, not 282 independent trading events.

External evidence is retained under `D:/Coding/claude-worktrees/tallyline-implementation/issue-84-scu-policy/`; the materialisation and temporal receipt remain in the task's read-only public corpus output. No image, annotation, verification flag or LFS object was changed.

## Bounded candidate sensitivity

The following table is a parameter sensitivity exercise, not calibrated field accuracy, OCR performance or a shipping recommendation. `P` is a whole percentage and `A` an absolute SCU minimum. All 282 stable pairs are Equal under every candidate. For the sole changed pair, Major requires `1600 > 86 × P` **and** `16 > A`.

| P | A = 0 | A = 15 | A = 16 | A = 25 |
|---|---|---|---|---|
| 10 % | Major | Major | Minor | Minor |
| 19 % | Minor | Minor | Minor | Minor |
| 25 % | Minor | Minor | Minor | Minor |
| 50 % | Minor | Minor | Minor | Minor |

Thus 14 of these 16 candidates accept the observed genuine change. Any whole percentage at least 19 **or** absolute minimum at least 16 does so, including many untested combinations. One changed observation cannot identify a unique pair, estimate representative normal-trading tails or support an optimised false-warning rate.

Independent worked examples exercise the safety trade-off without claiming observed recognition errors:

| Deliberately supplied prior → value | Exact change | Result for these 16 candidates |
|---|---|---|
| 86 → 860 | 774 SCU; 900 % | Major for all |
| 12,000 → 1,200 | −10,800 SCU; −90 % | Major for all |
| 12,000 → 120,000 | 108,000 SCU; 900 % | Major for all |
| 2 → 20 | 18 SCU; 900 % | Major at A = 0, 15, 16; Minor at A = 25 |

The last example exposes the intended absolute-floor trade-off: a floor large enough to suppress small trading movements can also suppress a small-stock digit error. No broad digit-error sensitivity metric follows from four constructed examples. The public Reporting application contracts reproduce this table with literal independently worked expectations, exact decimal boundaries and a bounded monotonic tightening matrix. Candidate values in tests supply no production settings.

## Explicit assessment checkpoint before guarded adoption

`ScuTolerance::new` requires both explicit inputs and has no `Default`, no UEX relaxation and no calibration provenance claim. `tighten` clamps the two bands independently and reports a looser attempted override. `ReportingApplication::review_stock` compares an entered quantity against only the matching environment/terminal/commodity/side prior without writing Drafts or approvals. Known zero differs from missing; a nonzero change against zero exceeds the percentage band and is Major only above the absolute band. Missing priors have no average substitute. Future timestamps have unknown age; staleness affects display only; unrepresentable exact arithmetic produces an unavailable assessment. A sell-side status 1 does not alter stock.

At this preceding checkpoint, the application assessment capability supported explicit policy evaluation without complete guarded desktop quantity review. No shipping pair, config persistence, schema change, stock approval, release operation, submission eligibility or native visual acceptance is activated. Schema 7 remains current. Issue [#84](https://github.com/tallyline-sc/tallyline/issues/84) remains open.

Corrected 2026-11-02: R-VAL-2 and O-140 require corpus-informed starting values, not a unique mathematical optimum or a validated population error rate. The provisional recommendation below is a routine initial engineering choice under those requirements. Narrow sample support is recorded as a limitation rather than an additional implementation prerequisite. The settings/default source, durable value/context-bound decisions, desktop review and release reassessment remain the next implementation slice; broader empirical validation remains visibly open.

## Complete-quantity experiment and provisional start pair

**Recommend 20 % plus an absolute minimum of 16 SCU for the next implementation slice.** These are provisional corpus-informed start values, not an observed OCR accuracy result. The 20 % band accepts the documented 18.604651 % genuine movement with approximately 1.395349 percentage points of margin; the 16 SCU floor also absorbs that movement at its exact absolute boundary. The percentage lets the allowance grow with stock, while the floor avoids treating every small-stock digit difference as Major. This is a recorded engineering trade-off, not a claim that the one movement establishes a whole trading distribution.

The separate quantity-only experiment reads every complete numeric SCU in the five unchanged public expected files, including complete quantities without commodity IDs. It finds **560 rows and 40 distinct quantities**, with **209 partial cards excluded**, **20 quantity rows without commodity IDs**, no missing quantity among complete cards and no verified entry. The earlier temporal-pair sample has 540 rows because it requires commodity identities. There are 382 zero rows and 98 rows of 12,000 SCU; repetition and stable views therefore dominate the row-weighted results. No image or inference engine is read by this experiment.

Reproduce it with `cargo run -p ocr-eval --locked -- scu-policy --public corpus/public`. Output is JSON containing input SHA-256 values, the quantity histogram and all 30 candidate results. The retained `public-scu-sensitivity-01.json` receipt has SHA-256 `9de0b9d6b6d55ecf7704efe2e830988e91d7db644a57280689da301e32a6e220`. The command reads at most 64 expected files of at most 4 MiB each and at most 65,536 cards. This finite digit experiment supports non-negative whole-SCU transcriptions only and explicitly refuses unsupported numeric types instead of silently dropping them; production `ScuQuantity` still supports exact fractions.

For each quantity, the experiment constructs every single-digit deletion (excluding an empty result), insertion of 0–9 at each position and adjacent unequal-digit transposition. Leading zeroes normalise numerically; unchanged values and duplicate results are removed separately per source quantity and family. Families remain separate. Each generated value is compared with its source quantity as a **constructed reference**, not an actual UEX prior. Counts below are generated comparisons labelled Major, not observed misread detections or accuracy. Distinct-quantity weighting counts each of the 40 scalar shapes once; row weighting repeats each shape as often as it occurs in the metadata.

| Candidate | Distinct deletion / 96 | Distinct insertion / 1,449 | Distinct adjacent swap / 60 | Row-weighted swap / 313 |
|---|---|---|---|---|
| 10 %, 16 SCU | 94 | 1,429 | 40 | 274 |
| 19 %, 16 SCU | 94 | 1,429 | 32 | 160 |
| **20 %, 16 SCU** | **94** | **1,429** | **31** | **159** |
| 25 %, 16 SCU | 94 | 1,429 | 27 | 154 |
| 50 %, 16 SCU | 94 | 1,429 | 18 | 135 |

At 20 % / 16 SCU the row-weighted deletion count is 479 of 485 and insertion 11,174 of 15,021. The latter is heavily affected by repeated zero-stock rows and their deliberately generated small insertions. No probability of a real reader making any of these edits is assigned.

The 20 % choice retains four distinct adjacent swaps that 25 % would leave within tolerance: 86 → 68 (−18 SCU; −20.93 %), 270 → 207 (−63; −23.33 %), 1,142 → 1,412 (+270; +23.64 %) and 1,482 → 1,842 (+360; +24.29 %). Their source shapes occur five times, explaining the row-weighted difference of five. Choosing 19 % instead flags one additional distinct generated swap, 456 → 546 (+90; +19.74 %); the rounded 20 % starting band leaves that case within tolerance while allowing more margin above the documented genuine change. None of these generated swaps is claimed to have occurred in OCR.

| Absolute minimum at 20 % | Distinct deletion / 96 | Distinct insertion / 1,449 | Distinct swap / 60 |
|---|---|---|---|
| 0 SCU | 96 | 1,449 | 33 |
| 8 SCU | 96 | 1,441 | 33 |
| 15 SCU | 94 | 1,430 | 31 |
| **16 SCU** | **94** | **1,429** | **31** |
| 25 SCU | 93 | 1,416 | 29 |
| 32 SCU | 90 | 1,410 | 28 |

The 16 SCU floor is tied to the actual 16 SCU movement rather than a server percentage, a terminal capacity or a status rule. Its blind spots are explicit: source quantity 12 generates deletions to 1 and 2, both within the floor; a generated 1 → 17 change is exactly at the floor and remains Minor. Ordinary trading below the percentage or absolute band is intentionally not Major. A 25 SCU floor would additionally hide the worked 2 → 20 example and more large differences than 16. These comparisons justify the bounded provisional choice without implying universal correctness or a statistical optimum.

The public CLI fixture independently works out deletion/insertion/swap totals for 0, 2, 70 and 86, repeats 86 to distinguish weightings, checks 20 % versus 25 %, and verifies that expected bytes and verification flags do not change. Unsupported fraction, negative and string quantities fail visibly. Neither the fixture nor the complete-corpus perturbation sweep upgrades annotation verification, measures golden accuracy or substitutes for future real trading and reader-error observations.

The recommendation can proceed to typed settings adoption and guarded quantity decisions without a new owner approval merely for selecting routine initial values. At that preceding checkpoint the code remained the explicit read-only interface described above, without an activated default, approval, schema migration or desktop release capability. Broader observations can inform later reviewed calibration work. Issue #84 remains open for its remaining composed quantity-review and release acceptance.


## Guarded review adoption (2026-11-02)

Corrected 2026-11-02: the preceding explicit-assessment checkpoint has been extended by [guarded desktop review](scu-review.md). The provisional 20 % / 16 SCU pair is now the typed shipping baseline with independently tighten-only settings, durable decisions and numeric readiness. The evidence, limitations and all verification flags above remain unchanged. Broader empirical validation and required native visual acceptance remain open; complete atomic release/send belongs to its separate slice.
