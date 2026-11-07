<!--
Thank you for the pull request. Fill in the sections and tick what applies.
Delete a section marked "(if affected)" when your change does not touch that area.
Never paste a UEX secret key, an app token or an unredacted screenshot here:
pull requests and their attachments are public.
The rules behind every box are in CONTRIBUTING.md.
-->

## What and why

<!-- One to three sentences: what this changes and why. The diff shows how. Link issues: "closes #123", "refs #456". -->

## Requirements and decisions

<!--
The requirement IDs (R-…), assumptions (A…), predecessor bugs (F…), supply-chain measures (S-…), open points (O-…) and ADRs this change implements, changes or relies on.
A change that contradicts a requirement or an accepted ADR needs @greluc's approval first: link it here.
-->

## Type of change

- [ ] Plan or documentation
- [ ] Corpus entry
- [ ] Bug fix
- [ ] Feature
- [ ] Recognition rule, layout profile or threshold
- [ ] Refactoring (no behaviour change)
- [ ] Build, CI or dependencies
- [ ] Breaking change to persisted data or settings (needs a migration)
- [ ] CLA signature (`cla: sign — <handle>`)

## Affected areas

- [ ] `docs/`
- [ ] `corpus/`
- [ ] `shared-kernel`
- [ ] Context crates: `game`, `reference-data`, `capture`, `recognition`, `reporting`, `submission`
- [ ] `workflows`
- [ ] Adapters: `adapter-uex`, `adapter-storage`, `adapter-ocr`, `adapter-vlm`, `adapter-files`, `adapter-platform`
- [ ] `ui`
- [ ] `app` or packaging
- [ ] `tools/ocr-eval`
- [ ] `xtask`, Cargo files (`Cargo.toml`, `Cargo.lock`, vendor directory, `deny.toml`, `rust-toolchain.toml`), `.github/`
- [ ] Governance and licensing files (`LICENSE`, `LICENSES/`, `NOTICE`, `REUSE.toml`, `CLA.md`, `CONTRIBUTING.md`, `CLAUDE.md`)

## How was this tested?

<!--
Be concrete: which tests, on which OS (Windows, Linux), which corpus evaluation, which app launch and what you saw.
If something could not be tested, say so explicitly. "Tests pass" is not a description.
-->

## Checklist

### Every pull request

- [ ] Every commit carries a `Signed-off-by:` trailer that matches its author (DCO 1.1, `git commit -s`). A GitHub noreply address is fine if the commit author uses the same address.
- [ ] I have signed the [CLA](https://github.com/tallyline-sc/tallyline/blob/main/CLA.md): my GitHub handle is in [`docs/cla-signatures.md`](https://github.com/tallyline-sc/tallyline/blob/main/docs/cla-signatures.md), or this pull request is my `cla: sign — <handle>` signature and quotes the acceptance sentence of CLA §11 verbatim in its description.
- [ ] Every commit message and the pull-request title follow Conventional Commits (`<type>(<scope>): <summary>`).
- [ ] The documents this change affects are updated in the same commit as the change, not in a follow-up commit or pull request; contradictions I noticed are fixed or reported. A user-visible change also updates `README.md`, the user guide and `CHANGELOG.md`.
- [ ] Every shipped component or bundled asset this pull request adds or changes has its `NOTICE` entry, and every bundled file (OCR model, font, icon, logo) also has its `REUSE.toml` annotation, its licence text under `LICENSES/` and its SHA-256 in `NOTICE`. Its licence is allowed by the "Stack rules" in `CLAUDE.md`.
- [ ] The Star Citizen Fan Kit unit is untouched: the logo file, the Fan Kit Guidelines section 2b trademark line and the Fankit Agreement clause 2(g) notice are unchanged byte for byte, still rendered together and in English in every locale, and the tests that pin them are unchanged. A deliberate change has @greluc's approval and is named under "Reviewer notes".
- [ ] AI involvement is disclosed with `Co-Authored-By:` trailers that name the tool and model.
- [ ] No requirement, assumption, open point or other ID is reused or renumbered; a dropped requirement is marked `Withdrawn`.
- [ ] Unverified statements are marked as assumptions; versions, API fields and licences name their source.
- [ ] No secrets (UEX secret key, app token), personal data, private or unredacted screenshots, or large binary files in the diff.
- [ ] Everything is written in English with British spelling; verbatim third-party text is unchanged.

### Code (if affected)

- [ ] `cargo xtask check` passes for every commit, because merges keep every commit (no squash merges); CI is green on Windows and Linux.
- [ ] Core crates: the test came first (red, green, refactor); a new use case has an acceptance test tagged with its requirement ID; a bug fix starts with a reproducing test.
- [ ] Domain terms follow the ubiquitous language (11 §A1); aggregate invariants are covered by tests.
- [ ] The code sits in the right crate and module; no new crate dependency outside the context map (02 §2); no technology types cross a crate boundary.
- [ ] No new Clippy, rustdoc, `cargo-deny` or architecture-test finding; a lint suppression is `#[expect(lint, reason = "…")]` (or `allow`) with one short sentence, has its row in the suppression register (09 §3a) and its longer reasoning in the commit message and this pull request.
- [ ] No `unsafe` outside the platform adapter; every `unsafe` site is registered in 09 §3a; no native code in a new dependency unless it has a row in the native-code exception register (09 §3b) and @greluc approved it.
- [ ] No comments besides rustdoc, also none in configuration files; rustdoc describes the actual contract.
- [ ] No magic numbers, boolean control parameters, `unwrap`/`expect`/`panic!` in non-test code or swallowed errors; error cases and cancellation are handled and tested.

### Recognition (if affected)

- [ ] Corpus evaluation run (R-QA-3): no regression in a `gated` class and no new silently wrong field in any class. The result is attached or summarised below.
- [ ] A change to locate, anchors or crop geometry: I looked at the crop dumps of the whole corpus.
- [ ] No absolute pixel or RGB thresholds (R-OCR-17, R-OCR-18).
- [ ] No `expected.json` was regenerated from a recognition run; `verified` was set only by a human who checked the values.
- [ ] A model or runtime change (including the `ort` pin) updates the OCR digest deliberately (R-QA-4); revision and SHA-256 of every OCR asset are recorded in its checksum file and in `NOTICE` (S-22).

### UEX API (if affected)

- [ ] New or changed fields, endpoints or error codes were verified against the live API, with `is_production=0` for anything that writes, and `docs/plan/06-uex-api.md` is updated.
- [ ] Test responses are written against the documented schema; recorded responses are committed only anonymised and only once UEX allows it (06 open point 7).
- [ ] Only resolved IDs from the UEX vocabulary reach the API, never free OCR strings.

### UI (if affected)

- [ ] All texts come from the message catalogue; colours only from the theme tokens, never as literals in view code.
- [ ] Deviations from the UEX value are shown with colour, icon and text (R-UI-10 to R-UI-12).
- [ ] I launched the app and saw the change working.
- [ ] The upload screenshot still never contains the balance (test).

### Privacy and network (if affected)

- [ ] No new network destination and no new kind of data sent to an existing one; otherwise @greluc approved it and 02 §9 is updated.
- [ ] Secrets never reach logs or the diagnostics export (R-NF-4, R-NF-6); images go to the AI recognition only as panel crops (02 §9).

### Dependencies (if affected)

- [ ] Every new dependency is justified under "Dependencies and keys": purpose, SPDX licence ID and its source, maintenance status (S-29), native code or build script.
- [ ] Changelog of the update read; new transitive crates checked in the `Cargo.lock` and vendor diff, including their `build.rs`, procedural macros and native parts (S-9).
- [ ] `Cargo.lock`, the vendor directory and the `cargo-vet` records are updated in this pull request, the lockfile and vendor update as a separate commit (S-3, S-5); `cargo-deny` and `cargo-audit` pass without a new exception, or the exception has its justification and expiry date.
- [ ] Source (crates.io only, no git dependencies), versions and licences follow the "Stack rules" and supply-chain rules in `CLAUDE.md` (S-1, S-2); any exception is recorded in 10 and named below.

### Build and CI (if affected)

- [ ] GitHub Actions are pinned by full commit SHA without a trailing comment, and `docs/dependency-pins.md` records the tag each SHA resolves to.
- [ ] Workflow `permissions` are minimal.
- [ ] No action step runs a proprietary default component (S-37).
- [ ] A CI tool a workflow downloads or installs (including WiX and `cargo-packager`) is pinned by version and SHA-256, or installed with `--locked` at a pinned version or from a hash-locked requirements file (S-36), and has its row in `docs/dependency-pins.md`.

## Dependencies and keys (if affected)

<!-- Per new or updated dependency: name, version, purpose, SPDX licence ID and where you read it, maintenance status, and whether it brings native code, a build script or `unsafe`. Per new `cargo-vet` audit or import: who audited and where it is recorded. Otherwise "None". -->

## Upgrade notes (if affected)

<!-- Database or config schema change and its migration (R-NF-11), changed setting defaults, anything a user has to do after updating. Otherwise "None". -->

## Reviewer notes

<!-- What needs a close look, which trade-offs were made on purpose, and what is deliberately left for a follow-up (link the issue). -->
