# Documentation

> **Doc type:** Living reference — current. Last reviewed: 2026-11-06.

`docs/` is the project's single source of truth ([`CLAUDE.md`](../CLAUDE.md), "The documentation is the project"). This file says what belongs where. A new directory or top-level file under `docs/` gets its row here in the same commit.

| Path | What belongs there |
|---|---|
| `panel-preparation.md` | Supplied geometry/privacy contracts, offline dump format, limits and automatic locate/calibration gates |
| [`scu-review.md`](scu-review.md) | Guarded manual quantity review, provisional baseline/source, tighten-only settings, durable decisions, numeric readiness and schema-eight/schema-five acceptance boundary |
| [`plan/`](plan/) | The plan, numbered 01–11: requirements (01), architecture (02), language decision ADR-0001 (03, superseded by ADR-0005), roadmap (04), bug analysis of the predecessor (05), UEX API notes (06), OCR concept (07), plan reviews (08), engineering principles (09), supply-chain security (10), DDD and TDD (11) |
| [`adr/`](adr/) | `0000-open-points.md`, the single register of open owner decisions and open verifications, and the decision records `NNNN-kebab-title.md` (ADR-0001 lives in `plan/03-language-decision.md`; ADR-0005 `0005-rust-and-egui.md` is the current implementation-language, toolkit, supply-chain and packaging decision; ADR-0006 `0006-tls-provider-aws-lc-rs.md` chooses the `rustls` crypto provider; ADR-0007 `0007-ort-rc14-and-onnx-runtime-link-mode.md` fixes the `ort` version and how the ONNX Runtime library is shipped and loaded; ADR-0008 `0008-onnx-runtime-telemetry.md` records how ONNX Runtime's telemetry is switched off and what is accepted; ADR-0009 `0009-visual-cpp-runtime-as-prerequisite.md` makes the Visual C++ runtime of ONNX Runtime a prerequisite instead of shipping it; ADR-0010 `0010-release-token-delivery.md` records release token delivery and replacement) |
| [`agents/`](agents/) | Per-repository configuration for the engineering agent skills: where issues are tracked (GitHub, writes need approval), the triage label vocabulary and how the domain documents are read |
| [`prompts/`](prompts/) | Prompts for AI tools, each with its run status |
| [`design-system/`](design-system/) | The design system Tallyline as the repository holds it: token documentation, states, accessibility report; the tokens themselves are in `ui/design-tokens/`; the JavaFX parts are historical since ADR-0005, and the recreated egui crate lives in `crates/egui/` (O-112) |
| [`images/fankit/`](images/fankit/) | The Star Citizen Fan Kit logo files only; they are not under the project licence (`NOTICE` §4, `REUSE.toml`) |
| [`user/`](user/) | The user guide (R-DOC-1) and contributor corpus capture protocol; its images are redacted screenshots only |
| [`release-process.md`](release-process.md) | Versioning, release checklist, upgrade and downgrade, Star Citizen patch-day and UEX API-change procedures |
| [`game-log-validation.md`](game-log-validation.md) | Completed initial R-OCR-20 Game.log spike (issue #107): validated build/format and finite mapping allowlist, measured capture offsets, synthetic specimens and explicit unsupported scope; initial #108 parser/privacy scope, local header check and later integration obligations |
| [`dependency-pins.md`](dependency-pins.md) | The tags of SHA-pinned GitHub Actions and the versions and checksums of CI tools (including the pinned WiX and `cargo-packager`) |
| [`ci-release-reference.md`](ci-release-reference.md) | Reviewed P4KX build/release patterns, further M1–M5 applications, adaptation boundaries and upstream feedback |
| [`prebuilt-package-verification.md`](prebuilt-package-verification.md) | Historical Development installer/archive inspections and metadata corrections, with current-source, isolation and installation acceptance limits |
| [`ocr-loader-fixture-failure.md`](ocr-loader-fixture-failure.md) | Native-free OCR loader fixture correction, reproduced writable-executable mechanism and historical mutation-evidence limits |
| [`mutation-evidence.md`](mutation-evidence.md) | Retained cause-log admission, withdrawn infrastructure-contaminated full-run scores, exact empty-selection handling and replacement-measurement limits |
| [`public-api-contract-drift.md`](public-api-contract-drift.md) | Fixed credential-free UEX field/type/code sampling, reviewed metadata baseline and opt-in main-only weekly CI; hosted activation remains pending |
| [`diagnostics-export.md`](diagnostics-export.md) | Masked immutable support previews, explicit acknowledgement, bounded fixed-name log tails and exclusive private JSON file contracts; desktop/logging composition remains pending |
| [`local-ocr-nine-pair-experiment.md`](local-ocr-nine-pair-experiment.md) | Completed Windows PP-OCRv6 Tiny/Small/Medium nine-pair raw-reader experiment: lifecycle compatibility, literal development discrepancies and bounded process resource observations; no production profile selection |
| [`local-ocr-tesseract-experiment.md`](local-ocr-tesseract-experiment.md) | Completed approved Linux Tesseract best/fast PSM 11 pilot: twenty fixed conditions, literal complementary/error examples and bounded worker resource observations; no product backend selection |
| [`local-ocr-joint-reader-comparison.md`](local-ocr-joint-reader-comparison.md) | Reconciled eleven-profile PP-OCR/Tesseract literal inventory over 63 development fields and all 55 pairs, with frozen references, shared-error examples and explicit accuracy/independence limits |
| [`local-reader-evaluation-next.md`](local-reader-evaluation-next.md) | Provisional next local-reader comparison and delivery-proof plan; no runtime or production profile selection and no new measurements |
| [`local-reader-verified-reassessment.md`](local-reader-verified-reassessment.md) | Post-hoc replay of retained OCR/VLM outputs against owner-verified values: all 55 OCR pairs, 165 triples, omissions, shared wrong answers and explicit coverage/acceptance limits |
| [`local-vision-runtime-research.md`](local-vision-runtime-research.md) | Source-backed comparison of bundled local VLM runtimes, complementary OCR readers, hardware tiers and evaluation plans; original research scope and later experimental evidence are distinguished |
| [`research/ollama-v0.40.13/`](research/ollama-v0.40.13/ollama-v0.40.13-cgo-sqlite-notice-delta-v01.md) | Historical partial-archive, source, module and notice evidence for the conditional Ollama candidate; authenticity, native scope, complete notices and execution/delivery admission remain unproved |
| [Ollama authenticity follow-up](research/ollama-v0.40.13/ollama-v0.40.13-authenticity-follow-up-v01.md) | Historical official post-signing path/hash matches, bounded build observations, attestation retrieval limits and a synthetic trial capsule checklist; execution/delivery admission remains unresolved |
| [`local-vision-first-campaign.md`](local-vision-first-campaign.md) | Completed 310-response local VLM experiment: exact inputs, structural coverage failures, separately labelled exploratory discrepancies and evidence limits; no validated accuracy or backend selection |
| [`local-vision-small-model-campaign.md`](local-vision-small-model-campaign.md) | Completed author-bound 2B CPU/GPU smoke and a separate literal-field development condition, with raw-value discrepancies, structural failures and independent cleanup evidence |
| [`local-vision-ministral3b-experiment.md`](local-vision-ministral3b-experiment.md) | Completed author-bound Ministral 3 3B development smoke: fixed-parser field concordance, paired 2B comparison, shared tab error, geometry and independent cleanup evidence |
| [`adr/0012-strictly-local-recognition-delivery.md`](adr/0012-strictly-local-recognition-delivery.md) | Owner-required local-only inference and recognition evaluation; its complete-initial-profile delivery promise is qualified by ADR-0013; engine choice and the separate Windows OCR prerequisite remain open |
| [`adr/0013-conditional-ollama-delivery.md`](adr/0013-conditional-ollama-delivery.md) | Conditional local Ollama and explicitly requested first/additional model downloads if custom integration becomes excessive; user installation only if bundled delivery cannot be implemented acceptably; no engine or payload selected |
| [`cla-signatures.md`](cla-signatures.md) | The public roster of CLA signatures |

Diagrams are Mermaid or plain-text trees, never binary files. Images in `docs/` are only redacted user-guide screenshots and the Fan Kit logo files.


The [packaging spike](packaging-spike.md) records the source-independent helper foundation, exact tool verification and remaining protected/installed acceptance. [ADR-0011](adr/0011-source-compiled-wix-packaging.md) supersedes ADR-0005 D7 only; historical WiX 3 descriptions are superseded by its dated correction.

- [Linux Tesseract research controller](tesseract-linux-research-controller.md): exact local research scope, safe Rust lifecycle, native/source/licence gates and honest measurement limits.

The [SCU policy evidence](scu-policy-calibration.md) records exact explicit-band application assessment, the available corpus sensitivity and the broader calibration and native visual acceptance boundary.

The explicit `scu-policy --public <directory>` developer command analyses complete whole-SCU transcriptions without images or recognition. Its synthetic counts remain separate from golden accuracy and verification flags; [the calibration note](scu-policy-calibration.md) records the provisional starting-pair recommendation and limits.


The [durable manual submission queue](submission-queue.md) records fixed release identity, actual refresh/send gates, transactional recovery/outbox delivery, schema 10/settings 6 and the remaining acceptance boundary.

The [local metadata transport contract](local-metadata-contract.md) records the fixed credential-free version route, generation-bound actual-socket gate, explicit bounds and the distinction between harmless transport tests and actual OS/runtime admission.

- [Owned-process contract](owned-process-contract.md): Windows supplied-command job/pipe lifetime, explicit cleanup observations and native acceptance limits.
