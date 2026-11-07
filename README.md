# Tallyline

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="branding/tallyline-wordmark-dark.svg">
    <img alt="Tallyline Line mark and wordmark" src="branding/tallyline-wordmark-light.svg" width="480">
  </picture>
</p>

<p align="center">Capture · Review · Report</p>

<p align="center">
  <a href="docs/user/desktop.md">Desktop guide</a> ·
  <a href="docs/plan/04-roadmap.md">Roadmap</a> ·
  <a href="docs/design-system/README.md">Design system</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

Tallyline is a Windows and Linux desktop client for reviewing Star Citizen commodity terminal data and reporting it to [UEX Corp](https://uexcorp.space). It combines manual entry with a local screenshot-recognition pipeline. UEX reference data supplies default values and plausibility bounds; explicit review decisions preserve the user's values.

> **Development status:** the M1 desktop foundation and manual reporting are being implemented. Reviewed manual Reports use a durable serial submission queue. Desktop capture/OCR integration, local AI selection and supported-system acceptance remain in progress. This repository does not yet provide a finished release.

## Capture, review and report

| Step | What Tallyline provides |
|---|---|
| Capture | Manual commodity Drafts with optional user-entered faction affinity, resumable after restart; bounded image decoding and private screenshot-region foundations. The desktop capture and OCR workflow is still being integrated. |
| Review | Exact decimal prices and SCU quantities, reference comparisons, price/stock/status counters, keyboard navigation and value-bound confirmation. Passive refreshes preserve user input. Manual field commits and explicit actions save together; refused requests keep unfinished text. |
| Report | Explicit test mode, reviewed release into a durable serial queue, remembered account/mode and cancellation before sending. Uncertain sends never replay automatically; explicit row resolution and selective new-Draft copying retain the original attempt. Token-scoped quota reset and recovery are implemented; known unsaved quota answers keep applicable authenticated reads held. Cooldown partition and verified row lifecycle reads are implemented in source; withdrawal, general Submitted copying and native acceptance remain open. |

Offline reference browsing and manual Drafts work without credentials. Onboarding and **Settings → UEX account** validate a masked key before saving it; session-only credentials are also supported. Actual OS-provider, real-service and native visual acceptance remain separate gates.

Settings presents existing theme, SCU tolerance, submission/age, updates, environment mapping and Game log preferences in one column of topic panels. Validation stays with its controls; SCU tolerance still requires an explicit valid save.

## Local recognition and privacy

Recognition is designed to run locally and without per-use inference charges. While the game runs, lightweight OCR is intended to handle extraction; optional local AI can cross-check results when resources are available. Model and engine combinations are still being evaluated.

- Images are never sent to cloud or LAN inference services.
- Planned recognition prefers a supplied runtime and models. If custom integration becomes excessive, local Ollama and requested model downloads are permitted; user installation is a last resort if bundled delivery cannot be implemented acceptably. AI remains optional, local and free of per-use inference fees; no engine or setup route is selected yet ([delivery policy](docs/adr/0013-conditional-ollama-delivery.md)).
- Confirmed screenshot regions stay in private application storage with integrity checks; original frames are not persisted.
- The optional Game log witness is off by default and never fills Report prices, quantities or statuses.

The current Windows OCR runtime still requires the separately installed Visual C++ Redistributable. A complete initial installation profile and physical weak-PC acceptance remain unresolved. See [strictly local recognition delivery](docs/adr/0012-strictly-local-recognition-delivery.md), the [OCR concept](docs/plan/07-ocr-concept.md) and the [image and attachment guide](docs/user/desktop.md).

## Run from source

Tallyline uses the pinned Rust toolchain in [rust-toolchain.toml](rust-toolchain.toml) and egui/eframe 0.37.0. Native code is limited to the registered exceptions in [ADR-0005](docs/adr/0005-rust-and-egui.md).

```powershell
cargo run --locked -p tallyline-app -- --assets crates/ui/assets
```

The assets argument selects the separately shipped fonts and app icon. Portable and installed images keep their approved resources beside the executable. A second launch activates the existing instance and does not start another writer.

Local builds identify themselves as **Development 0.0.0**. `target/debug/tallyline --build-info` (Windows: `tallyline.exe`) prints public compiled metadata and exits before desktop startup. Release versions come from compile-time tag metadata.

Builds can keep generated outputs outside the checkout using `CARGO_TARGET_DIR`; also set external `TEMP` and `TMP` for test scratch directories. See [external workspace output commands](xtask/README.md#external-workspace-outputs).

The [desktop guide](docs/user/desktop.md) covers settings, storage, portable mode, reference environments, credentials, private attachments and release notices. The [queue record](docs/submission-queue.md) describes release, dispatch and recovery boundaries.

## Design and branding

The **Line** mark, slate-blue surfaces, sky accent, IBM Plex Sans and JetBrains Mono NL follow the owner-selected Tallyline design system. Dark, light and high-contrast themes share the same state language.

State indicators use the approved Material Symbols Rounded glyphs alongside their existing text. The separate icon font is hash-checked on startup; its Apache licence and modification notice appear in the offline licence catalogue. Magnified icon and native acceptance remain open.

The [branding folder](branding/README.md) contains ready-to-use GitHub avatars, social previews, outlined SVG wordmarks and app icons. Use the avatar for the organisation and the 1280 × 640 social preview for repository sharing. Logo colours are fixed within each exported variant.

Reference-browser headings, manual review/confirmation, action labels and existing widgets use semantic typography with explicit line heights; exact review values use Mono. Manual price/SCU and stock-tolerance inputs use right-aligned exact-string Mono numerals, muted aUEC/SCU, SCU and % unit suffixes and accessible names including their units. The [design adoption reference](docs/design-system/components.md) tracks the supplied archives, accepted decisions and implementation coverage. The desktop shell provides a persistent top bar, responsive navigation, scrollable pages and bottom status while retaining setup, account and update actions. Review-table composition and rendered native acceptance remain in progress.

<details>
<summary>Implementation details and acceptance boundaries</summary>

Build with `cargo build --locked -p tallyline-app`, then run `target/debug/tallyline --assets crates/ui/assets` (on Windows, `target/debug/tallyline.exe`). The explicit assets argument selects the separately shipped font files; an installed or portable image instead keeps `assets/fonts/` next to the executable. See the [desktop guide](docs/user/desktop.md) for the storage paths, portable marker and settings errors. An explicit second launch restores the existing window even when it is minimised. Reference browsing loads SQLite before refreshing public UEX endpoints in the background. Reference-data adapters use exact decimal values and environment/static Windows proxy support; PAC and per-protocol Windows proxy lists remain unsupported by the current upstream path.

Mutation scores also require retained cause logs; unrelated infrastructure failures invalidate the measurement. See [the evidence boundary and corrected historical results](docs/mutation-evidence.md).

Source builds can keep their build products and automatic xtask work outside the checkout through `CARGO_TARGET_DIR`; executable paths shown under `target/` use the default location. Set external `TEMP`/`TMP` as well for test and mutation scratch directories. See [external workspace output commands](xtask/README.md#external-workspace-outputs).

The optional **Game log witness** in Settings reads only version-validated local facts after explicit opt-in. Choose a channel, enable the witness and refresh to inspect sanitised buy-side observations; `cargo run -p ocr-eval --locked -- witness --channel <directory>` offers the same bounded read-only inspection. The finite allowlist now includes MIC-L2 Long Forest Station and Orbituary, with seven additional literal commodity names; [the validation record](docs/game-log-validation.md) lists the exact pairs and supported build. Prices, quantities and status values are never filled from the log (R-OCR-20).

The **Reference data** page lets you choose LIVE, HOTFIX, PTU, EPTU or TECH-PREVIEW and search terminal names, locations, commodity names and codes. It retains your selection and search during passive refreshes and displays the supplied cache state and endpoint ages. **Settings → UEX environment mapping** changes each channel independently of the optional Game.log witness. Global staff notices remain capped plain text, including while submissions are paused. Corrected 2026-11-02: an app-owned native worker publishes SQLite snapshots before refreshing from UEX.

Onboarding and **Settings → UEX account** accept a masked secret key, use the packaged app token when available and offer an expert own-token override. Read-only validation precedes durable saving; an explicit session-only choice keeps credentials in memory. Linux offers an owner-only plaintext fallback only after Secret Service is unavailable and its warning is confirmed; unsupported filesystems retain the session-only option. Offline browsing and Drafts remain usable. Real OS-provider and native visual acceptance are still pending (R-API-4, R-API-7, R-NF-4).

The manual editor offers **Release Report in shown mode** and **Send test through queue**. Test always fixes dry-run mode. Queue/history preserves the released account and mode, shows holds and permits cancellation before Sending; uncertain sends never replay automatically. Repeated unavailable account checks and their later results survive restart; a permanently rejected captured game version requires a new capture. See [the queue implementation and acceptance boundaries](docs/submission-queue.md).

Settings also offers optional **daily release updates**, off by default. A newer standard GitHub release appears as a passive notice with release notes; offline checks never interrupt work or take focus, and failed settings validation preserves saved preferences. Settings backup retention also survives clock changes. Downloads and installation remain your choice (R-NF-7, R-UI-16).

Starting Tallyline again activates the existing installed or portable instance. If activation fails, an already-running notice appears and no second writer starts (R-NF-10).

The offline licence browser has a bounded component list with keyboard selection, documented licence choices, upstream SPDX declarations and every bundled licence/notice file. Its wrapped, read-only document supports keyboard scrolling, selection and copying; the full GPL uses the same viewer. The current source graph uses egui/eframe 0.37.0; [the adoption record](supply-chain/latest-stable-adoption.json) lists eligible releases, upstream constraints and the approved native limits. The [image intake record](supply-chain/image-intake-adoption.json) records decoder selection and the test-only encoder scope.

The shared image intake decodes SDR PNG (including full-precision 16-bit samples), JPEG and BMP into sRGB. It identifies HDR containers and unsupported colour encodings by content, gives SDR-copy advice and bounds file, header, pixel, profile and codec memory before decoding. Large supported inputs receive integer box reduction with their original dimensions recorded (R-CAP-8, R-OCR-17). Capture import controls and the recognition pipeline remain in progress. The headless OCR diagnostic now starts through an app-owned worker that verifies below-normal OS priority before running; production OCR scheduling remains open (R-NF-3).

Maintainers can check public UEX field/type/code changes without credentials through the existing API-spike CLI. The [contract drift guide](docs/public-api-contract-drift.md) explains the metadata-only baseline and the default-off, main-only weekly workflow; hosted activation remains pending.

The [support-export foundation](docs/diagnostics-export.md) supplies masked previews and exclusive private JSON writes with fixed application-log tails. The desktop Diagnostics and logging flow is still being implemented.

Use the offline [corpus capture and annotation guide](docs/user/corpus-capture.md) and `cargo run -p ocr-eval --locked -- inspect` to inspect conditions, expected values and sample readiness. Recognition metrics are not implemented yet.

The model-free [supplied panel tool](docs/panel-preparation.md) prepares upright safe shop/location crops with source transforms from independently supplied geometry. `ocr-eval prepare-panels` masks source-cell contact with the full-width upward half-plane of the supplied header supporting bottom edge before filtering and writes an exclusive bounded dump; automatic locate, text/tone measurements, corpus-wide crop review and recognition remain open.

Corrected 2026-11-03: game preferences persist independent channel-to-UEX mappings in settings schema 6, preserving witness and release-check choices across migration; the browsing controls save these mappings before applying them.

Startup config migration coordinates existing SQLite and exact settings bytes in one verified backup set, and refuses processing if the backup fails.

Corrected 2026-11-03: the schema-nine application database stores manual Drafts, price/stock/status review decisions and attachment metadata while preserving exact buy/sell averages and known terminal container limits; PTU snapshots contain no LIVE averages. Existing databases require a coordinated backup before migration. Manual Report now shows exact price/SCU/reference differences and price/stock/status counters, F8/Shift+F8 navigation and explicit value-bound confirmation dialogs. Passive reassessment preserves USER values and revokes approvals only under the documented worsening rule. Broader SCU empirical validation, native/provider and approved real-service acceptance remain open; manual release/submission uses the durable queue.

Reference snapshots now retain UEX acceptance switches, commodity tolerances and capped literal staff information independently of browsing availability.

Terminal priors now cache exact price/stock values and source provenance independently for 30 minutes; the typed reader accepts at most ten distinct terminal IDs and PTU snapshots omit LIVE priors.

The native reference browser now receives SQLite snapshots before its first network response and refreshes through the primary UEX host on an app-owned worker. Closing cancels pending GETs and joins the cache owner before releasing writer locks.

Reference status retains bounded Cloudflare Ray metadata and the injected UTC observation time for the last non-JSON response. The primary app uses the fixed `.uk` origin; the technical CLI requires explicit expert-host disclosure before sending credentials to an alternative host. Cached browsing remains usable after a transport failure.

Tallyline prepares local OCR telemetry mitigation before startup work. If required, it relaunches its own executable once with a private child environment; no separate launcher is needed. Windows keeps a waiting bootstrap parent without opening a helper console. The native OCR and packaged network verification remain in progress (R-NF-12).

Recognition integration now includes fixed Small model/dictionary/native data, verified private loading and an explicit headless Windows tensor diagnostic. Synthetic model outputs match the fixed dictionary classes. A developer-only portable raw-image/textline reader adds DB boxes, original-raster crops and lazy reusable sessions with explicit inactivity cleanup; desktop capture/recognition, Linux and complete installation-profile acceptance remain in progress (R-OCR-1, R-SEC-7).

Corrected 2026-11-04: all seven existing public corpus transcriptions are owner-verified, with unchanged values, partial-card boundaries and images. Missing metadata, frozen references and recognition accuracy remain separate; see [corpus status](corpus/README.md#offline-inspection).

Developer-only typed Tiny/Small/Medium OCR research pairs now retain exact model/dictionary identities and private raw experiment receipts. They reuse the existing verified native runtime; production model selection, hardware evidence and complete delivery remain open (R-OCR-1, R-SEC-7; ADR-0012).

Corrected 2026-11-03: confirmed manual screenshot regions now persist at source scale in the private application data directory and remain viewable and encodable after the original is deleted or the process restarts. Missing or changed working copies block attachment use. SQLite schema 9 migrates after a coordinated backup and stores crop integrity plus account-specific screenshot requirement dates; an explicit clear action permits trying without an image. A headless-tested attachment editor component supports explicit local selection/cropping, full-size privacy review, time decisions and actionable blocks. The controls are composed into Manual Report using the current accepted account, including offline reference state. Manual release/submission uses the durable queue; native/provider and approved real-service acceptance remain in progress.

Prebuilt distribution helpers verify version/MSI limits, approved tools and sealed payload/legal/source inputs before fixture insertion. Rebuilt Development MSI/ZIP/deb/tar.gz packages have independent source, payload, legal and metadata inspections; see [their exact signed-source tuples](docs/prebuilt-package-verification.md) and [remaining packaging gates](docs/packaging-spike.md). Portable shared-library data keeps non-executable permissions when archives are written on Windows. Windows OS isolation, authenticated release and clean-OS/native acceptance remain open. Windows OCR retains the separately installed Visual C++ Redistributable prerequisite of ADR-0009; these checks do not establish a no-extra-prerequisite installation.


Prebuilt preparation now seals the existing app, approved native/model/font resources, complete legal material and exact packaging tools before token insertion. The retained seal detects changed bytes, unlisted files or a different helper; physical notice text must match the legal catalogue. The original IBM font licence encoding is restored to its selected upstream hash. Protected installer/source-duty acceptance remains in progress; see the packaging record.

Packaging verification now pins each target’s exact legal catalogue to reviewed source inputs. Secret-free source evidence also rejects a source archive that differs from its unpacked tree. Complete native source scopes and protected installer assembly remain in progress; see [the packaging record](docs/packaging-spike.md).

Manual Report now includes [guarded SCU review](docs/scu-review.md), with provisional 20 % / 16 SCU bands, independent tighten-only Settings, durable exact-value approvals, counters and F8/Shift+F8 navigation. The [policy evidence](docs/scu-policy-calibration.md) records the empirical limits. Corrected 2026-11-03: database schema 9 and settings schema 6 migrate after coordinated backups; manual release/submission uses the durable queue, while native visual acceptance remains in progress (R-VAL-2, R-VAL-7, R-UI-10..12).

Source preparation now verifies actual archived component bytes and seals a source reference before harmless fixture insertion. The normal check rejects stale legal source bindings. Protected installer and installed-system acceptance remain pending; see [the packaging record](docs/packaging-spike.md).

Distribution source checks also bind every workspace manifest, so feature-only changes invalidate stale legal source bindings even when Cargo.lock stays unchanged.

The packaging helper now includes fixed native backend adapters and independently inspected Debian archive/CRT prerequisite contracts. Actual final protected distributions and clean-OS/native acceptance remain pending; the Windows Visual C++ runtime prerequisite remains unchanged.

The shared onboarding/Settings UEX form includes always-visible key guidance, explicit **Show key / Hide key** and **Show token / Hide token** controls for user-entered values, and typed account feedback with distinct glyphs and theme tones. Values start masked; validation, replacement and clearing reset visibility. Token, connection and secure-store errors keep their own actionable wording. Keyboard focus reveals clipped credential controls at the minimum surface and double zoom; native accessibility/scale acceptance remains pending.

Corrected 2026-11-03: remembered validated accounts can release reviewed Reports to the durable queue offline; dispatch waits for successful validation in the current session. Replacement credentials close both release and sending until validated (R-API-7, R-SUB-2). Native/provider and approved real-service acceptance remain open.

The source foundation also contains a versioned local-VLM transcription parser and bounded answer retry contract. It does not select or run an inference engine or change the desktop recognition flow (R-VLM-5, R-VLM-10).

Corrected 2026-11-03: [manual submission recovery](docs/submission-queue.md) adds bounded retries only for proven unprocessed requests, persistent account and API-change pauses, explicit Retry now and actionable returned/rejected outcomes. A production refusal that still needs its receipt safety check remains held across restart and performs only read-only checks. Database schema 10 supersedes schema 9 after the coordinated database/settings backup; settings remain schema 6. Partial-row resolution, daily-quota reset scheduling, native acceptance and an approved real-service dry run remain separate gates (R-SUB-2, R-SUB-6, R-SUB-9, R-SUB-11).

Held recovery work preserves safe unknown UEX identifiers and durable account pauses even when receipt evidence is unavailable or partial. Queue acknowledgement does not replay the uncertain Report; detailed resolution, quota reset and new-Draft duplication retain their separate implementation gates (R-SUB-6, R-SUB-9, R-SUB-11).

</details>

## Documentation

| Area | Start here |
|---|---|
| Product scope and delivery | [Requirements](docs/plan/01-requirements.md) · [Roadmap](docs/plan/04-roadmap.md) |
| Architecture and domain | [Architecture](docs/plan/02-architecture.md) · [DDD and TDD](docs/plan/11-ddd-and-tdd.md) |
| Design | [Design system](docs/design-system/README.md) · [Tokens](docs/design-system/tokens.md) · [Branding](branding/README.md) |
| Recognition and test data | [OCR concept](docs/plan/07-ocr-concept.md) · [Corpus capture guide](docs/user/corpus-capture.md) · [Next local-reader evaluation (provisional)](docs/local-reader-evaluation-next.md) · [Verified-reference reader reassessment (development)](docs/local-reader-verified-reassessment.md) |
| Build and release | [Release process](docs/release-process.md) · [Packaging gates](docs/packaging-spike.md) · [Dependency pins](docs/dependency-pins.md) |
| Verification | [Mutation evidence, 32-partition capacity candidate and withdrawn historical scores](docs/mutation-evidence.md) · [Public API drift](docs/public-api-contract-drift.md) · [Diagnostics export](docs/diagnostics-export.md) |
| Decisions and remaining checks | [ADRs and open points](docs/adr/0000-open-points.md) |

<details>
<summary>Complete engineering plan</summary>

| Document | Contents |
|---|---|
| [01 – Requirements](docs/plan/01-requirements.md) | Functional and non-functional requirements, assumptions |
| [02 – Architecture](docs/plan/02-architecture.md) | Crates, data flow, domain model, platform integration, data-flow inventory |
| [03 – Language decision (ADR-0001)](docs/plan/03-language-decision.md) | Java vs. Rust, evaluation; superseded by ADR-0005 (kept as the record of the evaluation) |
| [04 – Roadmap](docs/plan/04-roadmap.md) | Milestones M0–M5 with acceptance criteria, requirement coverage, risks |
| [05 – SC-Datarunner-UEX bug analysis](docs/plan/05-datarunner-bug-analysis.md) | Known bugs of the predecessor and our fixes |
| [06 – UEX API](docs/plan/06-uex-api.md) | Endpoints, payload, open verification points |
| [07 – OCR concept](docs/plan/07-ocr-concept.md) | Pipeline, screenshot observations, lessons from basetool-sc-extractor |
| [08 – Review](docs/plan/08-review.md) | Plan review: corrected errors, closed gaps, open points |
| [09 – Engineering principles](docs/plan/09-engineering-principles.md) | Modularisation, clean code, tests, definition of done, the native-code exception register – with enforcement |
| [10 – Supply-chain security](docs/plan/10-supply-chain-security.md) | Threat model, securing dependencies, build, CI and releases, licence gates |
| [11 – DDD and TDD](docs/plan/11-ddd-and-tdd.md) | Ubiquitous language, bounded contexts, aggregates and invariants; test-driven approach |
| [Release process](docs/release-process.md) | Versioning, release checklist, licence compliance, upgrade/downgrade, patch-day and API-change procedures |
| [ADR-0002 – Modules by bounded context](docs/adr/0002-modules-by-bounded-context.md) | Why the core is cut by bounded context and adapters by technology (in Rust: Cargo workspace crates) |
| [ADR-0005 – Rust and egui](docs/adr/0005-rust-and-egui.md) | Implementation in Rust with egui; native code only by registered exception (`ort`, bundled SQLite, the TLS provider); supply chain, licence allowlist and packaging for Rust |
| [ADR-0003 – Licence and contributions](docs/adr/0003-licence-and-contributions.md) | GPL-3.0-or-later, DCO and CLA, the Star Citizen Fan Kit unit, notice and source duties, licence gate |
| [ADR-0010 – Release token delivery](docs/adr/0010-release-token-delivery.md) | Separate protected packaging jobs and replacement-token releases |
| [Open points](docs/adr/0000-open-points.md) | The single register of open owner decisions and open verifications |
| [Dependency pins](docs/dependency-pins.md) | Tags of SHA-pinned GitHub Actions; versions and checksums of CI tools |
| [Design-system brief](docs/prompts/design-system.md) | Staged brief for Claude Design: an original design system and a screens canvas, at genre level close to the current Star Citizen website and forum, with the Fan Kit unit and the About dialog; with its [stage messages](docs/prompts/design-system-kickoff.md), the [owner checklist](docs/prompts/design-system-owner-checklist.md) and the [JavaFX handoff](docs/prompts/design-system-javafx-handoff.md) for Claude Code (historical since ADR-0005; the egui rendition replaces it, O-112) |

</details>

## Contributing and security

Plan reviews, redacted corpus captures and code contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md), the [Contributor Licence Agreement](CLA.md) and the [Code of Conduct](CODE_OF_CONDUCT.md). Commits carry a DCO sign-off from `git commit -s` and a Conventional Commits message; pull requests retain the individual commits.

Every contributor signs the CLA once; the [signature roster](docs/cla-signatures.md) records it. Pull requests are merged without squashing.

Report vulnerabilities privately using the process in [.github/SECURITY.md](.github/SECURITY.md).

## Licence and project status

Tallyline is free software under **GPL-3.0-or-later**, without warranty; see [LICENSE](LICENSE). The licence covers project code and documentation. Screenshots and Fan Kit artwork retain their own licences, and the Code of Conduct uses CC-BY-SA-4.0. [REUSE.toml](REUSE.toml), [LICENSES/](LICENSES/) and [NOTICE](NOTICE) record those boundaries. Every release ships the required legal texts/notices and source archives for bundled copyleft components.

The repository is [tallyline-sc/tallyline](https://github.com/tallyline-sc/tallyline). Corrected 2026-11-04: current persistence is SQLite schema 10 and settings schema 6, with coordinated backups before migration. The [open-points register](docs/adr/0000-open-points.md) tracks trademark, Fan Kit/screenshot permissions, icon review, runtime prerequisites and installer acceptance. Source and package foundations do not establish a completed release.

## Star Citizen fan content

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/fankit/MadeByTheCommunity_White.png">
    <img alt="Made By The Community" src="docs/images/fankit/MadeByTheCommunity_Black.png" width="150" height="150">
  </picture>
</p>

Star Citizen®, Roberts Space Industries® and Cloud Imperium ® are registered trademarks of Cloud Imperium Rights LLC

This site is not endorsed by or affiliated with the Cloud Imperium or Roberts Space Industries group of companies. All game content and materials are copyright Cloud Imperium Rights LLC and Cloud Imperium Rights Ltd.. Star Citizen®, Squadron 42®, Roberts Space Industries®, and Cloud Imperium® are registered trademarks of Cloud Imperium Rights LLC. All rights reserved.

The logo, the trademark line of the Fan Kit Guidelines §2b and the notice of the Fankit Agreement clause 2(g) above form one unit and are reproduced verbatim ([CONTRIBUTING.md](CONTRIBUTING.md)). The logo files are Cloud Imperium's artwork, used under the Fankit Agreement; they are not covered by this project's licence (`LicenseRef-Fankit-Agreement` in [REUSE.toml](REUSE.toml)).

## Disclaimer

Independent community project, not made or endorsed by UEX Corp and not affiliated with UEX Corp or Cloud Imperium Games / Roberts Space Industries.

### Unknown submission resolution

The manual queue can inspect an unknown attempt, record an explicit received or not-received decision and request a bounded read-only UEX evidence check. Not received retains the original unknown history; **Duplicate as new Draft** creates one separate unreviewed Report with the original observation time, game version and entered values. All release and send checks still apply. Database schema 13 requires the coordinated backup before migration; settings remain schema 6. See [the queue record](docs/submission-queue.md) for safeguards and open acceptance gates.

### Partial row resolution checkpoint — 2026-11-04

Partial entries provide a passive **Inspect original attempt** for the original target, values and wire date. The queue shows each partial row as verified accepted, explicitly received/not received, rejected, not sent or unresolved. A bounded read-only check verifies each returned ID against the exact original account, terminal, commodity and attempted wire date. Once every row is resolved, **Copy not-accepted rows to new Draft** preserves only the eligible rows and their original observation and values for complete review. Accepted and unmapped rows cannot be copied; the original request never resends. Exact verified production rows supply local accepted priors, while user statements create no IDs or withdrawal permission. See [the implementation and evidence limits](docs/submission-queue.md#partial-row-resolution-checkpoint--2026-11-04). Daily-quota reset, general lifecycle/withdrawal, native acceptance and an approved real-service dry run retain their gates.

## Daily token quota source checkpoint

Corrected 2026-11-04: the queue now retains the daily reset supplied for the exact attempted app token, pauses authenticated receipt reads and concurrent submissions for that token as soon as quota exhaustion is known, and resumes eligible work only through current send checks. Known token replacement keeps independent holds; account changes, cancellation and ordinary Retry cannot bypass them. The queue shows a reset check rather than a promised send time, and restart still requires renewed release. Schema 13/configuration 6 has source-bound previous12 backup/rollback contracts. See [the user guide](docs/user/desktop.md#daily-app-token-quota) and [source checkpoint](docs/submission-queue.md#token-quota-reset-checkpoint--2026-11-04); the explicit cooldown split and native/real-service acceptance remain open.

Corrected 2026-11-05: supplied-panel dumps preserve provably safe skewed title pixels, refuse unsupported header labels and use schema 2. Historical schema-one geometry requires regeneration and review; see [the panel contract](docs/panel-preparation.md).

## Explicit mixed-cooldown Reports

Corrected 2026-11-06: the queue's **Send the others now** action explicitly partitions mixed cooldowns into two released Reports while preserving values, decisions, screenshot evidence, observation time and optional affinity. It retains the original job history and repeats every normal send check for each replacement. Database schema 15/configuration 6 requires a coordinated backup before migration. See [the user guide](docs/user/desktop.md#send-the-other-rows-now) and [source checkpoint](docs/submission-queue.md#explicit-mixed-cooldown-split-checkpoint---2026-11-06). Complete M1, native and approved real-service acceptance remain open.


Corrected 2026-11-07: a permanent client-review refusal now offers **Duplicate as new Draft** once, preserving the original Report and attempt history while renewing all review and screenshot ownership decisions. A refusal requiring a new capture has no copy action. Database schema16/configuration6 uses coordinated backups and older-reader refusal. See [the queue checkpoint](docs/submission-queue.md#permanent-refusal-copy-checkpoint--2026-11-07) and [user guide](docs/user/desktop.md#copy-a-permanently-refused-report); complete M1 and general lifecycle/withdrawal acceptance remain open.

### Verified lifecycle read checkpoint

Verified production rows now show their saved UEX status, editability and check time, with automatic checks and account-throttled **Refresh lifecycle**. A positive decline or expiry revokes only its matching accepted local prior; original delivery and history remain retained. Database schema17/configuration6 requires the coordinated backup. See [the user guide](docs/user/desktop.md#check-a-submitted-rows-lifecycle) and [implementation/evidence limits](docs/submission-queue.md#verified-lifecycle-read-checkpoint--2026-11-07). Withdrawal, general Submitted copying and full M1/native/real-service acceptance remain open.