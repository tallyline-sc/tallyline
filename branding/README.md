# Tallyline branding

The original geometric Line mark (concept C) is the owner-selected Tallyline app icon in ADR-0004 §1 and O-83. Its three rectangles, corner radii and plate are taken directly from AppIconConcepts in the supplied Tallyline-Design-System.zip. The archive's undecided-concept caption predates that decision.

## Files to upload to GitHub

- Organisation avatar: tallyline-avatar-dark.png (1024 × 1024). The light alternative has the same dimensions.
- Repository Social preview: tallyline-social-preview-dark.png (1280 × 640). The light alternative has the same dimensions.
- Both social previews are PNG files smaller than 1 MB, following GitHub's recommended dimensions: https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/customizing-your-repositorys-social-media-preview

The files have been prepared locally. No GitHub repository or organisation setting was changed.

## Other exports

- tallyline-logo-dark/light.svg and .png: square logo, original 32-unit geometry enlarged to 512 × 512.
- tallyline-wordmark-dark/light.svg and .png: logo plus Tallyline name, 900 × 192. The SVG lettering is outlined and needs no installed font.
- tallyline-mark-white/black.svg and .png: transparent monochrome marks.
- tallyline-icon-16/24/32/48/64/128/256.png and tallyline.ico: fixed-colour app icon sizes.

Dark colours: plate #253849, text #e8eef4, accent #7cc3ef, canvas #12202d. Light colours: plate #ffffff, text #15212d, accent #1f6fa8, canvas #f5f8fb. These are the supplied design tokens; the icon colours are fixed within each exported variant.

Typography uses the supplied, already approved IBM Plex Sans Regular/SemiBold and JetBrains Mono NL Regular. The PNGs and outlined SVGs contain no font software and do not require a font installation. Original marks and exported artwork: Copyright 2026 Lucas Greuloch, GPL-3.0-or-later, matching the Tallyline project. Typeface software remains under SIL OFL-1.1.

## Source identity

- Tallyline-Design-System.zip SHA-256: ac6f69b6ecaf22535978eec678de598f82c1db46ac1729fba82e4de3baf50207
- tallyline-egui.zip SHA-256: 787c87a0ac63b0a7a393a8fa8aadcc499026c778de3a2e0d1cef99ca6b66b90f
- Geometry: tallyline-design-system/components/AppIconConcepts/preview.html, concept C, unchanged.
- Vector and raster exports were made from the same geometry using local Windows drawing facilities; archive scripts were not executed.
## Native and packaged identity

Corrected 2026-11-03: these are durable project assets under the project GPL-3.0-or-later licence; complete terms are in [LICENSE](../LICENSE), attribution in [NOTICE](NOTICE), and exported byte identities in [exports.json](exports.json). The SVG artwork is the editable geometric source; its lettering is outlined. Archive captions and JavaFX instructions are historical references, subordinate to ADR-0004 §1 and O-112.

The native app embeds `tallyline-icon-256.png` and decodes it on its bounded resource worker for both native viewports. Its colours stay fixed when the app theme changes. Packaging copies exactly that PNG and `tallyline.ico` into `assets/branding/` beside the existing font and OCR directories. The Debian desktop entry names `tallyline`, installed as `usr/share/icons/hicolor/256x256/apps/tallyline.png`; Windows Start menu and installed-programmes metadata reference the ICO. This authoring does not establish rebuilt-installer or installed-system acceptance.

The 32-unit mark has a 32 × 32 plate with radius 6, then bars at (7,8) 18 × 3, (7,14.5) 14 × 3 and (7,21) 10 × 3 with radius 0.8. AppShell reserves its header for navigation and actions: no product logo is placed inside the shell. The Star Citizen Fan Kit unit remains separate, byte-exact and limited to its approved legal placements.