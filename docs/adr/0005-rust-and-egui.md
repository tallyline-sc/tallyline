# ADR-0005: Implementation in Rust with egui, native code only by registered exception

> **Doc type:** Decision record — Accepted. Last reviewed: 2026-10-12.

- **Status:** Accepted (2026-10-12); D7 superseded by [ADR-0011](0011-source-compiled-wix-packaging.md) (2026-11-01). Amended by [ADR-0007](0007-ort-rc14-and-onnx-runtime-link-mode.md) (2026-10-14): the `ort` version in D2 is `2.0.0-rc.14`
- **Decider:** @greluc
- **Supersedes:** [ADR-0001](../plan/03-language-decision.md) (Java 27 + JavaFX 27); the toolkit and build parts of [ADR-0004](0004-design-system.md) (§3, §4, §6, §7 and the JavaFX spikes)
- **Amends:** the technology terms of [ADR-0002](0002-modules-by-bounded-context.md): "Gradle/JPMS module" reads as "Cargo workspace crate", `internal` package as `pub(crate)` module; the context cut itself is unchanged
- **Related:** open points O-108 to O-125 in [the register](0000-open-points.md), [01](../plan/01-requirements.md) (R-UI-17, R-NF-1, R-NF-3, R-SEC-8), [02](../plan/02-architecture.md) §2, §7, §8, §10, [09](../plan/09-engineering-principles.md), [10](../plan/10-supply-chain-security.md), [`CLAUDE.md`](../../CLAUDE.md)

## Context

ADR-0001 chose Java 27 and JavaFX 27 on 2026-10-08, with Rust scoring lower mainly on the review UI (K1, editable tables) and on the ONNX binding (K2, `ort` only as a release candidate). Since then:

- the packaging plan in 02 §8 grew a jlink image with a merged ONNX Runtime module, JavaFX natives copied into the image, FFM adapters, a pinned WiX major version and a quarterly JDK security release duty;
- the toolchain check for JDK 27 (Error Prone, NullAway, jlink plugin, PIT, JaCoCo) was still open (O-9);
- the owner decided that accessibility beyond contrast, keyboard operation, text size and "colour is never the only signal" is secondary;
- the Tallyline design system was delivered as an egui rendition (crate `tallyline-egui`, egui 0.36.2).

A review of the Rust ecosystem on 2026-10-12 (primary pages of rten, tract, ocrs, ort, egui, AccessKit, Turso, rustls-rustcrypto, cargo-packager; read in the session, not exhaustive) found no reason to keep Java: the decisive Rust gaps are the ONNX binding, SQLite and TLS, and all three have a mature solution that contains native code.

## Decisions

**D1 Language rule ("variant B").** Our own code is Rust only. A dependency must be pure Rust when a mature pure-Rust crate exists; "mature" means widely used and maintained (the owner chose the looser yardstick: a pre-1.0 crate in wide use counts; a release candidate of a native binding does not make it pure). Native code (C, C++, assembler) in a dependency is allowed only as a registered exception (09 §3b: crate, native part, reason, licence, revisit date). Platform FFI of our own sits in one adapter crate; every other crate has `#![forbid(unsafe_code)]`.

**D2 Initial exceptions.**

| Crate | Native part | Reason |
|---|---|---|
| `ort` (ONNX Runtime) | ONNX Runtime, C++ | PP-OCRv6 needs a runtime that is proven on the models (K2). Only a release candidate exists (`2.0.0-rc.13`): an explicit exception to "stable releases only". How the library reaches the build (downloaded by the crate or loaded at run time) is decided in M0 so that it is pinned by SHA-256 like the OCR assets (10 S-22) |
| `rusqlite` with bundled SQLite | SQLite, C | Turso Database is not production-ready by its own FAQ; revisit in a year |
| TLS provider of `rustls`: `ring` or `aws-lc-rs` | C and assembler | `rustls-rustcrypto` is marked "do not use in production". The licence of the bundled native parts is checked against the licence policy before the choice (O-110) |

**D3 UI: egui/eframe 0.36 with the wgpu backend**, native Wayland and X11 through winit. The review table, in-cell editing, the searchable combo box and keyboard navigation are our own work on top of `egui_extras` and the Tallyline widgets. Views are functions of an owned state value, commands go through channels; the MVVM layer of 02 §7 becomes "state + `update`" per view.

**D4 Concurrency and HTTP: tokio and `reqwest`** (rustls). CPU-heavy work (OCR, image decoding, hashing) runs on bounded dedicated thread pools, never on the async runtime; the pool bounds of 02 §7 and R-NF-3 stay. Log context is carried by `tracing` spans, with a masking layer.

**D5 Accessibility (R-UI-17).** Kept: contrast in every theme and the high-contrast theme, keyboard operation, text size and HiDPI, colour never the only signal. The screen-reader requirement becomes a Could (`R-UI-17a`, priority C) without a gate, spike or milestone. Widgets still set accessible labels where egui allows it.

**D6 Design system.** Tallyline continues as the design system; its egui rendition replaces the JavaFX CSS handoff of ADR-0004. What of the crate enters the repository, and under which conditions, is O-112.

**D7 Packaging: `cargo-packager`** with a pre-installed WiX 3.11.2 pinned by SHA-256 in `docs/dependency-pins.md` (the packager would otherwise download it; its NSIS download is checksummed with SHA-1 only, and a plugin not at all). No NSIS, no licence dialog. `.deb` and archive formats are verified in an M1 spike (DEP-5 `copyright` file, plain ZIP and `tar.gz` built by `xtask`). O-11 is largely moot (WiX 3 has no maintenance-fee EULA); O-12 shrinks to the ONNX Runtime DLLs.

**D8 Supply chain (10).** `Cargo.lock` with `--locked`, `cargo-deny` (licences, sources, bans, duplicates), `cargo-audit`/OSV, `cargo-vet`, and `cargo vendor` with hashes in the repository. crates.io only, no git dependencies, no build-time downloads that are not pinned.

**D9 Licence policy.** The allowlist in `CLAUDE.md` gains Unicode-3.0, Zlib, BSL-1.0, CC0-1.0 and `Apache-2.0 WITH LLVM-exception`; `cargo-deny` replaces the Licensee gate.

**D10 Toolchain.** Stable Rust, edition 2024, pinned in `rust-toolchain.toml`, raised in its own pull requests roughly every second or third release. O-9 (JDK 27 or 25) is moot.

**D11 Code comments.** `///` and `//!` rustdoc stay; `#[expect(lint, reason = "…")]` and `#[allow(lint, reason = "…")]` may carry one short sentence (exception recorded in the owner's global `CLAUDE.md` on 2026-10-12); a plain `//` stays forbidden. `#![deny(missing_docs)]` in every crate.

**D12 Build tooling in Rust.** Repository checks and generators (English check, header check, token and icon generation, architecture rules over `cargo metadata`) are an `xtask` crate. GitHub Actions YAML and installer configuration are the only non-Rust sources.

**D13 Architecture enforcement.** Crate dependencies replace JPMS; Clippy `disallowed_types` and `disallowed_methods` plus `cargo-deny` `bans` replace most ArchUnit rules; an `xtask` test over `cargo metadata` checks the context map and adapter leaves. Test tools: `proptest`, `cargo-mutants`, `cargo-llvm-cov`, `wiremock`, and an egui UI test harness (candidate `egui_kittest`, unverified, O-114). Candidate crates not named here are chosen and verified at source in M0.

**D14 Documents.** The conversion lands as one pull request: this ADR, `CLAUDE.md`, 01, 02, 04, 09, 10, 11, `release-process.md`, `README.md`, `CONTRIBUTING.md`, the PR template, the register and the status lines of ADR-0001, 0002 and 0004. The domain documents (05, 06, 07, 08) change only where they name Java technology; 08 is a historical record and gets a dated note.

## Alternatives considered

- **Keep Java 27 and JavaFX** (ADR-0001): rated higher on K1 and K9; costs the JDK treadmill and the packaging construction in 02 §8.
- **Strictly no native code anywhere** (variant C): would force `redb`/Turso instead of SQLite, `rustls-rustcrypto` for the channel that carries the UEX secret key, and an unproven pure-Rust ONNX path for PP-OCRv6 (`tract`, `rten` did not mention PaddleOCR, DBNet or CTC in their documentation). Rejected.
- **Pure-Rust ONNX path first** (`tract` or `rten`, `ort` as fallback): offered to the owner, who chose `ort` directly. It remains an optional optimisation if the ONNX Runtime telemetry (O-13) or the Microsoft runtime DLLs (O-12) turn out to be costly.
- **Other Rust toolkits** (slint, iced): not evaluated again; the owner named egui.

## Consequences

- Requirements: R-UI-17 is split (D5); R-NF-1 gains native Wayland, X11 stays; R-NF-3 loses heap and direct-memory limits in favour of a process-memory ceiling and the pool bounds; R-SEC-8 names ONNX Runtime and the packager instead of sqlite-jdbc and JavaFX; the coverage table in 04 follows. IDs stay; changes carry a dated "Amended 2026-10-12" note.
- ONNX Runtime telemetry (R-NF-12, O-13): the app is its own launcher, so the variable `ORT_DISABLE_TELEMETRY` is set by our code before the first ORT call (an `unsafe` call in the edition-2024 `std::env::set_var`, confined to the platform adapter and registered in 09 §3a); the jpackage launcher limitation disappears. The M0 network spike stays.
- The Windows C++ runtime (O-12): our binary can link the C runtime statically; the ONNX Runtime library still needs the Microsoft runtime DLLs.
- The Java tooling under `tools/design-system/` was removed on 2026-10-12 (the Git history keeps it); its replacement is an `xtask` task (O-112). The JavaFX handoff documents stay as historical records.
- The Fan Kit unit, the privacy gate, the crop rules, the submission lifecycle and all domain invariants are unchanged; the byte-exact pinning tests are rewritten for egui.

## Open points registered by this ADR

| ID | Point | Status |
|---|---|---|
| O-108 | ONNX path: `ort` directly; stable-release exception; how the library is pinned (M0) | decided 2026-10-12 (@greluc); link mode open |
| O-109 | SQLite through `rusqlite` with bundled SQLite as a registered exception | decided 2026-10-12 (@greluc) |
| O-110 | TLS provider (`ring` or `aws-lc-rs`), licence check, `rustls-platform-verifier` for the system store | open – default applied |
| O-111 | R-UI-17 split; screen reader as a Could (`R-UI-17a`) | decided 2026-10-12 (@greluc) |
| O-112 | Adopting the `tallyline-egui` crate: modified Apache-2.0 icon font (needs the owner's approval; ADR-0004 §5 chose path data), Python generators versus `xtask`, plain comments and missing docs in the delivered code, licence headers, source of truth for tokens (`ui/design-tokens/` DTCG versus the crate's `design/tokens.json`), retirement of `tools/design-system/*.java`, whether D1 to D11 of O-83 stand | decisions recorded 2026-10-12; crate recreated 2026-11-01, token reconciliation test added; dependency audits/vendor hashes and icon-font rendering acceptance remained open because the original crate/font were unavailable. Status 2026-11-03: the exact historically approved subset is restored with hash-refusal and actual atlas-glyph contracts; reproducible derivation, native acceptance and O-107 magnified legibility remain unproved |
| O-113 | wgpu backend; VRAM and GPU load next to the game measured in M4 (R-NF-3) | decided 2026-10-12 (@greluc); measurement open |
| O-114 | UI test harness for egui (candidate `egui_kittest`) and the Fan Kit pinning tests | open |
| O-115 | Native Wayland as a target (R-NF-1); clipboard monitoring stays A23 | decided 2026-10-12 (@greluc) |
| O-116 | Installer: `cargo-packager` with pre-installed pinned WiX 3.11.2; the spike for offline build, DEP-5 and archives runs in M1 | decided 2026-10-12 (@greluc); spike open |
| O-117 | Licence allowlist extended for Rust crates (D9) | decided 2026-10-12 (@greluc) |
| O-118 | Lint `reason` attributes allowed (D11) | decided 2026-10-12 (@greluc) |
| O-119 | Rust toolchain pin and update rhythm (D10); makes O-9 moot | decided 2026-10-12 (@greluc) |
| O-120 | Supply chain: lockfile, deny, audit, vet, vendor (D8) | decided 2026-10-12 (@greluc) |
| O-121 | Yardstick for "mature pure-Rust alternative" (D1) | decided 2026-10-12 (@greluc) |
| O-122 | Crate, package and binary names (waits for O-8) | open |
| O-123 | Async runtime and HTTP client: tokio and `reqwest` (D4) | decided 2026-10-12 (@greluc) |
| O-124 | Architecture enforcement by Clippy, `cargo-deny` and `xtask` (D13) | decided 2026-10-12 (@greluc) |
| O-125 | Clippy lint set: `unwrap_used` and `expect_used` denied outside tests, `clippy::pedantic` for the core crates | decided 2026-10-12 (@greluc) |
