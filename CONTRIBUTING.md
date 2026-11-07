# Contributing to Tallyline

Thank you for helping. The project is in its **planning phase**; [README.md](README.md) records the current implementation status and milestone, and the documents under [`docs/`](docs/) remain the specification.

## The short version

1. **Security problems never go into a public issue.** Report them privately through [GitHub private vulnerability reporting](https://github.com/tallyline-sc/tallyline/security/advisories/new); [`.github/SECURITY.md`](.github/SECURITY.md) explains the process.
2. **Never post a UEX secret key, an app token or an unredacted screenshot** (CURRENT BALANCE, player names, chat). Issues, pull requests, comments and their attachments in this public repository can be opened by anyone, without logging in.
3. Bugs, misreads and feature requests go through the [issue forms](https://github.com/tallyline-sc/tallyline/issues/new/choose).
4. Changes are pull requests against `main`. Commit messages follow [Conventional Commits](#commit-messages), and every commit carries a [DCO sign-off](#dco-sign-off) (`git commit -s`).
5. **Sign the [Contributor Licence Agreement](CLA.md) once** before your first contribution is merged ([how](#contributor-licence-agreement)).
6. Every change updates the documents it affects, in the same commit.
7. The project is licensed under the GNU General Public License, version 3 or any later version (`GPL-3.0-or-later`). You contribute under that licence and the CLA.

## Contents

- [What helps most right now](#what-helps-most-right-now)
- [Code of Conduct](#code-of-conduct)
- [Where things are](#where-things-are)
- [Open decisions](#open-decisions)
- [Asking questions](#asking-questions)
- [Reporting bugs, misreads and ideas](#reporting-bugs-misreads-and-ideas)
- [Changing documents](#changing-documents)
- [Changing code (from M0)](#changing-code-from-m0)
- [Commits and pull requests](#commits-and-pull-requests)
- [Corpus contributions](#corpus-contributions)
- [Star Citizen Fan Kit unit](#star-citizen-fan-kit-unit)
- [Licence](#licence)

## What helps most right now

- **Plan review:** contradictions between documents, wrong or unverified claims, gaps. Requirements have IDs (`R-…`), and the unverified assumptions are listed as `A…` in [`01-requirements.md`](docs/plan/01-requirements.md#assumptions-to-be-verified).
- **Corpus captures** of commodity terminals, especially for the capture classes of R-QA-2 (resolutions, HDR on and off, capture tools, terminal themes). See [Corpus contributions](#corpus-contributions).
- **Verified facts** about the UEX API (the open points in [`06-uex-api.md`](docs/plan/06-uex-api.md)), Star Citizen on Linux and capture tools (the assumptions in 01). Verified means checked at the source, with the source named.
- **Code** only from M0 on: the build, CI and quality gates come before the first domain code.

## Code of Conduct

Everyone who takes part follows the [Code of Conduct](CODE_OF_CONDUCT.md). Report unacceptable behaviour confidentially to [lucas.greuloch@gmail.com](mailto:lucas.greuloch@gmail.com).

## Where things are

| Topic | Where |
|---|---|
| Requirements (`R-…`) and assumptions (`A…`) | [`docs/plan/01-requirements.md`](docs/plan/01-requirements.md) |
| Crates, context map, threading, privacy | [`docs/plan/02-architecture.md`](docs/plan/02-architecture.md) |
| UEX API notes and open verification points | [`docs/plan/06-uex-api.md`](docs/plan/06-uex-api.md) |
| OCR pipeline, corpus and measurement concept | [`docs/plan/07-ocr-concept.md`](docs/plan/07-ocr-concept.md) |
| Engineering rules and Definition of Done | [`docs/plan/09-engineering-principles.md`](docs/plan/09-engineering-principles.md) (§11) |
| Supply-chain measures (`S-…`) | [`docs/plan/10-supply-chain-security.md`](docs/plan/10-supply-chain-security.md) |
| Domain language, DDD and TDD rules | [`docs/plan/11-ddd-and-tdd.md`](docs/plan/11-ddd-and-tdd.md) |
| Architecture decisions | [`docs/adr/`](docs/adr/): ADR-0001 is [`03-language-decision.md`](docs/plan/03-language-decision.md) (superseded); the implementation language, toolkit, supply chain and packaging decision is [ADR-0005](docs/adr/0005-rust-and-egui.md); the licence decision is [ADR-0003](docs/adr/0003-licence-and-contributions.md) |
| Open owner decisions and open verifications | [`docs/adr/0000-open-points.md`](docs/adr/0000-open-points.md) |
| Releases, patch day, UEX API changes | [`docs/release-process.md`](docs/release-process.md) |
| Pinned versions of GitHub Actions and CI tools | [`docs/dependency-pins.md`](docs/dependency-pins.md) |
| Corpus rules | [`corpus/README.md`](corpus/README.md) |
| Working rules for everyone, AI agents included: tech stack, stack and licence rules, architecture guardrails | [`CLAUDE.md`](CLAUDE.md) |
| Licence text, third-party notices, licence of each file | [`LICENSE`](LICENSE), [`NOTICE`](NOTICE), [`REUSE.toml`](REUSE.toml), [`LICENSES/`](LICENSES/) |
| CLA and its signature roster | [`CLA.md`](CLA.md), [`docs/cla-signatures.md`](docs/cla-signatures.md) |
| Security policy | [`.github/SECURITY.md`](.github/SECURITY.md) |

## Open decisions

Open owner decisions and open verifications are kept in one register only: [`docs/adr/0000-open-points.md`](docs/adr/0000-open-points.md). This guide keeps no list of its own. Check the register before you propose something that looks undecided; only @greluc reopens a decided point.

## Asking questions

GitHub Discussions stays off until milestone M1. Until then, ask with the [feature form](https://github.com/tallyline-sc/tallyline/issues/new?template=3-feature-request.yml) and replace the title prefix with `[Question]`.

## Reporting bugs, misreads and ideas

- **Bug:** use the [bug form](https://github.com/tallyline-sc/tallyline/issues/new?template=1-bug-report.yml) and attach the app's diagnostics export (R-NF-6). The export removes secrets, your UEX user name and your operating-system user name, and it contains no screenshots unless you add them. Before it saves the export, the app lists every file it includes: review that preview, because attachments are public.
- **Misread:** a value that the recognition (classic OCR or the AI recognition) read wrongly or missed, or a terminal it did not recognise. Use the [misread form](https://github.com/tallyline-sc/tallyline/issues/new?template=2-misread-report.yml) and attach the redacted panel crops and corrected values from the "Report a misread" export (R-QA-5), or a redacted crop of the shop panel. **Do not attach the export's separate reference-snapshot file:** it holds recorded UEX data, which is not published until UEX allows it ([`06-uex-api.md`](docs/plan/06-uex-api.md) open point 7). Say whether the wrong value was offered as sendable without a confirmation: such *silently wrong* values are the most important defects of this project.
- **Feature or plan change:** use the [feature form](https://github.com/tallyline-sc/tallyline/issues/new?template=3-feature-request.yml) and name the requirement IDs it touches.
- **Wrong data on UEX itself** (prices, terminals, your UEX account, DataRunner status) belongs to [UEX Corp](https://uexcorp.space), not to this repository.
- If a closed issue looks like your problem, open a new one and link the old one.

## Changing documents

In the planning phase the documents are the product. These rules apply to every change:

- **Read first.** Read the documents your change touches before you start.
- **Same commit.** Every change updates the affected documents in the same commit. Documentation is never "caught up later".
- **Fix drift.** If you notice a contradiction between two documents or a stale statement, fix it, even when it lies outside your change, and say so in the commit message.
- **Correct, never rewrite silently.** When a stated fact turns out to be wrong, correct it and add a dated note (`Corrected YYYY-MM-DD: …`).
- **Requirements and ADRs are binding.** A change that contradicts a requirement (`R-…`) or an accepted ADR needs @greluc's approval first; the requirement or ADR is amended before or with the change, never afterwards.
- **IDs are stable.** IDs of requirements (`R-…`, including letter suffixes such as `R-CAP-1a`), assumptions (`A…`), predecessor bugs (`F…`), supply-chain measures (`S-…`), report invariants (`I…`) and open points (`O-…`) are never reused or renumbered. A dropped requirement keeps its row, marked `Withdrawn (YYYY-MM-DD)` with the reason and its replacement.
- **ADRs are immutable once accepted.** A new decision gets a new ADR in `docs/adr/NNNN-kebab-title.md` that supersedes the old one; the old ADR only gets a status line. ADR labels have four digits (`ADR-0001`). Claim the next number against `origin/main` and the open pull requests when you push, because parallel work picks the same number.
- **Do not guess.** Versions, API fields, licences and library behaviour are checked at the source, and the source is named (URL, or file and line). Anything you could not check is marked as an assumption.
- **Do not invent** build commands, file paths, crates or type names that are neither in `docs/` nor in the tree. A crate or version that [ADR-0005](docs/adr/0005-rust-and-egui.md) does not name is chosen and verified at source in M0. If it is not written down, it has not been decided: ask.
- **Diagrams** are Mermaid or plain-text trees; no binary diagram files.
- **English only, British spelling with -ise,** in documents, code, commits, pull requests and issues: colour, behaviour, licence (noun) and license (verb), normalise, organise, recognise. Technical identifiers keep the spelling of their technology (Rust, egui and crate API names such as `egui::Color32`, Cargo's "lockfile", the `LICENSE` file name). Quote non-English text only verbatim, for example an error message.
- **Verbatim third-party text is never edited**, not for spelling, style or formatting: the GPL text in `LICENSE` and the texts in `LICENSES/`, the Code of Conduct, the Developer Certificate of Origin, the licence files and notices of bundled components, and the [Fan Kit notices](#star-citizen-fan-kit-unit).
- **No secrets and no personal data** in documents: no keys, tokens, player handles, balances, e-mail addresses or account names. The project contact address and the CLA roster are the deliberate exceptions.

## Changing code (from M0)

The binding rules are in [`09-engineering-principles.md`](docs/plan/09-engineering-principles.md) (Definition of Done in §11), [`11-ddd-and-tdd.md`](docs/plan/11-ddd-and-tdd.md) and the architecture guardrails in [`CLAUDE.md`](CLAUDE.md). The ones contributors most often miss:

- **Rust only, native code only by registered exception.** Our own code is Rust ([ADR-0005](docs/adr/0005-rust-and-egui.md) D1). A dependency that contains native code (C, C++, assembler, a `-sys` crate, a `build.rs` that compiles or downloads something) needs a row in the native-code exception register (09 §3b) and @greluc's approval first. Every crate has `#![forbid(unsafe_code)]` except the platform adapter.
- **Test first.** In the core crates no production code is written without a failing test (11 §B1). New use cases start with an acceptance test tagged with the requirement ID (the tagging mechanism is chosen in M0). A bug fix starts with a test that reproduces the bug. OCR heuristics may be explored in the evaluation harness, but before the merge their behaviour is pinned down by golden and unit tests that fail without the change.
- **Cargo and `cargo xtask` only.** Build and test with `cargo` and `cargo xtask`, not with an IDE test runner. `cargo xtask check` (format check, Clippy with `-D warnings`, rustdoc lints, tests, architecture rules, `cargo-deny`, the `cargo-vet` exemption rule and the Dependabot rules) is green before every commit; `cargo fmt` formats. These commands exist from M0 on. `cargo xtask check` needs the `cargo-deny` binary on your `PATH` at the version pinned in `docs/dependency-pins.md` (see `xtask/README.md`).
- **No comments besides rustdoc.** This is binding: @greluc decided it for all his projects on 2026-09-26.
  - It covers every source, test and configuration file: no plain `//` or `/* */`, no `#` comments in YAML, TOML, `.gitignore`, `.gitattributes`, `.editorconfig` or shell scripts, no `<!-- -->` in XML, and no commented-out code or configuration.
  - What stays: rustdoc (`///` and `//!`, short, the contract only, no history), the licence header, shebangs, and tool directives without prose after them.
  - The reasoning goes into the commit message and the pull request; durable facts go into `docs/`. Open work is an issue, not a `TODO`. A lint suppression is `#[expect(lint, reason = "…")]` (or `allow`) with one short sentence and a row in the suppression register (09 §3a); the longer reasoning goes into the commit message and the pull request.
- **UI texts** come only from the message catalogue; colours only from the theme tokens, never as literals in view code.
- **UEX API:**
  - Tests use responses written against the documented schema; recorded responses are committed only anonymised and only once UEX allows publishing them ([`06-uex-api.md`](docs/plan/06-uex-api.md) open point 7).
  - Live tests are opt-in (`TALLYLINE_LIVE_TEST=1`) and only ever use `is_production=0`. Never send anything with `is_production=1`.
  - Never put a key or token into code, tests, fixtures, logs or commits.
- **Recognition changes** are evaluated per corpus class, and no field may become silently wrong. After a change to locate, anchors or crop geometry, look at the crop dumps of the whole corpus before the golden sweep.
- **UI or packaging changes** are verified by launching the app, not only by tests.

### Dependencies and licences

The licence policy has one canonical place: the "Stack rules" in [`CLAUDE.md`](CLAUDE.md), which [09 §9](docs/plan/09-engineering-principles.md#9-dependencies-and-build) links to. It says which licences may be shipped, which need @greluc's approval first and how build-only tools are treated. This guide does not repeat it. In practice:

- Check the licence before you write code. If it is unclear, ask first.
- Justify every new dependency in the pull request: what it does, its SPDX licence ID and where you read it (the crate's `Cargo.toml` licence field and its licence file), its maintenance status (S-29), and whether it brings native code, a `build.rs` or `unsafe` that the exception register does not cover. Only crates.io; no git dependencies; the newest non-yanked stable release eligible under the seven-day release-age rule, apart from the registered exceptions (`ort`). Corrected 2026-11-01: the owner restored the cooldown for further version decisions and weekly Dependabot checks, while retaining major upgrades and existing exact approvals (10 S-7). Security updates remain exempt; required source, licence, native-code and compatibility reviews still precede adoption.
- A shipped component gets its `NOTICE` entry in the same pull request. A bundled file (OCR model, font, icon, logo) also gets its `REUSE.toml` annotation, its licence text under `LICENSES/` and its SHA-256 in `NOTICE` (OCR models also in their checksum file, S-22).
- `Cargo.toml`, `Cargo.lock`, the `cargo vendor` directory and the `cargo-vet` records are updated in the same pull request, the lockfile and vendor update as a separate commit (S-3, S-5). New crates are checked for a `build.rs`, procedural macros and the licence of their native parts; what you checked goes into the pull request. The step-by-step procedure for adding or updating a dependency, with the `cargo-vet` audit or import, is in [`supply-chain/README.md`](supply-chain/README.md); a new dependency is audited or imported, never added to the bootstrap exemptions. Failures of `cargo-deny`, `cargo-audit` or `cargo-vet` are never "fixed" by turning the check off or adding an unexplained exception.
- GitHub Actions are pinned by full commit SHA, without a trailing comment; the tag each SHA resolves to is recorded in [`docs/dependency-pins.md`](docs/dependency-pins.md). Workflow `permissions` are minimal. An action step whose default behaviour includes a proprietary component is configured to switch it off (S-37).

### Code from other projects

- Code from basetool and basetool-sc-extractor may be ported only under the porting conditions in [`CLAUDE.md`](CLAUDE.md). Note one consequence: basetool is `GPL-3.0-only`, so a file that contains basetool code stays `GPL-3.0-only` unless @greluc, as its copyright holder, relicenses it, and a release that contains it is effectively `GPL-3.0-only`. Porting basetool code without that relicensing statement therefore needs @greluc's approval first.
- Other third-party code needs a licence that the Stack rules allow; the pull request names it, and `NOTICE` records the origin.
- SC-Datarunner-UEX is closed source: never decompile it or copy from it.

## Commits and pull requests

### Commit messages

The repository follows [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) for every new commit. Earlier commit messages keep their plain style.

```text
<type>(<scope>): <summary>

<body>

<trailers>
```

- **Type:** `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `build`, `ci`, `style` or `chore`. Types map to the repository's labels: `feat` → `enhancement`, `fix` → `bug`, `docs` → `documentation`; the other types have no label of their own. The CLA signature pull request and its commit use `cla: sign — <handle>` (see [Contributor Licence Agreement](#contributor-licence-agreement)); this is the only other type.
- **Scope:** the crate (`recognition`, `adapter-uex`), the document area (`plan`, `adr`, `corpus`) or `deps`.
- **Summary:** imperative mood, lower-case start, no full stop at the end.
- **Breaking change** (a MAJOR version under the release process): `!` after the type or scope, or a `BREAKING CHANGE:` footer.
- **Body:** why the change is made, and the requirement IDs. Reasoning that would otherwise end up in a code comment belongs here.
- **Trailers:** the `Signed-off-by:` trailer on every commit ([DCO sign-off](#dco-sign-off)) and, for AI-assisted work, a `Co-Authored-By:` trailer ([AI-assisted contributions](#ai-assisted-contributions)).

Examples:

```text
docs(plan): close remaining cross-document gaps of the fourth review
feat(recognition): resolve status names through the active global.ini
fix(adapter-files): wait for the stable-file gate before reading a capture
build(deps): add proptest for parser property tests
```

### DCO sign-off

Every commit in a pull request carries a `Signed-off-by:` trailer. With it you certify the [Developer Certificate of Origin 1.1](https://developercertificate.org/) for that commit; read it once before your first sign-off. In short, you state that you have the right to submit the change under the project licence.

```bash
git commit -s
```

- Git builds the trailer from your `user.name` and `user.email`. Never type it by hand: a hand-written trailer is how a wrong address gets in.
- The name and e-mail address in the trailer must match the commit author. GitHub's noreply address (`…@users.noreply.github.com`) is accepted when the commit author uses the same address.
- Signing off is a conscious act: no Git setting adds it to every `git commit` (`format.signOff` only affects `git format-patch`).

Forgot it? Before review starts, add it to the last commit with `git commit --amend --signoff --no-edit`, or to every commit since `main` with `git rebase --signoff main`, then update your branch with `git push --force-with-lease`.

The CI job `DCO sign-off` (`cargo xtask dco`) rejects a pull request with a commit whose sign-off is missing or does not match its author, in name or address; merge commits are not checked. You can run the same check before you push: `cargo xtask dco origin/main HEAD`. The planning-phase commits already carry a sign-off.

### Contributor Licence Agreement

Before your first contribution is merged, sign the [Individual Contributor Licence Agreement](CLA.md) once. It is adapted from basetool's CLA, which is modelled on the Apache Software Foundation's Individual Contributor License Agreement. You keep the copyright in your contributions; the CLA grants a licence, and [`CLA.md`](CLA.md) states what it grants and how that grant may be used.

The CLA and the DCO do different jobs: the CLA is a one-time licence grant that covers all your contributions, while the sign-off states the origin of each single commit. Both are required.

How to sign ([`CLA.md` §11](CLA.md#11-how-to-sign) is the binding procedure):

1. Open a pull request titled `cla: sign — <your-github-handle>` that adds one row to [`docs/cla-signatures.md`](docs/cla-signatures.md): your full legal name, your GitHub handle, the e-mail address you commit with, the date (`YYYY-MM-DD`) and the CLA version. Its commit is signed off like every other.
2. Copy the acceptance sentence from [`CLA.md` §11](CLA.md#11-how-to-sign) verbatim into the pull-request description.
3. Wait until a maintainer has merged the signature pull request, then open your first contribution. The maintainer checks that the name and e-mail address match the commits you submit.

Good to know:

- The roster is public: the name and e-mail address you enter are published and stay in the repository's history. If you would rather not publish an address, commit, sign off and sign the CLA with your GitHub noreply address.
- The CLA covers every contribution that ends up in a commit on `main` under your authorship: code, documents, workflows, corpus transcriptions and typo fixes alike.
- If you contribute on behalf of an employer or another legal entity, an Entity CLA is needed. Contact [lucas.greuloch@gmail.com](mailto:lucas.greuloch@gmail.com) before you start; the Entity CLA template is drafted on the first request (O-80 in the [open-points register](docs/adr/0000-open-points.md)).

### Pull requests, review and merge

- Branch from `main` and open the pull request against `main`. Fill in the [pull-request template](.github/PULL_REQUEST_TEMPLATE.md); one concern per pull request.
- CI must be green before a merge (once CI exists, from M0).
- Review is requested automatically through [`.github/CODEOWNERS`](.github/CODEOWNERS). That file is review routing, not access control. It names @greluc for everything and lists the governance, licence and supply-chain paths explicitly, because those get the closest review.
- Branch protection for `main` (S-28) requires a pull request and green checks; @greluc may bypass it for his own pull requests.
- **No squash merges.** Pull requests are merged with a merge commit or a rebase merge, so every commit lands on `main` as it is, in particular the separate lockfile and verification-metadata commit (S-3, S-5). Each commit therefore has to stand on its own: a Conventional Commits message, a sign-off, and a green `cargo xtask check`.
- Do not force-push a branch once review has started, unless the reviewer asks for it. Never force-push `main`.
- Every user-visible change updates `README.md`, the user guide under `docs/user/` (from M1) and the `[Unreleased]` section of `CHANGELOG.md` (Keep a Changelog) in the same commit. `CHANGELOG.md` does not exist yet; the first user-visible change creates it. An entry is one to three sentences on what changed and why it matters to users, plus the requirement ID.

### AI-assisted contributions

- AI tools may be used. The human who submits the change is responsible for it and must understand it.
- Disclose AI involvement with a `Co-Authored-By:` trailer that names the tool and the model that actually wrote the change.
- The DCO sign-off is always given by a human, under their own identity, never in the name of an AI tool. When @greluc works with an AI agent in this repository, the agent commits under his identity with `git commit -s` and names the model in a `Co-Authored-By:` trailer; he authorised this explicitly. The planning-phase commits were re-authored under his identity with a sign-off on 2026-10-08; their `Co-Authored-By:` trailers name the model.
- AI agents working in this repository follow [`CLAUDE.md`](CLAUDE.md).

## Corpus contributions

Real screenshots of commodity terminals are the project's test data (R-QA-1 to R-QA-3). The rules in [`corpus/README.md`](corpus/README.md) are binding; in short:

- **Consent.** Submit only captures you took yourself, or captures whose author agreed to their publication. By submitting a capture you agree that it is published in `corpus/public/`.
- **Redact before you upload anywhere**, including issues: the CURRENT BALANCE, player names and chat, with filled rectangles only. Do not touch the shop panel, the location field or the anchors, and do not re-encode the rest of the image (PNG stays PNG; JPEG gets a lossless block-level wipe, or is decoded once and stored as PNG, and `sourceFidelity` says so). Check that the file's metadata holds no personal data.
- **Originals are best:** lossless, original resolution, not cropped or re-saved. Images that went through a chat upload are probably scaled and recompressed.
- **Record the conditions** (resolution, HDR on or off, capture tool, renderer, game version; [07](docs/plan/07-ocr-concept.md) §4). Unknown values are `null`, never guessed.
- **Git LFS:** the images are stored in Git LFS. Install `git-lfs` (and run `git lfs install` once) before you clone or add images; otherwise you commit the image itself instead of its LFS pointer.
- **Entry layout:** `corpus/public/<location>-<nr>/` with the images and an `expected.json`. New entries have `"verified": false`; only a human who checked the transcription independently sets it to `true`.
- **Never regenerate `expected.json`** from a recognition run to make a difference go away. Settle a disputed value against the pixels.
- **Recorded UEX data** (an entry's `reference/` directory, the reference snapshot of a misread export) stays in the private corpus until UEX allows publishing it ([`06-uex-api.md`](docs/plan/06-uex-api.md) open point 7).
- A capture that cannot be redacted without destroying the evaluated region is not published; it stays private.
- The crops and corrected values attached to a misread issue become a corpus entry only if you allow it in the form.

The screenshots show Star Citizen game content. It belongs to Cloud Imperium, is not covered by this project's licence (`LicenseRef-Game-Screenshots` in [`REUSE.toml`](REUSE.toml)) and is included only as test material; this project grants no rights in it. Whether Cloud Imperium's terms allow such screenshots in a public repository is not verified yet ([open points](docs/adr/0000-open-points.md)). Your transcription in `expected.json` is under the project licence.

## Star Citizen Fan Kit unit

The app carries the Star Citizen Fan Kit unit, adopted as in basetool. @greluc has accepted the Fankit Agreement; the checked kit version is `Fankit_2025_11_19`. The unit has three parts, which are always rendered together as one unit:

- the "Made By The Community" logo from the Fan Kit, used unmodified;
- the trademark line prescribed by the Fan Kit Guidelines, section 2b;
- the notice prescribed by the Fankit Agreement, clause 2(g).

Rules:

- The two notices are verbatim third-party text. They stay byte for byte as below, in English in every locale, each as one unbroken string. Never reword, translate, wrap, reflow, restyle, merge or "tidy" them. Their quirks are part of the prescribed text: the section 2b line has a space before its third ® sign, while the clause 2(g) notice has none and writes "Ltd." followed by a second full stop.
- The logo file is not changed in any way. `NOTICE` names it by file name and SHA-256, and `REUSE.toml` marks it `LicenseRef-Fankit-Agreement`: it is not covered by the GPL, and this project grants no rights in it.
- Tests pin the unit byte for byte. A change to the unit, or to the tests that pin it, needs @greluc's approval.

Fan Kit Guidelines, section 2b (SHA-256 of the UTF-8 string without a line break: `154deedbb183aeec2ba7f05c3d16416e55ceee634a36ec1f9ee4785b26467fa1`):

```text
Star Citizen®, Roberts Space Industries® and Cloud Imperium ® are registered trademarks of Cloud Imperium Rights LLC
```

Fankit Agreement, clause 2(g) (SHA-256 of the UTF-8 string without a line break: `703b53f047769fe1a5299b53454cc79a36e2b3a09129291baf2c6948ce740933`):

```text
This site is not endorsed by or affiliated with the Cloud Imperium or Roberts Space Industries group of companies. All game content and materials are copyright Cloud Imperium Rights LLC and Cloud Imperium Rights Ltd.. Star Citizen®, Squadron 42®, Roberts Space Industries®, and Cloud Imperium® are registered trademarks of Cloud Imperium Rights LLC. All rights reserved.
```

## Licence

The project is licensed under the GNU General Public License, version 3 or (at your option) any later version, SPDX identifier `GPL-3.0-or-later`. @greluc decided this on 2026-10-08, and [ADR-0003](docs/adr/0003-licence-and-contributions.md) records it. [`LICENSE`](LICENSE) holds the unmodified licence text.

- You contribute under the project licence, which your DCO sign-off confirms, and under the [CLA](CLA.md).
- From M0 on, every source file starts with a licence header: an SPDX copyright line and an SPDX licence-identifier line naming `GPL-3.0-or-later`. Files that cannot carry a header (images, models, generated files, and configuration files under the no-comments rule) are annotated in [`REUSE.toml`](REUSE.toml), and `reuse lint` checks the whole tree.
- The project's copyright line names Lucas Greuloch. When you change a file that carries a header, add one copyright line of the same form for yourself, once per file.
- Not everything in the repository is under the GPL. [`REUSE.toml`](REUSE.toml) and [`NOTICE`](NOTICE) name the files that keep their own licence: the corpus screenshots (game content of Cloud Imperium), the Fan Kit logo files, the Code of Conduct and, once they are added, the vendored crates (each under its own licence), the OCR models and bundled fonts. `NOTICE` also lists every third-party component shipped in the installers.
