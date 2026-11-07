# Changelog

All notable changes are recorded here.

## [Unreleased]

- Show saved lifecycle status, editability and check time for verified production rows, with automatic checks and account-throttled manual refresh. Positive decline or expiry atomically revokes only the matching accepted local prior; failed local saves retry without another HTTP read. Database schema17 requires a coordinated backup; settings remain schema6. Withdrawal and full native/real-service acceptance remain open (R-SUB-4, R-VAL-7).

- Explicitly send the ready rows of a mixed-cooldown Report as a separate released Report while preserving confirmations, observation time, screenshot evidence and optional affinity. Both replacements repeat normal send checks; original history remains intact (R-SUB-4). Database schema 15 requires a coordinated backup.

- Enter optional faction affinity from -100 to 100 in manual capture and the Report editor; an explicit zero stays distinct from an empty field. Commodity submission, queue restart and selected-row copying preserve it, while choosing another terminal clears the terminal-specific value. Database schema 14 requires a coordinated backup; settings remain schema 6 (R-SUB-13, R-NF-11).

- Wait for the exact app token's daily quota reset with visible UTC check times and durable receipt-read holds. Eligible work resumes through all current checks, while original attempts and restart review remain intact; schema 13 requires coordinated backup and settings remain schema 6 (R-SUB-2, R-SUB-11, R-API-3). A failed local save of a known quota answer keeps reads for that token held until the exact answer is saved and its own reset is due. Receipt checks stop their remaining requests and concurrent submissions for the same token as soon as quota exhaustion is known, including before the check returns and while its evidence transfers to the queue owner.

- Amend planned recognition delivery: permit conditional local Ollama and explicitly requested first/additional model downloads; user installation is a last resort if bundled delivery cannot be implemented acceptably. Local-only inference, optional AI, private child ownership and exact admission gates remain; no backend or download feature is implemented by this policy change (R-VLM-6/7/12; ADR-0013).

- Show the immutable target, row values and attempted observation date through **Inspect original attempt** for partial submissions before recording row decisions (R-SUB-9, R-UI-1:reports).

- Resolve partial submissions row by row with exact positive receipt proofs, typed failure provenance and preserved original transport history. Explicitly copy only resolved not-accepted rows into a separate unreviewed Draft; row-specific ordering and accepted priors commit atomically with history. Schema 12 requires the coordinated database/settings backup; settings remain schema 6 (R-SUB-9, R-SUB-11, R-UI-1:reports, R-VAL-7).

- Add an offline supplied-geometry panel tool with upright perspective crops, source transforms and conservative full-width header-and-upper-region masking before filtering. Dumps are bounded and exclusive; automatic locate and calibrated text/tone analysis remain open (R-OCR-2, R-CAP-7, R-SUB-7).

- Resolve unknown submissions through explicit received/not-received decisions or verified read-only evidence while keeping the original uncertain attempt. A separate new-Draft action preserves the original observation and values for complete review; schema 11 requires the coordinated upgrade backup (R-SUB-9, R-UI-1:reports, R-SUB-4).

- Record the 2026-11-04 owner verification of all seven existing public corpus transcriptions. Values, clipped-card boundaries and images are unchanged; historical experiments retain their original verification state and hashes, and recognition accuracy remains unmeasured (R-QA-1, R-QA-3).

- Save manual field commits together with explicit takeover, Add commodity and context changes so one interaction cannot lose another row's input. Preserve unrelated invalid text and bind save results to their exact request across Draft selection and retries; a refused batch saves nothing (R-MAN-2, R-MAN-3, R-UI-16).
- Add explicit keyboard-accessible reveal/hide controls and persistent credential-confusion guidance to the shared UEX setup form. Show typed account feedback with distinct glyphs and semantic theme colours while preserving token, storage and connection error actions, masked defaults and session/fallback choices. Explicit focus reveals the Settings account header, while passive feedback preserves keyboard focus and page position (R-API-4, R-NF-4, R-UI-17).
- Group existing Settings preferences into named single-column topic panels with one heading each and inline feedback. Invalid SCU saves reveal their explanation; saved values, tighten-only validation, explicit actions and busy guards are preserved (R-UI-7, R-UI-17, R-NF-5).

- Present offline open-source licences in a bounded, keyboard-selectable component browser with exact upstream SPDX declarations, attribution and all bundled licence/notice files. Share a wrapped, selectable, read-only text viewer with the full GPL dialog, including keyboard scrolling and copying at 200% scale (R-UI-17, R-UI-18).
- Style existing manual price/SCU and stock-tolerance inputs with exact-string right-aligned Mono numerals, muted aUEC/SCU, SCU and % unit suffixes and unit-bearing accessible names. Preserve explicit reference takeover, field-leaving commits and saved tolerance validation; wrap whole numeric input groups in narrow views (R-MAN-2, R-UI-7, R-UI-17).

- Restore the approved Material Symbols Rounded state icons and separately shipped, hash-checked subset, with complete Apache licence and modification notices (R-UI-4, R-UI-17, R-UI-18).
- Compose every existing page within a persistent top bar, responsive sidebar or rail, scrollable content and bottom status. Preserve setup guards, account choices, reference warnings and update actions across views, with expandable endpoint details (R-UI-7, R-UI-8, R-UI-16, R-UI-17).

- Use the approved fixed-colour Line mark for native window identity and packaged desktop icons. Provide durable project-logo, avatar and social-preview exports for manual GitHub upload (R-UI-9, R-DOC-1).
- Apply approved semantic typography with explicit line heights in reference-browser headings, manual review/confirmation, action labels and existing widgets, alongside small-corner theme/widget styling, with visible keyboard button focus and theme-correct confirmation scrims. Current manual-review prices/SCU and confirmation values use strong/large exact grouped Mono numerals; reference/delta details retain regular Mono, explicit delta signs and compact percentages; full shell/table adoption and native visual acceptance remain in progress (R-UI-7, R-UI-10, R-UI-11, R-UI-17).

- Follow the project repository transfer to `tallyline-sc/tallyline` for optional release checks, About source/release links and packaging source information (R-NF-7, R-UI-18, R-DOC-4).

- Preserve successive read-only account-check results across SQLite reopen and restart, including independent receipt checks and queue acknowledgement. Permanent refusals retain a typed correction action and their safe technical code (R-SUB-2, R-SUB-9, R-SUB-11).

- Preserve safe unknown UEX codes and account pauses independently of unavailable or partial receipt evidence. Explicit queue acknowledgement does not replay the originating uncertain Report (R-SUB-6, R-SUB-9, R-SUB-11).


- Allow a previously validated account to release reviewed Reports to the persistent queue while offline. Sending waits for validation in the current session, and replacement credentials close release and sending until accepted (R-API-7, R-SUB-2).
- Extend the optional Game.log witness to the exact MIC-L2 Long Forest Station and Orbituary terminal pairs and Astatine, Carbon, Copper, Diamond, Potassium, Scrap and Waste. The same validated build and default-off privacy boundary apply; the log supplies no reported prices, quantities or status values (R-OCR-20).

- Verify rebuilt Development MSI/ZIP/deb/tar.gz packages, exact preferred-source inventory, legal terms and fixture boundaries. Keep shared libraries as non-executable data in portable archives written on Windows, with independent format-reader and actual artefact red/green. These recorded Development tuples do not establish Windows OS isolation, installed acceptance or authenticated releases (R-NF-2:installers, R-DOC-3, R-DOC-4, R-SEC-8).

- Add fifteen balance-redacted Orbituary corpus captures with manual Buy and Local Market Value transcriptions, long names, literal warning markers and explicit clipped-card boundaries; recognition accuracy remains unmeasured (R-QA-1, R-QA-3).

- Add four balance-redacted MIC-L2 Long Forest Station corpus captures with manual transcriptions and explicit clipped-card boundaries; recognition accuracy remains unmeasured (R-QA-1, R-QA-3).
- Reject mutation scores contaminated by known independent test/build infrastructure failures using retained cause logs. Preserve genuine domain-error test failures and the frozen score floor; handle the pinned tool's explicit empty selection without inventing a measured score (R-SEC-1, R-DOC-3).

- Added bounded headless `--build-info` before native bootstrap. About, logs, UEX identity and update comparison share fixed Development `0.0.0` or complete compile-time tag metadata; invalid or partial metadata refuses startup. Bounded source-job receipts bind compiled metadata and executable hashes into protected staging before fixture access; signed-tag/CI authentication and final package acceptance remain release gates (R-NF-11, R-API-7, R-UI-18, R-SEC-8).

### Fixed

- Preserve provably safe skewed terminal titles with full-width supporting-edge source-cell privacy before both filters. Refuse unsupported header labels; supplied-panel geometry and dumps use schema 2, requiring reviewed regeneration of schema-one inputs (R-CAP-7, R-SUB-7).

### Added

- Duplicate a permanent client-review refusal once into a separate unreviewed Draft, preserving original history and renewing review/screenshot decisions. New-capture refusals have no copy action; schema16 requires a coordinated database/settings backup before upgrading (R-SUB-11, R-SUB-10, R-VAL-7).

- Explicitly send the ready rows of a mixed-cooldown Report as a separate released Report while preserving confirmations, observation time, screenshot evidence and optional affinity. Both replacements repeat normal send checks; original history remains intact (R-SUB-4). Database schema 15 requires a coordinated backup.

- Recover from UEX refusals with bounded safe retries, durable queue-wide account/API-change/quota fences, explicit strictly-newer-release version recovery and Retry now and report-fixable Draft findings. Production refusals with uncertain prior rows stay held for read-only receipt checks; schema 10 requires a coordinated backup, while partial-row resolution and daily-quota reset scheduling remain separate gates (R-SUB-2, R-SUB-6, R-SUB-9, R-SUB-11, R-API-7).

- Release reviewed manual Reports through a durable serial queue with fixed account/mode, fresh checks, restart recovery, passive history and unsent cancellation. The onboarding test uses dry-run mode; database 9/settings 6 require coordinated backups, and uncertain sends never replay automatically. Native and approved real-service acceptance remain pending (R-SUB-1, R-SUB-2, R-SUB-3, R-SUB-8, R-SUB-9, R-VAL-7, R-UI-1:reports, R-API-7).

- Add the source foundation for versioned local-VLM transcription and shared exact numeric parsing, with bounded format failures and one explicit truncation retry. Desktop inference and report fusion remain pending (R-VLM-5, R-VLM-10).

- Correct unnecessary MSI file-language defaults and the Debian launcher's missing optional icon reference, with actual prebuilt artefact red/green and independent historical package inspections. Current-source freeze, Windows OS isolation and installed/release acceptance remain separate gates (R-NF-2:installers, R-DOC-3, R-SEC-8).

- Serialise OCR test-executable preparation with all probe spawns and clean owned fixtures after setup/spawn errors. Model-free subprocess contracts retain their refusal checks; the historical Linux writable-handle cause and separate Windows linker failure remain unproved (R-SEC-7, R-SEC-8, R-NF-12).

- Add guarded manual SCU review with provisional 20 % / 16 SCU bands, independent tighten-only Settings, durable value-bound confirmations and F8/Shift+F8 targets. Database schema 8 and settings schema 5 migrate after coordinated backups; complete release/submission and native visual acceptance remain pending (R-VAL-2, R-VAL-7, R-UI-10, R-UI-11, R-UI-12, R-NF-5, R-NF-11).

- Add masked support-preview and private JSON-export foundations with bounded application-log tails. Desktop diagnostics, rotating logging and the complete support-export flow remain in progress (R-NF-6, R-UI-15, R-DOC-2).

- Add sealed prebuilt MSI/Debian assembly adapters, original-term DEP-5 output, safe Debian layout and leak-safe tool-log retention. A core CRT probe passes actual WiX validation and read-only engine cases; final protected application packages and installation acceptance remain pending (R-NF-2:installers, R-DOC-3, R-SEC-8).

- Add credential-free public UEX field/type/code checks and a default-off, main-only weekly workflow. Output excludes response values, and observed drift fails visibly without automatically changing the baseline (R-API-5, R-DOC-2).

- Bind every workspace manifest during legal source freshness checks, including feature-only changes with unchanged lockfile bytes; retain verified existing Rust/ORT source offers without helper binaries (R-DOC-3, R-SEC-8).

- Verify actual source-archive file evidence and exact native source scopes before sealing fixture insertion; reject stale legal source bindings in the normal check. Protected installer and actual token-format acceptance remain pending (R-DOC-3, R-SEC-8).

- Pin exact target legal catalogues during prebuilt verification and reject source archives that differ from their unpacked source; complete native source and protected installer acceptance remain open (R-DOC-3, R-SEC-8).

- Add explicit-band read-only SCU assessment and tighten-only policy evaluation. A read-only corpus sensitivity command records a provisional 20 % / 16 SCU recommendation without claiming OCR accuracy; broader empirical validation remains open (R-VAL-2, R-UI-10, R-UI-11).

- Prebuilt staging now seals approved native/model/font resources and binds delivered legal text to its catalogue before token insertion. The IBM font licence retains its original upstream encoding; protected installer acceptance remains pending (R-DOC-3, R-SEC-8).

- Prebuilt distribution helper foundations validate release-version syntax and approved packaging tools, write plain archives and detect harmless fixture leaks without echoing values. Complete protected installer and installed-system acceptance remain pending (R-NF-2:installers, R-NF-4, R-SEC-8).

- Add explicit developer-only Tiny/Small/Medium OCR research pairs with exact model/dictionary bindings and private experiment receipts; production profile selection and real hardware acceptance remain open (R-OCR-1, R-SEC-7; ADR-0012).

- Add fixed Small OCR model/dictionary/native data, private Windows load and mapped CRT checks, immutable model snapshots and a bounded synthetic tensor diagnostic. Desktop OCR, Linux, production worker lifecycle and installation-profile acceptance remain in progress (R-OCR-1, R-SEC-7, R-SEC-8, R-NF-12, R-NF-13).

- Prepare local OCR telemetry mitigation before startup through a bounded relaunch of the same executable, without requiring a separate launcher or mutating a running multithreaded environment (R-NF-12).

### Added

- Review manual prices and statuses with exact reference prices, age, unit/percentage differences and counters; F8/Shift+F8 visits required decisions and a separate dialog approves the displayed value. Approvals and unreviewed/revoked state survive restart through backup-gated schema 7; passive reassessment preserves USER values and existing status decisions. complete release/submission remains pending (R-VAL-2, R-VAL-2a, R-VAL-7, R-UI-10, R-UI-11, R-UI-12, R-NF-11).

- Confirmed private manual attachment regions now survive restart and deletion of the original; changed or missing working copies block attachment use. Dated screenshot requirements are remembered per validated account and can be explicitly cleared, with controls in Manual Report and a schema-six backup-gated migration (R-MAN-5, R-SUB-7, R-CAP-7, R-VAL-6).

- Keep a persistent warning for writable Linux data directories with broad permissions instead of treating the warning as a fatal storage error. Actual read/write failures still stop storage startup, and credential fallback remains strictly owner-only (R-NF-5, R-NF-4).

- Add masked UEX account setup during onboarding and in Settings, with read-only validation before saving, a packaged-token/own-token choice and explicit session-only credentials. Linux offers a warning-confirmed private-file fallback only when Secret Service is unavailable; local browsing remains available, and real-provider/visual acceptance remains pending (R-API-4, R-API-7, R-NF-4, R-VLM-12).

- Add a developer-only portable Small image/textline reader with DB boxes, original-raster crops, lazy session reuse and explicit idle cleanup. Desktop screenshot recognition and full installation-profile acceptance remain in progress (R-OCR-1, R-NF-3).

- Run the headless OCR diagnostic through an owned worker that verifies below-normal OS priority before its task starts. Production OCR scheduling remains in progress (R-NF-3).

- Preserve manual status approvals, revoked decisions and keyboard-review state across restart at the application boundary. Schema-four migration requires a coordinated backup; complete release remains in progress (R-UI-12, R-VAL-7, R-NF-11).

- Added shared bounded PNG/JPEG/BMP intake with explicit colour conversion, full-precision 16-bit PNG handling, HDR/unsupported-colour advice and original-size findings after integer box reduction. The capture UI and recognition pipeline remain in progress (R-CAP-8, R-OCR-17).
- Reference failures now show the last non-JSON HTTP status, validated Cloudflare Ray and UTC observation time without retaining HTML or changing cached data. HTTPS and same-origin checks precede every request; credentialled technical host overrides require explicit confirmation and disclosure (R-API-3, R-NF-4).
- Added persistent manual commodity Drafts with explicit terminal, side and environment choices, keyboard field entry, aged UEX references and explicit takeover. Draft edits survive restart without credentials; schema-three storage migrates after coordinated backups. Shared validation and submission remain in progress (R-MAN-1, R-MAN-2, R-MAN-3, R-MAN-6, R-NF-11).

- The desktop reference worker now loads SQLite data before network responses, rechecks versions at startup and fetches terminal priors on opening. Failed saves retain old visible records; closing cancels pending GETs and joins the cache owner before releasing the writer lease (R-API-1, R-API-2, R-UI-13, R-NF-10).

- Per-terminal LIVE priors retain exact prices and SCU, container sizes, status and source age across restarts, with independent 30-minute freshness and bounded ten-terminal requests. PTU contains no borrowed LIVE priors (R-API-1, R-API-5, R-API-6).

- Added environment-specific terminal and commodity browsing with searchable cached records, passive refresh status and endpoint ages. Channel-to-UEX mapping controls save before applying and remain independent of the optional Game.log witness; cached records now load before HTTP and refresh on the app-owned worker. Global staff notices are capped literal text, and resolved reference failures clear without hiding settings errors (R-API-1, R-API-2, R-API-6, R-CAP-3a, R-UI-8, R-UI-13, R-UI-16).

- Cached UEX submission switches, commodity tolerances and bounded staff messages remain available offline. Submission closure leaves browsing usable; parameters refresh daily and require a fresh observation before sending (R-API-1, R-UI-8, R-SUB-5).

- Cached commodity averages retain exact buy/sell values and terminal container limits. SQLite schema 2 backs up old data before migration, preserves vocabulary and fetch times, and removes the ambiguous legacy average without introducing LIVE values into PTU (R-API-1, R-API-6, R-VAL-2a, R-NF-11).

- Game-channel mappings persist independently of the optional Game.log witness in settings schema 4. Migration preserves release-check preferences and backs up old settings before replacement (R-CAP-3a, R-NF-11).

- Added exact decimal price/quantity value types and enabled environment/static Windows proxy support in the UEX adapter. PAC and per-protocol Windows proxy lists remain unsupported; desktop reference-data wiring is still in progress (R-API-3, R-API-5).

- Added optional daily GitHub release checks, off by default, and a persistent dismissible release-notes link. Offline or invalid replies remain passive; checks never download or install updates, and the permission, last attempt, validated notice and dismissal survive restarts in settings schema 3 (R-NF-7, R-UI-16).

- Added an optional, default-off Game.log witness with channel selection, safe unavailability reasons and sanitised facts in Settings and the offline eval tool. Only the documented build and finite mapping allowlist are accepted; logs never supply reported prices, quantities or status values, and settings migrate forwards to schema 2 (R-OCR-20, R-NF-5).

- A second installed or portable launch activates the existing window through local IPC. Both writer locks remain held for the application lifetime; failed activation shows an already-running notice and never starts another writer (R-NF-10).

- Added the native desktop shell with first-launch onboarding, dark, light and high-contrast themes, offline legal notices and persistent application settings. The workspace and Settings have clear Home/Done navigation, and Settings failures remain visible without taking focus; installed and portable builds keep state in their documented directories (R-UI-7:theme, R-UI-8, R-UI-16, R-UI-18, R-UI-19:app, R-L10N-1, R-NF-2:portable, R-NF-5, R-NF-8).

- Offline corpus inspection reports schema-v2 conditions, expected values, frozen reference metadata, sample readiness and unavailable inputs. Anchor annotation writes a separate unverified proposal, and a contributor capture guide explains reproducible ladders and privacy protections (R-QA-1, R-QA-3, R-DOC-1).

### Fixed

- Give the Game channel directory input the same accessible name as its visible label (R-UI-17a, R-OCR-20).

- Settings migration with an existing SQLite database now creates one integrity-checked database/config backup set before replacement. Backup failure stops startup processing and preserves the previous files (R-NF-11).

- Backup retention preserves the newly created snapshot when the clock moves backwards and retains the newest three completed creations across restarts. Failed backups and timestamp collisions leave existing snapshots untouched (R-NF-11).

- Update-check timestamps outside the settings format are refused without panicking or replacing saved preferences (R-NF-7, R-NF-5).

- Settings upgrades require a successful backup before replacement. The bootstrap without a database preserves exact previous settings and retains the last three recognised backup sets; a database requires coordinated backup (R-NF-11).

- A second launch restores a minimised window even when no view is being drawn. Ordinary background results remain unable to restore or focus it (R-NF-10, R-UI-16).

- OCR test fixtures retry transient Windows directory-allocation denial within their existing limit, without skipping assertions or changing product storage behaviour.

- Open-source licence rows show the documented elected option while retaining the upstream declaration and every original legal text; stale or incompatible elections fail generation (R-UI-18).

- Invalid onboarding values now report the actual rejected JSON value and default without exposing unrelated account settings (R-NF-5).

- Installed Windows data and Linux logs now use the dedicated directories required by the storage specification; portable paths retain their separate layout (R-NF-5).

- About identifies the fixed development package version together with the OS and architecture (R-UI-18).

- Theme changes now repaint the native root surface and text together, including dark/light/dark transitions. The offline licence catalogue excludes Apache boilerplate from copyright-holder metadata and preserves the actual AccessKit source attribution.

### Changed

- Repository checks and generators support external Cargo output directories, preserve invocation-relative paths and isolate mutation builds per source copy. Owned-output checks refuse replacement of foreign directories (R-SEC-1).


- Require strictly local optional AI and prefer a complete supplied recognition profile. Corrected 2026-11-04: ADR-0013 qualifies the earlier absolute no-separate-Ollama/no-later-setup promise with the owner-authorised conditional fallback and requested model acquisition. Runtime/model/hardware selection and the separate Windows OCR prerequisite remain open (R-VLM-6/7/11/12).

- Refresh the desktop UI, TLS and async dependencies to eligible stable releases while preserving the approved native boundaries and complete legal notices (R-SEC-1, R-SEC-10).

- Migrated the Pyro Gateway transcription to schema version two with unknown conditions left null; human verification is still required (R-QA-1).

### In progress

- Preserve manual attachment privacy and time decisions on Drafts, with source-hash/geometry-bound region previews and bounded background PNG encoding. Schema-five migration requires a coordinated backup; desktop controls, durable crop files, account requirement memory and release remain pending (R-MAN-5, R-SUB-7, R-CAP-8, R-VAL-6, R-NF-11).
