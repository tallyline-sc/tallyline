# crates

The workspace crates of Tallyline: the core (`shared-kernel`, the context crates, `workflows`), the adapters, `ui` and `app` of `docs/plan/02-architecture.md` §2. Each directory is one crate; its package name is the directory name with the prefix `tallyline-` (O-122), and its purpose is the crate-level `//!` documentation of its `lib.rs`. A crate is created with its first test (04 M0). `xtask/` and `tools/` are not here.

| Directory | Package | State |
|---|---|---|
| `adapter-uex/` | `tallyline-adapter-uex` | the read-only UEX client of the M0 API spike and its CLI `tallyline-uex-spike` (issue #24) |
| `egui/` | `tallyline-egui` | Tallyline theme tokens and reusable widgets |
| `workflows/` | `tallyline-workflows` | application preferences, save-before-apply workflows and restart-safe daily release policy |
| `adapter-files/` | `tallyline-adapter-files` | versioned atomic settings, bounded read-only game witness parsing and installed/portable directory self-tests and explicit private developer artifact writes |
| `ui/` | `tallyline-ui` | owned shell/manual Draft state, keyboard entry, message catalogue, legal unit and user-driven dialogs |
| `app/` | `tallyline-app` | native `tallyline` composition root and bounded settings/reference/manual/release workers |
| `adapter-platform/` | `tallyline-adapter-platform` | two-lock writer election, bounded local activation, credential-free release metadata and bounded packaged-token reading; the three approved Windows FFI sites |
| `shared-kernel/` | `tallyline-shared-kernel` | published IDs, exact price/SCU values, precise timestamps and the injected clock port |
| `reference-data/` | `tallyline-reference-data` | immutable environment snapshots, cached vocabulary, versions/statuses and per-endpoint freshness |
| `adapter-storage/` | `tallyline-adapter-storage` | shared schema-versioned SQLite reference cache and manual Draft repository, atomic indexed replacements and consistent marked backups |
| `game/` | `tallyline-game` | read-only witness port, typed facts and durable opt-in workflow |
| `reporting/` | `tallyline-reporting` | manual Draft commands, exact user fields and the durable Report repository port; shared validation and release remain pending |
| `submission/` | `tallyline-submission` | credential validation and secure-store ports, save-before-publish account state, revalidation and session lifetime; native stores and desktop integration pending |
| `adapter-ocr/` | `tallyline-adapter-ocr` | bounded reusable tensors, class-indexed CTC evidence, fixed asset/native guards, private Windows loading, synthetic tensor diagnostics and a portable raw-image/textline reader with lazy reused sessions and explicit idle cleanup; production scheduling/pipeline and Linux acceptance pending |
