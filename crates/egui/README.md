<!-- SPDX-FileCopyrightText: 2026 Lucas Greuloch -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# `tallyline-egui`

Tallyline theme tokens and reusable widgets for egui 0.37.0.

The crate currently provides dark, light and high-contrast colour palettes;
theme, density and reduced-motion settings; 156 named Material Symbols Rounded
codepoints and nine compatible semantic state icons; and the
`IconButton`, `Chip`, `DeltaBadge`, `Field`, `Panel` and
`Toast` widgets.

```rust,ignore
use tallyline_egui::{apply_theme, Density, Motion, Theme};

apply_theme(ctx, Theme::Dark, Density::Comfortable, Motion::Full);
```

`design/tokens.json` is a snapshot of `ui/design-tokens/`. The workspace test
`tallyline_egui_tokens_match_the_design_system_sources` compares the snapshot
with those DTCG source files. `src/tokens.rs` contains the generated resolved
colour tables. Token and icon generation sources are maintained outside this
repository; only generated Rust and the reconciled JSON snapshot are committed.

Corrected 2026-11-03: the exact historically approved Material Symbols subset
is restored from the supplied design archive. The desktop loader hash-checks
the separate font file before registering its dedicated family. The public
symbols module retains all 156 named codepoints; existing state consumers
retain their Icon API. NOTICE §2.13 records source and modification limits.
Actual atlas-glyph contracts cover 16/20 points at 100, 125 and 150 percent;
native/magnified legibility review remains open.

Corrected 2026-11-03: see [component adoption](../../docs/design-system/components.md) for the supplied archive identities and partial typography/widget adoption. All 20 named type styles and five approved face weights are installed through the separate-file desktop loader; native visual acceptance remains open.
