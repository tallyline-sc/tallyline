# Guarded manual SCU review

> **Doc type:** Living reference — composed; native visual acceptance pending. Last reviewed: 2026-11-02.

## Policy and evidence

R-VAL-2/O-140 use provisional shipping bands of **20 % and 16 SCU**, supported by the bounded [calibration evidence](scu-policy-calibration.md). A difference is Major only when it strictly exceeds both independently typed bands. The one known 86 → 70 trading movement and the explicitly synthetic 560-row/40-quantity experiment support an engineering starting choice; they do not establish population accuracy, representative warning rates or a unique optimum. Broader real empirical validation remains open. No corpus bytes or verification flags change.

`ScuTolerance::baseline` and `ScuToleranceSource::ProvisionalPublicCorpus` identify this source independently of UEX parameters. UEX's 5000 % diagnostic never loosens these bands. `reporting::api::model::StockSettings` validates each chosen pair; percentage may be 0–20 whole points and the exact non-negative absolute minimum 0–16 SCU. Neither override requires changing the other. Settings rejects loose/invalid user edits visibly and does not save them. The configuration reader replaces an invalid or loose persisted band independently with its baseline and publishes a visible validation issue, preserving the valid other band.

## Decisions and readiness

`ReportingApplication::review_manual` atomically reassesses price, stock and status against one immutable environment-matched reference snapshot, saves changed decision state before publication and returns `ManualReview`. It refuses unavailable exact arithmetic without changing values or publishing a partial review. It never re-runs recognition or overwrites USER values. SCU uses only the matching terminal/commodity/side inventory prior, without a commodity-price-average substitute. Missing value, missing prior, known zero prior, future/unknown reference age, stale reference and unavailable arithmetic remain distinct. SELL status 1 never implies zero inventory.

Stock approval binds the exact quantity and the Report's context plus mapped reference environment and unlowered deviation class. Editing that quantity or the Report key clears it. Fresh-reference reassessment revokes it only when the unlowered class worsens (or its mapped reference context changes); different magnitude within the same class, a tightened policy that leaves the class unchanged and stale presentation lowering alone do not revoke it. Focus clears only the persisted unreviewed marker. A required decision still blocks after focus until approved.

`ManualReview::numeric_readiness` checks all entered rows for mandatory price/SCU/status and every unresolved numeric/status confirmation, returning typed `NumericBlock` values; empty vocabulary rows are not sent rows. `ManualReview` is crate-constructed and owns its exact immutable committed Report and `StockSettings`. A caller must use that same Report identity/version and policy, and obtain a fresh review before release/send; an earlier `Ok(())` is not authority for a later edited Report. Exact assessment failure prevents construction of a passing review. Account, observation age, game-version/context, attachment readiness, atomic outbox and network send remain their separate gates. This capability does not claim that full release/submission is implemented. The arbitrary-band `review_stock` research API cannot construct release readiness or bypass the shipping baseline.

## Desktop composition

Manual Report displays stock reference, age, signed SCU difference, percentage or explicit unavailability, effective bands and source. Missing/future ages stay unknown; stale Minor presentation lowers without changing the unlowered decision, and stale Major stays Major. Existing design tokens, icons and text provide marking. Counters and F8/Shift+F8 include stock in row order between price and status. Required fields have stable IDs. Only a direct user action opens the value-bound confirmation dialog. It displays only the currently selected privacy-confirmed private attachment preview when that preview's binding still matches; no original screenshot is opened.

The existing bounded app-owned ManualWorker executes stock review/focus/approval. Requests bind Report ID/version, reference snapshot and validated policy. Policy changes invalidate displayed review; late results from the old policy update neither current assessments nor focus/dialog state. Pending typing still defers passive reassessment. Saved settings apply only after storage succeeds. Settings adds separate exact inputs with visible bounds; it never changes entered quantities.

## Persistence and acceptance

SQLite schema **8** adds `stock_decision` to each stored Report row. Schema **7 → 8** and older forward paths require the existing coordinated verified database/config backup; an empty database migrates directly. Legacy quantities receive no invented approval. Report identities, versions, observation times, recency, price/status decisions, private attachment digests and account screenshot evidence stay intact. Reconstitution refuses contradictory value/decision state.

Configuration schema **5** stores `app.stock_percent` and exact string `app.stock_minimum`. Schemas 1–4 migrate after backup of the exact old bytes; validated release preferences and independent game mappings remain preserved. Missing pre-schema-five choices use the baseline. A future schema is refused. This format change follows the release-process MAJOR-version rule when released; no version is assigned in source.

Public Reporting contracts, real SQLite/config contracts, the bounded worker and headless drawn egui contracts cover these paths. Required native Windows/Linux launch, scale/focus and visual acceptance remains explicitly pending under the computer-control ban. No GUI, native inference, real credential/provider operation, UEX POST, corpus verification or complete submission acceptance is claimed.
