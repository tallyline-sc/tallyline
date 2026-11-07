# Desktop resources

`fonts/` holds the approved, unmodified static font files and each family's exact upstream OFL text. The app image keeps this directory as `assets/` next to `tallyline`; a source build may select it explicitly with `--assets`. Provenance and hashes are in `NOTICE` §2.12 and `docs/dependency-pins.md`. Star Citizen Fan Kit images remain in `docs/images/fankit/` and are compiled unchanged into the app by the composition root.

The unmodified upstream OFL texts retain their original line endings and
trailing space. Their narrowly scoped Git whitespace attributes preserve
these legal source bytes; application source formatting is unchanged.


The fonts/material-symbols/ directory additionally holds the exact historically approved modified Material Symbols Rounded subset, its upstream Apache licence and explicit modification notice. This font is the O-112 exception to the unmodified text-font rule. Startup checks its fixed hash before registration; NOTICE §2.13 records its source and derivation limits.
