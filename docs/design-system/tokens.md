# Design tokens

> **Doc type:** Living spec — draft (JavaFX handoff stage 2; ADR-0004 partly superseded by ADR-0005). Last reviewed: 2026-11-03.

> **Note 2026-10-12:** [ADR-0005](../adr/0005-rust-and-egui.md) replaces JavaFX with egui. The token files, their schema, the conversion rules and the values below stand until open point O-112 (adoption of the `tallyline-egui` crate) says otherwise. The section "JavaFX mapping" and the Java scripts under "Regenerating" describe a superseded mechanism: the egui rendition maps the same tokens to `egui::Style`, `TextStyle` and per-face font families, and an `xtask` task replaces the Java scripts once O-112 is settled. See [README.md](README.md), "egui rendition (ADR-0005)".

The design tokens of the app's design system, Tallyline, live in [`ui/design-tokens/`](../../ui/design-tokens/). They were converted from the Claude Design system Tallyline, version `1791542897-a5d3` (2026-10-09), by `ConvertTokens.java` (removed 2026-10-12, see "Regenerating" below). The token files are the repository's source of truth; the Claude Design artifact is a reference and is never read at build or run time. Why this schema and these rules: [ADR-0004](../adr/0004-design-system.md).

## Files

| File | Contents |
|---|---|
| `tokens.resolver.json` | The [DTCG Resolver Module 2025.10](https://www.designtokens.org/TR/2025.10/resolver/) document: one set `base` and one modifier `theme` with the contexts `dark` (default), `light` and `high-contrast` |
| `base.tokens.json` | Theme-independent tokens in the [DTCG Format Module 2025.10](https://www.designtokens.org/TR/2025.10/format/): `font-face`, `font-family`, `typography` (20 styles), `spacing` (10), `radius` (3), `width` (6), `size` (16, two of them deprecated), `motion` (2 durations, 1 easing) |
| `theme-<id>.tokens.json` | Per theme: `color` (99 tokens, [DTCG Color Module 2025.10](https://www.designtokens.org/TR/2025.10/color/), sRGB with `hex`) and `shadow` (3) |
| `pairings.json` | The declared foreground/ground pairs with their required ratio per theme (575) and the co-occurrence list, from the system's "Accessibility pairings" section |
| `states.json` | The state ids with label, icon, tokens and accessible text (179), from the system's "State language" section; their mapping is [states.md](states.md) |

The three DTCG modules are Final Community Group Reports of 28 October 2025 (checked on designtokens.org on 2026-10-09). Our own data sits under the `$extensions` key `tallyline` (the product name, O-8).

## Conversion rules

- **Colours:** every value is either an alias `{color.<name>}` or an sRGB object with `components` (0–1), `alpha` and the six-digit `hex`. Only `scrim` is translucent, and only in dark and light; high contrast has no translucent colour.
- **Shadows:** single layer, zero spread. A shadow that is `none` in a theme is kept as a fully transparent shadow marked `"none": true` in `$extensions`, so every theme defines every token.
- **Typography:** DTCG `typography` with `letterSpacing` 0 and a unitless `lineHeight` (line height ÷ font size); the line height in px is kept in `$extensions` as `lineHeightPx`. Weights are 400, 600 and 700; 600 exists only as its own face (see "JavaFX mapping").
- **Font families:** one family per style, without the fallback chain of the web previews (J4).
- **Lengths:** px at the 14 px base, as in the system.
- **Retired tokens:** a token whose usage in the system starts with "Retired YYYY-MM-DD" keeps its value and gets the DTCG `$deprecated` property (Format Module §5.2.4) with that date. No view and no generated sheet may use it; it stays only so that references do not break.
- **Motion:** the system's README defines 120 ms (hover, pressed), 160 ms (selection, panel open) and `ease-out`, but its `tokens.json` has no motion family; the converter adds them as `motion.duration-hover`, `motion.duration-select` and `motion.easing-standard` (`[0, 0, 0.58, 1]`).
- **Validation:** `ValidateTokens.java` (removed 2026-10-12; an `xtask` task takes over its checks in M0) failed loudly and fixes nothing. It checks that every colour and shadow token has a value in every theme, that aliases resolve without cycles, that every name matches `[a-z0-9-]`, that colours are sRGB objects or aliases whose components agree with their `hex`, that high contrast has no translucent colour and no shadow, that shadows have one layer and no spread, that type styles have no letter spacing and use a listed face for their weight, that rows and in-cell targets are at least 24 px, and that every pairing and state names known tokens and unique ids. Last run: passed on version `1791542897-a5d3` (2026-10-09); two conversion runs gave byte-identical files.

## JavaFX mapping

> Superseded 2026-10-12 by ADR-0005 D6 (the mapping to egui replaces it; kept as the record of the JavaFX handoff). The SP-n findings it cites are JavaFX spike results.

| Token | JavaFX 27 CSS |
|---|---|
| colour `<name>` | looked-up colour `-tl-<name>` on `.root`, written as `#rrggbb` or `rgba(r, g, b, a)`; never `-fx-`, never `hsl()` or a named colour (J2). The prefix is required: an unprefixed lookup such as `red` overrides the named colour (SP-3) |
| length `<n>px` | written as a **literal** `<n ÷ 14>em` into the generated sheets where the node's font is the root font; an `em` resolves against the font of the node that uses it (SP-1), so exceptions are listed per component in `components.md`. Size lookups fail for padding, insets, radii and border widths (SP-1), so lengths are never runtime lookups |
| `typography` | `-fx-font-family` with one family (a list is no fallback), a literal `-fx-font-size` in em (a lookup is a parse error, SP-1), `-fx-font-weight` 400 or 700 only; weight 600 is `-fx-font-family: "IBM Plex Sans SmBld"` at normal weight, because weights up to 600 render regular (SP-4). The family names are "IBM Plex Sans", "IBM Plex Sans SmBld" and "JetBrains Mono NL"; IBM Plex Sans lacks ● ▲ ▼ ⚠, which are drawn as icons |
| `lineHeight` | `-fx-line-spacing` = line height − the face's natural line height at that size; the natural line height is measured per face in SP-4, and `lineHeightPx − fontSize` is only an upper bound |
| `shadow` | `dropshadow(gaussian, <colour>, <radius>, 0, <x>, <y>)` with `radius = 1.5 × blur` (a CSS blur radius `b` is a Gaussian with σ = b ÷ 2; the JavaFX kernel radius is 3σ), capped at 127; `none` sets no effect; one effect per node (J13) |
| `motion` | `transition` with literal durations inside `@media not (prefers-reduced-motion)`; a duration lookup fails in transitions (SP-1) but works for tooltip delays |

Root font size = max(`Font.getDefault().getSize()`, 14 px) × the user's text-size factor (handoff §5).

## Values

**Type styles** (px at the 14 px base; family `sans` = IBM Plex Sans, `mono` = JetBrains Mono NL):

| Style | Family | Size / line height | Weight |
|---|---|---|---|
| `display-name` | sans | 40 / 44 | 700 |
| `heading-1` | sans | 22 / 28 | 700 |
| `heading-2` | sans | 18 / 24 | 600 |
| `heading-3` | sans | 15 / 20 | 600 |
| `label-caps` | sans | 12 / 16 | 600 |
| `body` | sans | 14 / 20 | 400 |
| `body-strong` | sans | 14 / 20 | 600 |
| `ui-small` | sans | 13 / 18 | 400 |
| `caption` | sans | 12 / 16 | 400 |
| `button` | sans | 14 / 20 | 600 |
| `chip` | sans | 13 / 18 | 600 |
| `table-header` | sans | 12 / 16 | 600 |
| `table-name` | sans | 14 / 20 | 600 |
| `table-text` | sans | 14 / 20 | 400 |
| `table-meta` | sans | 13 / 18 | 400 |
| `numerals` | mono | 14 / 20 | 400 |
| `numerals-strong` | mono | 14 / 20 | 700 |
| `numerals-large` | mono | 18 / 24 | 700 |
| `mono-log` | mono | 13 / 18 | 400 |
| `keycap` | mono | 12 / 16 | 400 |

**Spacing:** 2, 4, 6, 8, 12, 16, 24, 32 px; cell padding 8 px (compact) and 12 px (comfortable). **Radius:** 0, 2, 4 px. **Width:** hairline 1, state 2, focus ring 2, focus gap 1, frame accent 2, selection mark 4 px.

**Size:** rows 28 px (compact) and 36 px (comfortable); controls 32 px, small controls and in-cell controls 24 px; icons 16 and 20 px; rail 48, sidebar 232, top bar 40, status bar 28 px; minimum window 1280 × 720 px; Fan Kit logo frame 96 px. `size-crop-thumb-compact` (20 px) and `size-crop-thumb-comfortable` (28 px) are deprecated together with the CropThumbnail component (retired 2026-10-09): no view shows a small fixed-height crop.

**Readable crop** (R-UI-3, R-UI-11; a rule, not a token): the crop of the focused field in the DetailStrip and in the deviation-confirmation dialog is scaled up by the smallest whole-number factor, with nearest-neighbour sampling, at which the cap height of its digits is at least twice the cap height of the body text. IBM Plex Sans has a cap height of 0.698 em, so at the 14 px base the target is about 19.5 px; digits about 7 px high in the capture get factor 3. The crop is never blurred by interpolation and never cut; the strip grows when the crop is taller. The bounds that replaced `size-crop-detail-min-height` and `size-crop-detail-max-height` (32 and 96 px, removed in version `1791541897-0513`) are this rule. Unverified until the toolkit spike (written for JavaFX; under ADR-0005 the same question is asked of egui's nearest-neighbour texture filtering): that `ImageView` with `smooth = false` scales a crop without interpolation at every HiDPI scale, and the source of the digit cap height (the R-OCR-17 measurement of the price digits).

The colour values per theme are in the theme files; their contrast results are in [accessibility-report.md](accessibility-report.md).

## Regenerating

> Removed 2026-10-12 (ADR-0005 D12, O-112): the Java scripts `ConvertTokens.java`, `ValidateTokens.java` and `Contrast.java` that converted a Claude Design export into `ui/design-tokens/`, validated it and wrote the contrast section of the accessibility report are gone from the repository and stay in the Git history. Their replacement is an `xtask` task, built in M0 together with the reconciliation of `ui/design-tokens/` and the `design/tokens.json` of the Tallyline egui crate. Until then the token files below are edited only together with the documents they change, and a new system version is converted in the same commit.

## egui typography and alpha checkpoint (2026-11-03)

Corrected 2026-11-03: resolved Rust scrims preserve source alpha: dark 204, light 230 and high contrast 255 out of 255. The requested manual-value modal consumes this token. All 20 type styles register their exact face/size. Current reference-browser headings, review and confirmation headings, required-decision emphasis, action labels and existing widgets select semantic `TypeStyle::text` roles with explicit line heights. Current price/SCU labels use `numerals-strong`, reference/delta details use `numerals`, and confirmation values use `numerals-large`. Other standard egui labels retain natural line height. `FontFace` shares names with the separate-file loader. Action labels explicitly select the named button face, size and line height because egui 0.37's default atomic button provider otherwise selects Body. Existing approved font/licence bytes are unchanged. See [component adoption](components.md).
