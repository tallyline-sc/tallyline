# xtask

Repository checks and generators, run as `cargo xtask <task>` (ADR-0005 D12). Nothing here ships in the application.

| Task | What it does |
|---|---|
| `check` | The supply-chain guard, the reviewed build-time code inventory (S-12), the approved native-scope restrictions (see below), `cargo deny --locked check licenses sources bans` (needs `cargo-deny` 0.20.2 on the `PATH`, see below), the justification rule for the suppressions in `deny.toml`, the exemption rule for the `cargo-vet` store in `supply-chain/` (no network, no `cargo-vet` binary), the Dependabot rules over `.github/` (weekly checks and an explicit seven-day release-age delay for every ecosystem, no version filters or auto-merge), the byte comparison of `LICENSE` and `LICENSES/GPL-3.0-or-later.txt`, the licence header of every Rust file, the English check over the tracked files (all three read the files Git tracks or would track), the architecture check over locked `cargo metadata`, `cargo fmt --all --check`, Clippy with `-D warnings`, rustdoc with `RUSTDOCFLAGS=-D warnings`, all tests, core line coverage (at least 85%, or an explicit no-core skip), and the requirement-tag check, in that order; every Cargo call that resolves dependencies passes `--locked` |
| `install-cargo-mutants` | Installs the fixed cargo-mutants 27.1.0 Windows/Linux X64 release with native curl and tar, verifies its SHA-256 and exact version through Cargo, then appends its directory to the existing `GITHUB_PATH`. Refuses unsupported or mismatched hosts and pre-existing installation directories; failure removes only newly owned output. |
| `mutation [--in-diff <diff>] [--shard <k/n>]` | Selects the exact core workspace packages from locked Cargo metadata, lists their mutants, then runs pinned `cargo-mutants` with all workspace tests and `--cargo-arg=--locked`. A PR diff restricts selected mutations to changed lines; the nightly workflow runs all core mutants. Optional zero-based round-robin shards use distinct `target/core-mutation/shard-<k>-of-<n>/` directories. Every prelisted mutant must complete exactly once. The completed report must have a successful baseline, consistent outcome counts, no timeouts and a score of at least 100% (`caught / (caught + missed)`; unviable mutants excluded). A completed unviable-only shard is scoreless pending aggregation; an unsharded all-unviable measurement still fails. No core packages or selected mutants is an explicit skip with no baseline measured |
| `mutation-aggregate <directory> <head> <run> <attempt>` | Requires all 32 exact artifact identities, complete lists, round-robin partitions and outcomes, then enforces the 100% floor on their real combined counts. Missing, foreign, partial or duplicate evidence fails; an all-unviable whole selection fails; all 32 empty selections visibly skip without a score |
| `dco <base> <head>` | The DCO check of ADR-0003: runs `git log <base>..<head>` and fails on any non-merge commit without a `Signed-off-by` line in the last paragraph of its message, in the first column, whose name and address match the author (GitHub noreply addresses match with and without the numeric prefix), and on an empty range. CI runs it on pull requests with the pull request's base and head commits; locally, `cargo xtask dco origin/main HEAD` |
| `native-scope` | Runs the supply-chain guard and the same two supported-target metadata restrictions used by `check`, without reading or changing the cargo-vet store |
| `build-code` | Runs the supply-chain guard, then checks the reviewed S-12 inventory against full locked offline metadata, exact registry/archive identity and actual entrypoint hashes. Missing/stale reviews or changed GL generator consumers fail. No network or dependency build is performed |
| `guard` | The CI guard of 10 S-4: reads `Cargo.lock`, every `Cargo.toml`, every `.cargo/config.toml`, every `rust-toolchain.toml` and everything under `.github/`, and fails on a package without checksum or from another source, on `[patch]`, `[replace]`, `[source]`, `[registries]`, the listed environment overrides, a `git` or `registry` dependency, a Cargo command without `--locked`, and a toolchain pin that is not an exact version. It needs no network |
| `osv-gate <report.json>` | The vulnerability gate of 10 S-19: reads the JSON that `osv-scanner scan source --format json` wrote for `Cargo.lock`, selects the shipped packages with `cargo metadata` and `cargo tree` (normal edges, all features, the Windows and the Linux target, every workspace member except `xtask` and the crates under `tools/`), and fails when a shipped package has a vulnerability group with a CVSS score of 7.0 or more or without a score, or when the report cannot be read. Findings in tooling packages and shipped findings below 7.0 are printed, not blocking. It needs no network |
| `vendor-hashes write` | Vendors the dependencies with `cargo vendor --locked --versioned-dirs` into `target/xtask-vendor` under an empty `CARGO_HOME`, hashes every package directory, writes `supply-chain/vendor-hashes.txt` and deletes the tree. Needs the network |
| `vendor-hashes verify` | The same vendor run, then compares the tree with the committed list and fails on a missing, unexpected or changed package. Needs the network; CI runs it on both systems |
| `notices` | The notices directory of R-DOC-3 (10 S-34): selects the shipped crates with `cargo tree` (normal edges without procedural macros, all features, the Windows and the Linux target, the shipped workspace members of `osv-gate`), reads their licences from `cargo deny --locked list --format json --layout crate`, vendors the dependencies as `vendor-hashes write` does, and writes one directory per shipped target, `target/notices/<target>/legal/`, from the crates shipped for that target: every licence or notice file of a shipped crate (a name starting with `LICENSE`, `LICENCE`, `COPYING`, `COPYRIGHT`, `NOTICE`, `UNLICENSE`, `ThirdPartyNotices` or `PATENTS`, in any directory of the crate, or any file of a directory named `LICENSES`, `LICENCES`, `LICENSE` or `LICENCE`) unmodified under `crates/<name>-<version>/`, the index `crates/index.tsv` (crate, version, the SPDX expression of the vendored manifest's `license` field or `-`, the licences `cargo-deny` detected), the generated `legal/catalogue.json` for the offline About view (name, version, licence, source, copyright and full notice texts), and the files of every shipped entry of the reviewed list `supply-chain/notices.toml` under `<directory>/`. It fails, and removes `target/notices/` so no stale directory can be packaged, when a shipped crate is not vendored, has no such file in its vendor tree or a shipped reviewed-list entry supplies it, or has no licence in the report, an entry names a crate that `Cargo.lock` does not hold, or a shipped entry lacks a file. Needs the network and `cargo-deny`; CI runs it on both systems |

## External workspace outputs

Automatic `xtask-vendor`, `notices` and `core-mutation` trees follow `CARGO_TARGET_DIR`. Without that environment override, their existing `target/` locations remain the defaults. A relative override resolves against the original invocation directory, matching Cargo, and becomes absolute for nested workspace commands even when they run from the workspace root. Normal commands leave Cargo configuration unchanged when the override is absent. See [Cargo path semantics](https://doc.rust-lang.org/cargo/reference/config.html#config-relative-paths).

Corrected 2026-11-03: `check` runs its Cargo quality steps, coverage and requirement listing with both `CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` set to the owned `xtask-check/` child of the resolved target root. The outer `cargo xtask` executable stays in its original target directory. Workspace feature unification can relink that executable during nested tests, which Windows refuses while it is running; independent artefacts avoid this self-lock without changing the workspace graph, features, locked arguments or gates. Metadata-only supply-chain and architecture checks retain the original workspace and output semantics. This check-specific build-directory override also isolates an inherited `CARGO_BUILD_BUILD_DIR`; other commands retain their existing configuration behaviour. The child is reusable by the same checkout and is never filled by copying compiler artefacts. Coverage now resides under this check-owned target root.

Each generated tree carries a `.tallyline-xtask-owner` marker bound to its canonical workspace and directory. Replacement and cleanup refuse unmarked, mismatched, linked or obstructed trees; they preserve foreign content. Choose a fresh task-owned output root when an earlier unmarked directory exists. Reuse belongs to the same checkout with one active writer. Cleanup never removes the target root itself. A vendor run still uses a fresh empty Cargo home inside its owned scratch directory, and the committed `supply-chain/vendor-hashes.txt` remains the explicit output of `vendor-hashes write`.

The mutation workflow writes each shard's source/run/attempt identity only after the tool has prepared its owned output. This post-execution step runs after either success or failure, checks the existing ownership marker and never precreates a report directory. Artifact retention still runs after a failure; an early failure without a tool-owned directory remains a failed shard with no fabricated selection or score.

Mutation reports and selections use the resolved external `core-mutation` tree. The pinned mutation tool builds copied source roots below system temporary storage; its subprocess receives relative `CARGO_TARGET_DIR=target` and `CARGO_BUILD_BUILD_DIR=target`, so each copy owns its build and intermediate artefacts. `TEMP` and `TMP` select the external parent for these copies and filesystem tests, including the offline research CLI contracts. Existing mutation selection, locked workspace argv, timeout and score gates remain in force. Coverage follows the check-owned external Cargo target root through the pinned `cargo-llvm-cov` tool.

For this Windows workspace, use a fresh task directory before the first Cargo invocation:

```powershell
$taskRoot = 'D:/Coding/Agents/Temp/codex/tallyline-check'
New-Item -ItemType Directory -Force "$taskRoot/tmp" | Out-Null
$env:CARGO_HOME = "$taskRoot/cargo-home"
$env:CARGO_TARGET_DIR = "$taskRoot/build"
$env:TEMP = "$taskRoot/tmp"
$env:TMP = $env:TEMP
cargo xtask check
cargo xtask notices
```

Use a different task root for another checkout. Empty overrides, drive-relative Windows paths, filesystem roots, workspace ancestors, files, symlinks and Windows reparse points are refused by the automatic-output resolver. Packaging/archive/source-map commands retain their explicit caller-selected paths and source-independent entry point; they do not discover a Cargo workspace or acquire these automatic paths.
The architecture check compares workspace dependency edges with the context map in `docs/plan/02-architecture.md` §2, rejects unknown crates, cycles, forbidden external dependencies in core crates, `submission` dependencies on `reporting` and adapter-to-adapter edges, and allows adapter dependencies on context crates only. The reviewed pure dependency allowlist and workspace edge map live in `src/architecture.rs`. Fixture tests cover each rejection. The check uses the existing `serde_json` dependency of `xtask` (MIT OR Apache-2.0 in its crates.io package manifest; source and licence pass the existing supply-chain gates); it adds no dependency.

The requirement-tag check lists the tests with `cargo test --workspace --locked -- --list` and validates the tags in their names against the requirement table of `docs/plan/01-requirements.md` and the qualifiers of `docs/plan/04-roadmap.md`. The grammar is in `docs/plan/11-ddd-and-tdd.md` section B4.

The supply-chain tasks are specified in `docs/plan/10-supply-chain-security.md` (S-4 and its implementation note); the list format and the update procedure are in `supply-chain/README.md`.

The crate is a build tool, not a context crate: its modules are public so that the tests and `main.rs` can reach them, and the `api` / `api::model` export rule of the context crates does not apply. Its code is tested at these seams: the tag parser (`tags`), the requirement registry (`registry`, `listing`), the check plan (`check`), exact core membership and completed mutation reports (`quality`), the mapping of an error to the exit code and its report (`exit`), the lockfile reader (`lockfile`), the hash list and its comparison (`hashlist`), the guard rules over file texts (`guard`), the file collector (`sources`), the vendor tree hashing and verification against fake trees on disk (`vendor`), the negative tests that run a real Cargo against a stale `Cargo.lock` (`negative`), the justification rule for `deny.toml` suppressions (`deny`) and the suppression register rows it reads (`register`), the exemption rule for the `cargo-vet` store (`vet`), the Dependabot rules (`dependabot`), the OSV gate over recorded `osv-scanner` reports (`osv`; the reports are under `xtask/fixtures/osv/`), the notices directory (`notices`: the reviewed-list parser, the `cargo-deny` report reader, the plan of the directory with its findings, and the file scan and copy against fake trees on disk), the licence-header rule (`header`), the licence copy comparison (`licence`), the sign-off rule and the parser of `git log` (`dco`, including one test against a real `git log` in a scratch repository), the English check and its allowlist (`english`), the file listing from Git (`tracked`, with a real `git ls-files` in a scratch repository), and the negative tests that run a real `cargo-deny` with the repository's `deny.toml` against fixture projects (`denygate`; they need `cargo-deny` on the `PATH` and fail, not skip, without it).

## English check

The `english` step flags a line of a tracked UTF-8 file when it holds a German umlaut or the German sharp s or at least two distinct words of the list of German function words in `src/english.rs`. Exceptions live in `english-allowlist.toml`: one `[[exception]]` table per file or directory (a path ending in `/`), each with a `path` and a `reason` that says why the German text must stay (a verbatim third-party text, in-game text in a corpus `expected.json`, a game-language fixture, a message catalogue, the task's own source). An entry that excuses no German text fails the check, so the list shrinks with the text it excuses.

## Licence information

`reuse lint` runs in CI only (`.github/workflows/repository-gates.yml`, job `reuse`), from the hash-locked requirements files in `.github/requirements/`. To run it locally with the same version: `uvx --from "reuse[charset-normalizer]==6.2.0" reuse lint`. A file that quotes the SPDX tag names followed by a colon, in a string or a doc comment, is read by `reuse` as that file's own licence information; the `header` module builds the tags with `concat!` for that reason.

## cargo-deny

`check` and the `denygate` tests need the `cargo-deny` binary at the version pinned in `docs/dependency-pins.md` (0.20.2). CI installs the release binary and verifies its SHA-256. Locally, download the archive for your system from the `EmbarkStudios/cargo-deny` release, compare its SHA-256 with the row in `docs/dependency-pins.md`, and put the binary on the `PATH`; `cargo install --locked --version =0.20.2 cargo-deny` also works.

The configuration is `deny.toml` at the repository root. It checks licences, sources and bans (wildcards and duplicate versions); its `[advisories]` section is checked by CI (`.github/workflows/advisories.yml`), not by `check`, because it fetches the RustSec database. A `skip`, `skip-tree`, `ignore`, licence `exceptions` or `clarify` entry needs a `reason` where the key has one and a row in the suppression register (`docs/plan/09-engineering-principles.md` §3a) that names `deny.toml` and the entry in backticks and ends with a revisit date; `check` fails otherwise.

## Dependencies

`xtask` has three direct dependencies, declared once in `[workspace.dependencies]` of the root `Cargo.toml` (10 S-2). `xtask` itself is build tooling; these libraries may also be used independently by shipped workspace crates. Corrected 2026-11-01: the earlier blanket claim of no assembler was inaccurate. Rust sources in `sha2`, `cpufeatures` and `zmij` contain literal assembly indicators; the exact native-assembly inventory and owner approval boundary below apply. The versions below match the frozen issue #80 lockfile.

| Crate | Version | Purpose | Licence (SPDX) | Source | Build script | Procedural macro |
|---|---|---|---|---|---|---|
| `serde_json` | 1.0.154 | Reads the JSON that `osv-scanner` and `cargo metadata` print (the OSV gate, 10 S-19) | `MIT OR Apache-2.0` | crates.io, `github.com/serde-rs/json` | yes (probes the target's pointer width) | no |
| `sha2` | 0.11.1 | SHA-256 of the vendored files | `MIT OR Apache-2.0` | crates.io, `github.com/RustCrypto/hashes` | no | no |
| `toml` | 1.1.8+spec-1.1.0 | Reads `Cargo.lock`, manifests, Cargo configuration and the toolchain pin | `MIT OR Apache-2.0` | crates.io, `github.com/toml-rs/toml` | no | no |

Compiled transitive crates: `itoa` (`MIT OR Apache-2.0`), `memchr` (`Unlicense OR MIT`, used under MIT), `serde` (build script that probes `rustc`) and `zmij` (`MIT`, build script that probes `rustc` and the optimisation level) for `serde_json`; `cfg-if`, `cpufeatures` (and `libc` on Linux, with a build script that only probes `rustc`), `digest`, `block-buffer`, `crypto-common`, `hybrid-array`, `typenum` for `sha2`; `serde_core` (build script that probes `rustc` and writes a generated module), `serde_spanned`, `toml_datetime`, `toml_parser`, `winnow` for `toml`. All are `MIT OR Apache-2.0` except `winnow` and `zmij` (`MIT`) and `memchr` (`Unlicense OR MIT`). `serde_derive`, `proc-macro2`, `quote`, `syn`, `unicode-ident`, `indexmap`, `hashbrown`, `equivalent` and `toml_writer` are in `Cargo.lock` and vendored but not compiled.

## Core quality tools

`check` runs `cargo llvm-cov --locked --fail-under-lines 85` for the present core packages: `tallyline-shared-kernel`, `tallyline-game`, `tallyline-reference-data`, `tallyline-capture`, `tallyline-recognition`, `tallyline-reporting`, `tallyline-submission` and `tallyline-workflows`. This is the same exact membership as the architecture guard. Adapters, UI, design-system and tools have no coverage quota. Malformed Cargo metadata fails rather than becoming an empty selection.

Install the exact quality-tool versions and verify the archive digests in `docs/dependency-pins.md`; `llvm-tools-preview` must belong to the repository's pinned Rust toolchain. CI checks the binary versions and archive hashes before use. No toolchain override is required. `cargo xtask mutation --in-diff <diff>` consumes a Git unified diff; without the option it evaluates the full core. `--shard <k/n>` partitions the same selection before execution; both options may appear in either order and malformed or repeated options fail. The selected list is retained even for explicit empty skips. The report stays under `target/core-mutation/` and is not committed. CI materialises Git LFS payloads before mutation: the tool's core-only baseline does not exercise the workspace adapters built for actual variants. JSON/text reports, complete and per-shard selections, source/run/shard identities and build logs are retained as distinct seven-day artifacts even after failure. The aggregate downloads the exact attempt with the pinned action and rejects digest mismatches. A complete unviable-only shard contributes its validated unviable count without a score; only the full selection can establish a measured score, and an all-unviable whole run fails.

Corrected 2026-11-01: `tallyline-workflows` is the first present core crate. Its full Windows measurement at integration commit `5b1895690ee8462359d9519d90ef27583ead0f95` completed with eight mutants: four caught, four unviable, none missed or timed out. The score is 100% and the non-decreasing floor is frozen at that value. Core line coverage is 33/33 lines (100%). The initial run timed out while running the larger workspace suite; its result was rejected and is not the baseline. The successful repeat used a 120-second minimum test timeout, now passed explicitly by `xtask`; timeouts still fail the report gate. The pinned tool's baseline tests only mutated packages even when mutants run all workspace tests ([upstream source](https://github.com/sourcefrog/cargo-mutants/blob/v27.1.0/src/lab.rs)). Linux and hosted measurements of this core remain pending.

Corrected 2026-11-05: mutation subprocesses override inherited build settings with `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and `CARGO_INCREMENTAL=0`. Copied-root target isolation and ordinary command settings stay unchanged. Debug assertions, overflow checks and default optimisation remain enabled as before; debugger/backtrace detail is reduced and repeated compilation may take longer. The focused `tests/mutation_build_storage.rs` contract exercises public `cargo xtask mutation` using two real Cargo probes plus a CLI harness built from the actual entry point and library, observing real compiler arguments and both named tests under hostile inherited settings. It needs pinned `cargo-mutants` on `PATH`; ordinary CI and mutation jobs call `cargo xtask install-cargo-mutants` with the existing platform archive digests and version check; its ordinary Cargo control retains the inherited settings. This fixture adds workspace-test overhead and proves neither disk capacity nor completion within the unchanged hosted 120-minute limit. The [failed hosted run and replacement-measurement boundary](../docs/mutation-evidence.md#mutation-build-storage) remain explicit.
## Mutation cause admission

Corrected 2026-11-06: CI uses 32 fixed round-robin partitions (`0/32` through `31/32`) with matching owned output paths, artifact identities and aggregate admission. The [capacity checkpoint](../docs/mutation-evidence.md#mutation-campaign-capacity) records the failed eight-way measurement and the qualified estimate behind this candidate. The existing genuine compiler/workspace contract also invokes the public aggregate CLI over 32 explicitly constructed partitions containing 33 unique fixture identities, then rejects incomplete or foreign evidence. Constructed reports use the genuine campaign's report shape and cause log; they are not 32 executed campaigns or 33 measured mutants. The contract reuses its already-built CLI harness without additional compiler or mutation subprocesses beyond aggregate calls. Failed fixture evidence is retained. This verifies aggregate evidence semantics, not external sharder behaviour or hosted capacity.

Both mutation execution and aggregation require bounded retained cause logs before scoring. Named failed tests and coded Rust diagnostics remain separate; known executable-busy, disk, linker and crate-artefact failures invalidate the measurement. The pinned explicit empty-listing protocol is retained as a scoreless skip. See [mutation evidence admission](../docs/mutation-evidence.md) for the exact boundary and the withdrawn historical full-run claims.

## Approved native-scope restrictions

Status 2026-11-01: `cargo xtask check` reads full `cargo metadata --locked --format-version 1 --filter-platform <target>` for both `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`, before the other quality checks. It follows `resolve.nodes[].deps[].pkg` from every `workspace_members` root, including renamed dependencies, rather than treating every entry in `packages` as active. Missing or malformed reachable graph data fails closed. Active features come from the resolved node, not the package's feature declarations.

The owner-approved restrictions are: `wayland-backend` 0.3.17 must keep feature `log` disabled (its C shims); `android-activity` 0.6.1 and `objc-sys` 0.3.5 must be absent from both supported resolved target graphs. The same reachability guard also excludes `android-properties` on both supported targets: 0.2.2 has unsafe Android property-buffer/callback defects and is covered only as off-target code, not certified as sound. The Apple-only `core-graphics` and `dispatch` packages are also excluded at every version because their unsafe FFI code lacks deployable audit coverage for this graph. These are Rust FFI exclusions, not new native-code exceptions. The guard recognises package names across version changes, so a version bump cannot bypass these restrictions. Later native activation requires new owner approval and a corresponding guard and register change. Packages retained only in lockfile metadata are allowed. Public metadata fixtures cover activation, off-target leakage, excluded packages, renamed and transitive dependencies, and malformed inputs.

The same guard excludes every `send_wrapper` version from both supported graphs. Its optional Future/Stream Pin projection and wrong-thread Drop/leak interaction has an unresolved source-review concern. No source-soundness audit or publisher trust follows from retaining it only in off-target metadata. Activation requires a focussed source review and owner approval; the fixture covers a future version and a renamed transitive dependency from an alternative workspace root.

This guard enforces these explicit package and feature restrictions only. It does not complete the general native-code inventory or the reviewed build-script and procedural-macro list of 10 S-12. A `links` key alone is insufficient to classify native code: `rayon-core` 1.13.0 (`rayon-core`) and `wasm-bindgen-shared` 0.2.130 (`wasm_bindgen`) use pure-Rust coordination markers. Metadata cannot establish the contents or behaviour of arbitrary build scripts; source review remains necessary (09 §3b, 10 S-12).


## Property-testing profile preparation

The `native scope` step also checks the property-testing dependency boundary before workspace compilation. The exact numeric property-testing profile is adopted in `supply-chain/property-testing.json`, with the owner's 2026-11-03 chat approval and proposal digest. Missing/proposed records, changed versions, archives, source trees, features, targets or introductions fail closed. Only the existing getrandom 0.4.3 std,sys_rng test-feature extension is authorised; native source, backend and build-script identities remain unchanged.

The public seam is `property_scope::violations(metadata, lock, review, target, source_trees)`. The adopted record binds exact package versions, crates.io archive checksums, complete resolved source-tree hashes, feature sets, expiry and both supported x64 targets. Source collection reuses the vendor tree hash algorithm and the build-code link/reparse-point rejection. Cargo extraction markers count as source-tree inputs; `.cargo-checksum.json` does not. Switching from cached source to vendor source therefore requires reconciled source-tree evidence. First-party test files are not approval-hash-pinned.

The sole external introduction is one unrenamed `tallyline-recognition` dev edge to proptest. Normal/build, conditional, renamed and other-root introductions fail. Internal normal edges and the existing getrandom 0.4.3 to rand_core edge are allowed under exact reviewed identities/features. The latter is a metadata-union exception, not evidence that rand_core ships. Unrelated older rand/rand_core versions are not banned. Separate full test-tree projection checks exact activation/features, and normal/build-tree projection rejects the testing packages. Both trees are collected by the check; a supplied unfiltered metadata union alone never proves release exclusion.

`cargo xtask property-scope <metadata.json> <Cargo.lock> <target> [<approved.json>]` inspects supplied evidence without fetching, compiling or executing its dependencies. It checks conservative scope only; its success is not target-tree, release or runtime acceptance. Use an already built xtask from its approved baseline graph when inspecting a proposed dependency graph. The automatic `check` path additionally collects target trees before build/test steps.

Fixtures exercise exact synthetic approval, absent/proposed approval, changed archives/source bytes/versions/features, normal/build and renamed leakage from alternative workspace roots, malformed graphs, unsupported targets, source collection, and dev-free versus leaked normal/build trees. The current reviewed runner source supplies its complete fixed configuration and exact 16-byte XorShift seed. Struct update syntax would inherit future feature-dependent fields; exact dependency/source/feature checks reject profile expansion. Corrected 2026-11-03: the owner granted the exact profile, the guard is adopted, and the process-isolated hostile-environment contract verifies effective configuration, actual case execution and identical generated values. The original read-only source review itself executed no dependency code or tests. These guards do not prove arbitrary first-party test behaviour, matching-host compiled cfg, release units/SBOM content, native runtime acceptance or numerical precision.

## Reviewed build-time code

`cargo xtask build-code` and the `build-time code` step of `check` compare full, unfiltered `cargo metadata --locked --offline --format-version 1` with `supply-chain/build-code.json`. Every registry custom-build/proc-macro target needs an exact name, version, kind, published archive checksum, entrypoint SHA-256, behaviour and review scope. A workspace-owned custom-build script instead requires an explicit `workspace` identity with normal workspace-relative `manifest_path` and `source_path`, exact locked name/version and actual workspace membership; it has no fabricated archive checksum. Missing/stale/duplicate entries, changed source bytes or source identity, incomplete metadata and unknown graph references fail closed. Cargo's cached sources must already exist; the command never fetches them. It does not run build scripts, procedural macros or notices generation.

The public fixture seam is `build_code::violations(metadata, lock, reviewed, source_hashes)`. Fixtures cover accepted input, missing/stale review, changed version/kind/source/checksum/code, malformed graph data and the GL consumer boundary. Hash collection reads regular package entrypoint files and rejects paths outside the manifest directory. The inventory is a reviewed behaviour list; it does not claim a new full-crate audit of every dependency. Existing macro audit coverage and the accepted AWS-LC native-source exception remain separate and explicitly qualified in the registry.

The additional `gl_generator` context pins 0.14.0 and the exact three reviewed reverse consumers, glutin_egl_sys 0.7.1, glutin_glx_sys 0.6.1 and glutin_wgl_sys 0.6.1, including archive and build-script hashes. Their source selects StructGenerator, StaticGenerator or iOS StaticStructGenerator. A new consumer, generator version or different source requires review. GlobalGenerator has safe unsynchronised mutable globals and DebugStructGenerator logs arguments; both are excluded from this current-project assessment. The gate supports the expressly scoped cargo-vet notes, not an unrestricted soundness assertion or new unsafe/native usage.


## Literal inline assembly inventory

Owner approval recorded 2026-11-01 covers exactly 19 versions of 14 crates and 98 indicator-file hashes in `supply-chain/native-assembly.json`, with the recorded feature/target bounds and revisit by 2027-04-13. The two additional portable-atomic files contain comment-only indicators within the same approved archive. New versions, features, backend selections or platforms need fresh review and approval. This approval records the native scope; it does not claim a human source audit or complete unsafe-code audit.

`cargo xtask native-assembly [<approved.json>]` checks literal `asm!`, `global_asm!` and `llvm_asm!` indicators in registry Rust sources against `supply-chain/native-assembly.json` (ADR-0005 D1, 09 §3b). `check` runs it before test/build work. The registry must explicitly say `approved`; proposed review data stays outside the repository until the owner approves it. Each review binds the exact package version, locked archive checksum, matching source paths/hashes, full-resolution features and package-ID reachability/features on both supported targets. Reaching the recorded revisit date fails closed.

Collection rejects symlinks and path escape. This conservative lexical scan includes comments and strings, and accepts whitespace before the exclamation mark. It does not expand macros, evaluate every cfg, prove arbitrary native-code absence or verify non-matching cached selector files. An exact reviewed hash can account for a comment/string indicator; new indicators and stale records fail. S-4 archive/vendor checks, S-12 build-code reviews, licensing and cargo-vet remain independent gates. The inventory adds no registry dependencies.


Exact canonical metadata lines may add an optional `line` string to an
exception for one file. Matching preserves every character, including spaces;
other lines in that file remain checked. Distinct exact lines may share a
path; duplicate or redundant whole-file/line entries fail. A changed or absent
line makes the exception stale. The two generated cargo-vet publisher-name
comments use this boundary; audit/trust notes retain the English rule. Unicode
escapes in the TOML configuration encode the original names without adding a
self-exemption for the allowlist file.

Target evidence added 2026-11-02: existing `targets` selectors still bind the
conservative target-filtered metadata, unchanged. An optional `resolved_targets`
object adds a second, independent exact activation/feature selector for every
supported target: `null` means absent and an array means present with exactly
those features. It cannot waive the original metadata, archive, source or
full-feature bindings. Missing evidence or malformed selectors fail closed.

The CLI collects the complete workspace tree directly with locked/offline Cargo,
normal/build/dev edges, depth prefixes and `{p}|{f}` formatting. The reviewed
Cargo build is exactly `cargo 1.99.0 (5f94df478 2026-08-27)`; its source at
[commit 5f94df478](https://github.com/rust-lang/cargo/tree/5f94df478/src/ops/cargo_tree)
uses the target feature resolver, filters inactive platform/optional edges and
includes host build/procedural-macro dependencies. Collection records that
Cargo build, the observed Cargo host and the requested target. The parser checks
every workspace root, package identity, edge, feature, depth and duplicate
reference against full metadata; unknown or ambiguous identities fail closed.
Missing/empty output is never evidence of exclusion. Host and target occurrences
are conservatively unioned. A Linux target tree collected on Windows therefore
includes Windows host units and is not evidence of a matching-host Linux build.

This distinguishes inactive optional feature-union edges without silently
redefining existing approvals. Baseline tree features differ from metadata for
Windows getrandom 0.3.4 and for half 2.7.1 and zlib-rs 0.6.8 on both targets; their
existing metadata selectors remain exact. Cargo documents the tree as an
approximation of compilation, not a unit-plan or emitted-code proof. The
source-bound collection is an additional activation guard; matching-host builds
and practical native acceptance remain separate requirements. This prerequisite
adds no credential dependency, native approval, trust or vet exemption.

## Approved SQLite source scope

`cargo xtask sqlite-native` checks `supply-chain/sqlite-native.json`: exact
locked wrapper/sys identities, complete bundled source and build-script
hashes, full plus both supported target feature/reachability graphs, expiry
and rejected compiler/native overrides. Explicit Cargo env/target Rust flags
are checked as well as ambient inputs; MSVC CL/_CL_, CFLAGS and forged Rust
feature cfgs cannot silently activate the C timer selectors. Real source
collection rejects missing files, symlinks and escaping paths. The check is
second in the aggregate plan, after the supply-chain guard, and stops the
plan before workspace build/test work on failure. Its pure metadata/configuration
and real-file fixtures do not compile native code. The independent existing
Rust literal assembly inventory retains its exact approved set.

The 2026-11-01 reference-data extension also rejects every reachable version
of system-configuration and system-configuration-sys on either supported
target, including renamed dependency edges. Both retain justified off-target
S-6 exemptions with their concrete review limitations. Native-scope and
build-code failures stop the aggregate check before workspace build/test work;
no feature or package is removed from audit input to obtain a pass.

Decimal runtime scope is exact: rust_decimal defaults and every optional
feature remain disabled. The same rooted metadata guard rejects any enabled
feature before workspace build/test work; the pure-runtime architecture
allowlist does not grant its optional database, JSON or other capabilities.

Git child commands remove the repository-local environment names recognised across the supported Git versions, including the legacy `GIT_INTERNAL_SUPER_PREFIX`. The real Git contract compares this removal set with `git rev-parse --local-env-vars`; an additional name fails rather than inheriting another repository context.

Workspace-script reviews remain source reviews, not archive trust or exemptions. The `workspace` object contains only `manifest_path` and `source_path`; the entrypoint must remain inside its own reviewed package. Ambiguous workspace/archive records, workspace procedural-macro records, renamed or foreign packages, missing/duplicate membership, path changes, missing or malformed hashes and malformed metadata fail closed. Source hashing requires absolute paths, a real owning workspace manifest, regular source files, canonical package containment, and no symlink/reparse point in file or ancestor paths. Registry scripts keep the existing crates.io/locked archive identity checks; the separate GL consumer restrictions remain in force. The public guard fixtures include actual changed/missing files and an observed Linux symlink-ancestor regression. No workspace build script is adopted merely by supporting this identity format, and no network, native compilation or publisher trust is authorised by it.
Credential extension 2026-11-02 adds the six exact native source packages to the
same hash/archive/feature/target inventory. Explicit credential FFI and transport
selector files are collected even without literal assembly, so dropping their
review entries fails. The original nineteen package versions remain intact;
sha2's approved Linux crypto feature addition now also has resolved selectors
that keep Windows features empty. cmov's metadata selector records the feature
union while its resolved Windows selector requires absence. The fixed native
scope guard also rejects future versions or new optional features of credential
entry points. This dependency foundation does not implement an OS-store adapter.

The resolved credential scope also rejects the off-target zbus Windows SDK helper features Win32_NetworkManagement, Win32_NetworkManagement_IpHelper and Win32_Security_Authorization on every reached windows-sys version. Their appearance only in conservative metadata is recorded separately; no target activation is permitted.

The ONNX binding gate also enforces ADR-0007 before build/test work: reachable `ort` and `ort-sys` must be exactly2.0.0-rc.14 and have the complete reviewed `load-dynamic`/API-31 feature closure. Defaults, downloading, copying, alternate providers and unknown features fail closed. Only `tallyline-adapter-ocr` may depend on `ort`, and only `ort` may depend on `ort-sys`; dependency names are resolved through package IDs so aliases cannot bypass the private adapter boundary. This gate adopts no dependency or runtime by itself.


## Prebuilt packaging foundation

Corrected 2026-11-02: the package subcommand provides version/archive/tool/source/legal verification, bounded compiled-identity receipts, staging, fixture checks and fixed native MSI/Debian adapters. Run package help for exact arguments. The same arguments work with the prebuilt helper outside a checkout and without Cargo on PATH; that route is required after token injection. Containers must be independently unpacked before scanning. Adapter availability does not establish successful final packages or installed acceptance. Full boundaries and pending evidence: [packaging spike](../docs/packaging-spike.md).


The preparation commands are package prepare windows|linux vMAJOR.MINOR.PATCH PAYLOAD TOOLS FRESH-STAGE and package verify STAGE SEAL. Package legal-verify PAYLOAD independently checks physical notice bytes, registry index and catalogue correspondence. Prepare verifies the native x64 application header and fixed baseline resource identities without running supplied application code, excludes mutable data and existing token files, and seals the currently running helper. It reads no fixture. Protected injection/backends and complete source-duty verification remain #97 work.

The strict target form package legal-verify windows|linux PAYLOAD verifies the intrinsic profiles in supply-chain/package-legal-profiles.json in addition to physical notice binding. The form without a target checks general consistency only. Source-only profiles currently contain 219 Windows and 273 Linux components; they do not complete installer acceptance. Regenerate/review current notices and profile source hashes before compiling a replacement helper after a legal/dependency change.

Secret-free package source-map PAYLOAD UNPACKED-SOURCE ARCHIVE FRESH-OUTPUT binds source evidence to the pinned deterministic tar.gz archive (tallyline/ root) and rejects mismatched source before creating output. It streams the archive hash with bounded working memory. Current registry scopes fail closed for unreviewed component layouts; complete native scopes and protected map use remain in progress.

The source.tar.gz archive profile writes all source files with data-only mode 0644. Exact reviewed native scopes now live in supply-chain/package-source-scopes.json alongside locked registry layouts. Package source-verify PAYLOAD ARCHIVE streams the actual archive and checks mapped file bytes and fixed safe metadata without executing or extracting it. Preparation validates the fixed source-capsule.tar.gz before sealing its reference in manifest schema 2; the archive is not copied to the protected stage. Diagnostic stages without a source reference cannot admit a fixture.

Package inject STAGE SEAL FIXTURE and verify-injected STAGE SEAL FIXTURE check immutable inputs before fixture access and allow exact fixture bytes only in payload/tallyline-app-token. The envelope follows the application's ASCII-graphic 1–1024-byte reader contract; actual UEX lexical-format acceptance remains open. These commands do not enable installer backends. Package source-bindings SOURCE-ROOT checks canonical legal profile inputs; the normal check runs it before quality build/test steps. Both target catalogue bytes must be freshly compared before retaining profile hashes after a dependency/legal source change.

The source-binding profile additionally includes root Cargo.toml and every actual workspace member manifest. Its compiled member inventory must match the root declaration. Regeneration independently compares current Cargo metadata with the complete bound inventory. A manifest-only feature change invalidates the binding even when Cargo.lock and both catalogue bytes remain unchanged.

## Prebuilt backend adapters

Package build STAGE SEAL FIXTURE FRESH-OUTPUT clones only verified sealed inputs, checks the clone before fixture access and invokes the fixed native backend plus plain archive writer. It does not read the separate source capsule. Actual final source-free/network-denied distribution acceptance remains pending. Package copyright PAYLOAD FRESH-OUTPUT writes preserved DEP-5 source scopes/terms; package repack-deb PREPARED-TREE FRESH-OUTPUT creates only the fixed safe Debian launcher link and rejects scripts/untrusted links. These data adapters alone do not establish approved target identity. Unit contracts use preinstalled Python 3 standard-library readers as independent parsers, not as packaged/runtime dependencies.

Package msi-probe APPROVED-TOOLS FRESH-OUTPUT builds/default-validates/decompiles fixed core prerequisite authoring and harmless text data without a fixture or app. The shared 361 CRT rows were independently matched to actual tables and evaluated in a restricted read-only Windows Installer session, with no actions, UI or registry writes. Search properties must be uppercase/public; deliberate overrides are outside the prerequisite preflight's security boundary. Final app-MSI, CRT/install/native-display acceptance remains open. Tool capture discards both output streams before persistence on any bare/wide fixture leak; persisted streams are limited to eight MiB each.

Corrected 2026-11-02: the app now produces bounded headless BuildInfo before native bootstrap, using fixed Development `0.0.0` or complete compile-time tag/commit metadata. Cargo.package.version is never a release authority. A trusted secret-free source job captures the report from its known compiled app, then runs `package identity-receipt TARGET TAG APP COMPILED-REPORT FRESH-RECEIPT`; retain its printed SHA-256 independently. `package identity-verify TARGET TAG APP RECEIPT RECEIPT-SEAL` checks canonical bounded receipt bytes, exact metadata and the executable hash without executing supplied code. Use `package prepare-admitted TARGET TAG PAYLOAD TOOLS FRESH-STAGE RECEIPT RECEIPT-SEAL` to bind that receipt in schema three; copied/staged app bytes are rechecked. Protected fixture/assembly requires both admitted identity and a source archive reference before fixture access. Ordinary preparation and legacy schema-two stages remain diagnostic. `development` and `release-metadata` receipt purposes are not authenticated release provenance: report origin, signed tag and CI authentication remain the source job/#98 responsibility. Actual final package and installed acceptance remain open.

The measured vendor identities and exact Cargo-omitted archive .gitignore files are retained in `supply-chain/property-testing-vendor-reconciliation.json`. They do not replace the enforced registry-tree hashes or authorise source rewriting. The real vendor list covers all 508 locked registry packages.

The installer reads `RUNNER_OS`, `RUNNER_ARCH`, `RUNNER_TEMP` and `GITHUB_PATH` and accepts no arguments or release overrides. Its fresh `RUNNER_TEMP/cargo-mutants` directory holds the downloaded archive, temporary process output and extracted `bin`. Download protocols are restricted to HTTPS; downloads have a 30-second connect timeout, 120-second per-attempt timeout and 64 MiB size limit. Curl retries transient failures at most three times with its default backoff, starting retries only within a 20-second window measured from the first attempt. A started transfer may outlive that window; the outer 150-second process limit bounds the whole download. Permanent HTTP errors, connection refusal, checksum failures and verification failures do not receive additional retries. Each native child has a 150-second runtime limit and separate 64 KiB stdout/stderr limits. Hash verification precedes extraction, and the expected executable must be a regular file. Only one final LF or CRLF is removed from UTF-8 version output. The archive and temporary process output are removed before one UTF-8 directory line is appended to the existing `GITHUB_PATH` file. Existing bytes are preserved; a nonempty file whose last byte is not LF receives a separating LF first. Empty files and existing LF or CRLF endings receive no extra separator. The boundary check reads only the final byte; the file must permit reading and appending. Only `bin` remains for subsequent steps. A failed cleanup is reported alongside the installation error.

The owning installer-module tests use a credential-free loopback server, independently generated platform archives and a Cargo-built Rust fixture executable through the public pipeline. Only a private `cfg(test)` descriptor factory supplies fixture inputs; production has no URL or hash override. These tests cover recovery from HTTP 503 then 504 before a verified installation, exhaustion of four HTTP 503 attempts without publication, actual curl, extraction and Cargo dispatch, digest-before-execution, download/archive/executable/version failures, output obstruction, literal paths and owned cleanup. Fixture success does not establish genuine release installation or hosted Windows/Linux acceptance. The separate real `tests/mutation_build_storage.rs` acceptance remains required.
