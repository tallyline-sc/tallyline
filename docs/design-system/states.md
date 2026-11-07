# States: from the domain to the screen

> **Doc type:** Living spec — draft (JavaFX handoff stage 2; ADR-0004 partly superseded by ADR-0005). Last reviewed: 2026-11-03.

> **Note 2026-10-12:** [ADR-0005](../adr/0005-rust-and-egui.md) replaces JavaFX with egui. The state ids, their meaning and their plan sources stand. The JavaFX column below (pseudo-classes such as `:confidence-confirm`, `:row-selected`, and the style classes `tl-state-<id>`) names a CSS mechanism that no longer exists: in the egui rendition each state is a variant or flag on the widget (for example `Confidence`, `Deviation`, `ChipTone` of the Tallyline crate), keyed by the same state id, and the "Selection and focus in the review table (SP-9)" paragraph is a JavaFX finding (egui has no CSS pseudo-classes; the review table is our own work, ADR-0005 D3). Open point O-112 decides what of the crate is adopted; see [README.md](README.md), "egui rendition (ADR-0005)".

This document maps every user-visible state of the plan to the Tallyline state ids, and each id to what the JavaFX theme uses: a pseudo-class or a style class. The id list with labels, icons, tokens and accessible texts is generated into [`ui/design-tokens/states.json`](../../ui/design-tokens/states.json) from the system's "State language" section (Claude Design system Tallyline, version `1791542897-a5d3`, 179 ids). The domain terms are those of [11](../plan/11-ddd-and-tdd.md) §A1.

## Rules

- Every state is shown by colour, icon and text together (R-UI-4, R-UI-11, R-UI-17). Colour comes only from the tokens; the ViewModel sets pseudo-classes and style classes and holds no colours or thresholds (02 §7, 09 §7).
- A state id is the stable key. The label and the accessible text come from bundle keys `state.<id>.label` and `state.<id>.accessible` (R-L10N-1); the icon is fixed per id.
- **Reference outdated** is a modifier, not a level: with it, a minor deviation is shown like equal, a major deviation is never lowered and still needs a confirmation, and "no reference" stays ([07](../plan/07-ocr-concept.md) §2.6). The default staleness limit is UEX's `commodity.ttl` (15 days today) with a 15-day fallback (R-UI-11).
- **Unreviewed** follows its definition in 11 §A1: a counter and a row marker (`radio_button_unchecked`), never a gate; F8 does not jump to it. The system files it under "Row markers and counters".
- **Needs confirmation** marks every case that blocks release and drives the row marker, the F8 targets and the counters. An optional value that fails its checks is `field-not-sent` and never sets it; that includes container sizes the game log contradicts (`gamelog-sizes-inconsistent`).
- UEX's own per-row statuses always carry the prefix "UEX:", so they never read as a report or job state. Only consolidated, declined and expired are final; whether a row can be withdrawn comes from UEX's `is_editable`, never from the status.
- Glyphs are distinct within each group, including the 17 block reasons; the action "Withdraw row" keeps `undo`, its result uses `settings_backup_restore`.
- Test and production are shown in the shell of every view and on report and history rows; their tokens (`env-test-*`, `env-production-*`) stay outside the critical family.

## Field level: the pseudo-class set of 02 §7

The "JavaFX" column in this and the next section is the superseded CSS mapping (note above); the state ids in the first column are the stable keys.

| State id | JavaFX | Plan source |
|---|---|---|
| `confidence-ok` | none (level OK has no marking in cells) | 07 §2.6 |
| `confidence-confirm` | `:confidence-confirm` | 07 §2.6, R-UI-4 |
| `confidence-select` | `:confidence-select` | 07 §2.6, I2 |
| `confidence-correct` | `:confidence-correct` | 07 §2.6, I2 |
| `user-entered` | `:user-entered` | 11 §A1 "Origin" |
| `confirmed` | `:confirmed` | I3 |
| `needs-confirmation` | `:needs-confirmation` | 02 §7, I2 |
| `deviation-equal` | none | 07 §2.6 |
| `deviation-minor` | `:deviation-minor` | R-UI-10 |
| `deviation-major` | `:deviation-major` | R-UI-10, R-UI-12 |
| `no-reference` | `:no-reference` | R-UI-10 |
| `reference-outdated` | `:reference-outdated` | R-UI-11, 07 §2.6 |

**Additions to 02 §7** (decided 2026-10-09 by @greluc, checkpoint B):

| State id | Pseudo-class | Why |
|---|---|---|
| `double-confirmed` | `:double-confirmed` | OCR and AI agree (07 §2.7); a subtle border and icon distinct from `:confirmed` |
| `deviation-worsened` | `:deviation-worsened` | I6: a confirmation was revoked because the deviation worsened |
| `superseded-conflict` | `:confirmation-suspended` | I7: a confirmation is suspended by a conflicting reading |
| `ai-hint` | `:ai-hint` | 07 §2.7: a differing AI reading that never overwrites |
| `field-not-sent` | `:not-sent` | 11 §A1 "Mandatory field": an optional value that failed its checks is not sent and does not block |
| `superseded-later-reading` | none; `:confidence-confirm` with the reason as text | 07 §2.5b |

**Selection and focus in the review table (SP-9):** in cell-selection mode the focused cell gets `:focused` (never `:focus-visible`), but rows never get `:selected`; in row-selection mode the row gets `:selected`, but no cell gets `:focused`. The design needs both a focused cell and a selected row, so the table runs in cell-selection mode and the row receives the pseudo-class `:row-selected` from the selection model (02 §7, decided 2026-10-09).

**`:needs-confirmation`** covers the I2 cases, a confirmation revoked by a worsened deviation (I6), a confirmation suspended by a conflicting reading (I7), an unexpected commodity (R-UI-12) and a row flagged "newer observation already sent" (R-VAL-8); all of them already block release (02 §7, decided 2026-10-09).

## Row level

| State id | JavaFX | Plan source |
|---|---|---|
| `unreviewed` | row pseudo-class `:unreviewed` | 11 §A1 "Unreviewed" |
| `row-excluded` | row style class `tl-state-row-excluded` | 02 §3 |
| `not-observed` | row style class (NotObservedRow) | 11 §A1, R-VAL-5 |
| `marked-missing` | row style class (MissingRow) | 11 §A1, R-VAL-5 |
| `unexpected-commodity` | row style class plus the finding | 11 §A1, R-UI-12 |
| `possibly-incomplete` | report notice | 07 §2.5b |
| `section-not-visible`, `section-unsupported` | report notice | 07 §2.5b |
| `newer-observation-sent` | row pseudo-class `:newer-observation-sent`, plus `:needs-confirmation` | R-VAL-8 |

## Everything else: style classes

Report, job, capture, intake, folder, game, game version, game log, faction affinity, staff notice, AI, AI job, connection, account, queue, UEX row lifecycle, report-ID mapping, per-row withdrawal, mode and generic-feedback states are not field states. Each is drawn by a component (StateChip, QueueRow, HistoryRow, UexRowLifecycle, RowWithdrawControl, QueueHoldNotice, AccountStateLine, IntakeSetAsideLine, AiJobChip, Banner, StatusBar) with the style class `tl-state-<id>`; no change to 02 §7 is needed. The groups, their ids and their plan sources:

| Group (`states.json`) | Ids | Plan source |
|---|---|---|
| Report state | 19 (`report-*`) | 02 §3 `ReportState`, Draft sub-states R-UI-1, `report-partly-withdrawn` I4, `report-split` 02 §3, `report-possibly-received` R-SUB-9 |
| Job | 16 (`job-*`), each with its own glyph; `job-next`, `job-waiting-row-budget` and `job-waiting-retry-time` are defined in the queue group. A report whose job is queued, next or sending reads "Queued", one waiting for its cooldown "Waiting for cooldown", never "Released" | 02 §6, R-SUB-2 |
| Queue | `queue-paused-app-token`, `queue-paused-key-unconfirmed`, `queue-held-api-changed`, `queue-held-stale-acceptance`, `queue-held-not-accepting` | R-SUB-11, R-SUB-5 |
| Account | `account-*` (6), `key-*` (5) | R-API-4, R-SUB-11 |
| UEX row lifecycle | `uex-*` (10) | R-SUB-4, 06 "Reading, correcting and withdrawing reports" |
| Report-ID mapping and withdrawal | `mapping-*` (4), `row-withdraw*`, `row-withdrawn`, `row-already-consolidated` | R-SUB-9, I4 |
| Capture | `capture-*` (10), among them `capture-waiting-version-data` and `capture-ready` ("Ready to scan") | R-CAP-1c, R-CAP-1d, R-UI-13, R-OCR-8, R-OCR-16 |
| Capture intake (files set aside, not captures) | `intake-*` (8): HDR, HDR container, SDR copy used, decode failed, file too large, too many pixels, colour profile too large, skipped after a crash | R-CAP-8, R-CAP-1d |
| Folder | `folder-*` (7), among them `folder-polling` | R-CAP-1e |
| Release split and returns | `release-split`, `returned-*` (5: date rejected, screenshot too large, too many rows, terminal not found, interrupted – restart) | 02 §3, R-SUB-11 |
| Game, game version, game log | `game-*` (3), `game-version-*` (4), `gamelog-*` (6) | 02 §4b, R-CAP-3b, R-OCR-20 |
| Faction affinity, staff notice | `affinity-*` (2), `staff-notice` | R-SUB-13, R-UI-8 |
| AI and AI jobs | `ai-*` (11), `ai-job-*` (6), `ai-queue-paused`, `ai-remote-host`, `ai-pull-progress` | R-VLM-1…8, 02 §4b |
| Connection and mode | 11 connection ids, `env-test`, `env-production`, `env-other-channel` | R-UI-8, R-SUB-8, R-CAP-10 |
| Generic feedback | `feedback-*` (6) | — |

## Design defects

Found when version `1791537441-68f0` was checked against its brief and the plan on 2026-10-09 (1–16), and when version `1791541897-0513` was checked on the same day (17–18). They go back to Claude Design or to @greluc; the repository does not patch the design silently. A closed defect keeps its row with the version and the check that closed it.

| # | Defect | Plan | Status |
|---|---|---|---|
| 1 | "UEX: approved" is shown as final and "not editable" | Final UEX statuses are consolidated, declined and expired; editability comes from `is_editable` (R-SUB-4, 02 §6) | Closed 2026-10-09 in `1791541897-0513`: "UEX: approved" is not final ("still checked for changes"), and withdrawal follows UEX's "editable" flag |
| 2 | Combination case 13 (confirmation revoked, now "no reference") cannot occur: a move to no reference is never "worse" | 07 §2.6 `isWorseThan`; the case came from the update brief, not from the plan | Closed 2026-10-09 in `1791541897-0513`: the case is removed, and the matrix lists a revocation by a move to "no reference" under "never show" |
| 3 | The staff notice is a banner only in the report editor and manual capture, and "Show full notice" opens the uncapped text | R-UI-8, 02 §7, O-100: in the global status area, length-capped | Decided 2026-10-09 (R-UI-8). Closed 2026-10-09 in `1791541897-0513`: a short form in the StatusBar of every view plus a banner in the report editor and Manual capture, cut after 280 characters, no "Show full notice" |
| 4 | `gamelog-sizes-inconsistent` sets the gate marker | Container sizes are optional: a failing set is "not sent" and does not block (11 §A1); R-OCR-20 makes a contradiction `Ambiguous` | Closed 2026-10-09 in `1791541897-0513`: the state is `field-not-sent` (`cancel_schedule_send`, `neutral-*`), never sets the gate marker |
| 5 | The release-split sample counts 11 rows in three parts and ignores the row without a size set | A row without a sent set goes into its own part without the field (02 §3) | Closed 2026-10-09 in `1791541897-0513`: four parts, the row without sizes in its own part; one leftover is defect 18 |
| 6 | Reports whose job is sending, next or waiting for cooldown are labelled "Released" | Those jobs belong to a `Queued` or `WaitingForCooldown` report (02 §6) | Closed 2026-10-09 in `1791541897-0513`: "Queued" while the job is queued, next or sending; "Waiting for cooldown"; never "Released" then |
| 7 | "Queue held – UEX API changed" has no "retry now" | R-SUB-11: until an app update or the user's "retry now" | Closed 2026-10-09 in `1791541897-0513`: the label ends "· Retry now" |
| 8 | A failed AI job uses the critical tone | 02 §4b: the report keeps its OCR result and the queue row shows an info note | Closed 2026-10-09 in `1791541897-0513`: "AI could not read it – … OCR result kept", `report_off`, `info-*` |
| 9 | One "Reference outdated after 7 days" example remains (Panel preview) | 15-day default (R-UI-11) | Closed 2026-10-09 in `1791541897-0513`: no outdated example under 15 days remains in the system or the boards (text search) |
| 10 | Glyphs still repeat inside a group: `help` six times in the block-reason list, `schedule` four times, `visibility_off` twice; `swap_horiz` for both superseded states; `undo` for the withdraw action and its result; "newer observation already sent" uses `schedule` as a block reason and `upcoming` as a row flag | The design's own rule: distinct within each group | Closed 2026-10-09 in `1791541897-0513`: 17 distinct block-reason glyphs, `timeline` for the later reading, `settings_backup_restore` for a withdrawal result, `upcoming` for both uses of "newer observation already sent", a glyph per job state; no group in `states.json` repeats a glyph (script check) |
| 11 | Findings severity still files Ambiguous, a repair without a witness, UnvalidatedGameVersion and UnexpectedCommodity as blocking/critical | The confidence ladder makes them select- or confirm-level (07 §2.6) | Closed 2026-10-09 in `1791541897-0513`: only Unreadable and TextTooSmall are critical; Ambiguous is select level; the others confirm level; OutOfTolerance uses the deviation families |
| 12 | The detail-strip crop is bounded by box height (32–96 px), not by digit size; under 1:1 a small-text capture shows digits at or below body-text size | R-UI-3 "readable size"; R-OCR-17 | Decided 2026-10-09 (R-UI-3). Closed for the design 2026-10-09 in `1791541897-0513`: whole-number nearest-neighbour scale until the digits' cap height is at least twice the body text's, in the DetailStrip, the Main board and the deviation dialog; `size-crop-detail-*` removed ([tokens.md](tokens.md), "Readable crop"). The JavaFX spike is still open |
| 13 | StatusBar and TopBar previews do not show the queue-wide holds, the app-token pause or the staff notice; the Settings board has no "UEX account" section with the app-token field | 02 §7 status area, R-API-4 | Closed 2026-10-09 in `1791541897-0513`: the StatusBar, TopBar and shell boards show holds, pauses and the staff notice; board 9b SettingsAccount has the account section |
| 14 | `PossiblyAlreadyReceived` (R-SUB-9), the capture label "waiting for the first UEX version data" (R-UI-13), the folder status "polling active" (R-CAP-1e) and specific labels for the return reasons `TOO_MANY_ROWS`, `TERMINAL_NOT_FOUND` and `RESTART` are missing | — | Closed 2026-10-09 in `1791541897-0513`: `report-possibly-received`, `capture-waiting-version-data`, `folder-polling`, `returned-too-many-rows`, `returned-terminal-not-found`, `returned-restart` |
| 15 | The capture label "Ready" collides with the report label "Ready" in the same queue list | — | Closed 2026-10-09 in `1791541897-0513`: "Ready to scan" (`check`) |
| 16 | `README.md` of the system still calls "unreviewed" a proposal | Decided 2026-10-09 (11 §A1) | Closed 2026-10-09 in `1791541897-0513`: "Unreviewed" is a listed domain term and has its own section |
| 17 | `capture-ready` and `capture-refinery` lost their own table rows: they appear only in the capture summary line, without tokens or accessible text (`68f0` had `neutral-*` and "Capture ready to scan"); the refinery label reads "scan result: Refinery terminal – not supported" with a description folded into the label | Every state has a label, an icon, tokens and an accessible text (R-UI-4) | Closed 2026-10-09 in `1791542897-a5d3`: a new section "Capture states before and after the scan" gives `capture-waiting-version-data`, `capture-ready`, `capture-refinery` and `folder-polling` their own rows with `neutral-*` and an accessible text; the refinery label is "Refinery terminal – not supported" |
| 18 | The ReleaseSplitNotice README gives the accessible name "Will be sent as 3 reports", while its preview and the boards say four | Defect 5 | Closed 2026-10-09 in `1791542897-a5d3`: the README says "Will be sent as 4 reports" |

## Release update states (2026-11-01)

The desktop shell draws release status with normal token-backed text and controls. A newer standard release has a persistent bordered notice with a version label, **Open release notes** and **Dismiss update notice**. Settings shows a distinct text status for not checked, no newer release and unavailable; none relies on colour alone. The setting is a labelled checkbox, off by default, and changes only after a successful save. Results never select another view, open a modal, request focus or change window state (R-UI-16). Dismissal waits for successful persistence and survives restarts; the same release stays hidden, while a different newer release can appear. A known available release survives unavailable checks and restarts without another request.

## Settings control names (2026-11-01)

The Game channel directory text input is associated with its visible catalogue label for accessibility metadata. The critical drawn Settings flow checks the input role and that exact name; upgrading the harness keeps its accessible-name validation enabled (R-OCR-20, R-UI-17a).

## Reference browser states (2026-11-01)

The reference browser uses normal token-backed labels and controls in every theme. Global text identifies the mapped UEX environment, an empty cache, usable cached data or offline retained records, with a separate static refresh label. Endpoint ages distinguish missing data from an unknown age after a reversed clock. Search fields have explicit accessible label associations and stable widget IDs so background status changes preserve keyboard focus. Terminal and commodity rows expose their UEX IDs alongside display text; selection never depends on colour alone. Channel mapping is independent of the optional witness and remains unchanged until persistence succeeds (R-CAP-3a, R-UI-8, R-UI-13, R-UI-16, R-UI-17).

The shell shows one copy of reference status in every view, including the parameter endpoint age and at most 280 Unicode characters of literal UEX staff text. Markup and links have no active behaviour. A paused submission service leaves browser controls enabled; reference delivery recovery clears its own notice without suppressing a settings failure.

## Manual attachment states (2026-11-02)

The private-region editor component uses normal token-backed text and controls in every theme, with accessible labels on the source path, rectangle spin buttons and time input. A 100% source-scale scrollable preview uses nearest-neighbour texture sampling and an explicit contains-no-private-data action. Dated account-required/unknown/unavailable states, missing or changed files, uncertain time, unsupported HDR/colour, invalid geometry and storage failures have text plus an action; none relies on colour. Passive results never change focus, view or window state. Current Report ID/version/binding and account identity filter stale results. Manual Report composes the controls with accepted account state even offline. Passive updates preserve unfinished input and do not clear another Report's pending request. Native visual acceptance remains pending.

## Manual numeric review states (2026-11-02)

Manual Report composes price/status review with Reporting-owned counters and required-decision targets. Major/minor/no-reference states use existing theme tokens, text and semantic icons; stale Minor displays as Equal while retaining its unlowered comparison. Value approval opens only after an explicit action and requires a second explicit action in the dialog. F8/Shift+F8 records durable focus without approval, with stable widget identities across counter changes. Passive reference checks keep previous details visible with an updating notice, suspend approval and preserve keyboard focus; unfinished input delays admission. Failed checks wait for an explicit retry. Numeric review and attachment readiness remain separate from complete release readiness. Native visual/accessibility acceptance remains pending.

## Existing widget rendition checkpoint (2026-11-03)

Chips, deviation badges, fields and raised panels use small radii; minor badges have an outline. Toast tones are a closed `SemanticTone` enum with actual foreground/background/border tokens. Built-in button focus uses the 2 px focus ring; text-selection backgrounds remain separate from row selection. Exact manual-review values use grouped Mono text, positive delta signs, U+2212 and adjacent percent signs without changing domain decisions. Native walkthroughs, full state matrices and independent text-edit focus styling remain open. See [component adoption](components.md).

## Shared credential feedback (2026-11-03)

The existing `AccountState` distinguishes unconfigured, rejected and validated identities. No validated secret key, invalid key and not allowed use distinct approved `key_off`, `vpn_key_off` and `block` glyphs with critical foreground text; validated identities use `check_circle` and success foreground. Invalid input/token, update-required, connection and secure-store failures retain their typed messages and separate edit/key/update/cloud/store glyphs. Connection, update and store failures use caution foreground and never imply a banned account. Disabled, banned and RSI-verification distinctions from the archive cannot be inferred from this model.

Secret key and own-token entry start masked, use explicit Show/Hide actions and reset visibility on replacement, validation or clearing. The key confusion hint stays immediately below its field. Complete fields and actions are vertically composed within the existing page scroll area; clipped controls and the Settings account expander reveal only on gained focus. A persistent credential input scope preserves keyboard focus when passive feedback inserts text without following the changed layout. Drawn contracts cover masking and actual painted output, accessible names, command lifetime/clearing/busy/fallback behaviour and minimum-size keyboard reachability at ordinary/double zoom across three themes. These checks do not establish native rendering, screen-reader or operating-system scaling acceptance.
