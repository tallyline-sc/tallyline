# Supplied panel preparation

> **Doc type:** Living reference — implemented source slice; automatic locate and calibration open. Last reviewed: 2026-11-05.

The model-free `recognition::api::prepare_panels` application interface accepts an immutable decoded `ImageRaster`, supplied shop/location/header quadrilaterals and constructor-validated tightenable limits. It produces read-only upright safe working rasters bound to observable forward/inverse projective transforms, or a typed geometry, privacy-extent, output or sampling refusal. Required privacy confirmation is derived from the supplied-geometry result and cannot be disabled by a caller. It does not run OCR or locate a terminal automatically. Manual and profile geometry remain supplied evidence and require the separate full-size R-SUB-7 privacy confirmation before upload; this tool neither uploads nor supplies that confirmation.

## Coordinates, filtering and privacy

Coordinates describe the raster actually decoded, including any intake reduction. Corners are clockwise top-left, top-right, bottom-right and bottom-left in raster-edge coordinates; the full raster has edges `(0,0)` through `(width,height)`. Output pixel centres are half-integers. The returned transforms map output-edge coordinates to source-edge coordinates and back. Tall panels remain upright. Working dimensions describe layout pixels, never physical resolution, source glyph height or improved recognition detail.

Corrected 2026-11-05: the supplied header describes the complete terminal header/balance envelope independently of balance text; it may start below the raster top. Its supporting bottom edge BL→BR defines `F(x,y) = dx*(y-BLy) - dy*(x-BLx)`, with `F <= 0` prohibited across the entire raster width, including outside the header's X extent. The header must be finite, in-raster, convex and clockwise TL/TR/BR/BL, with provable `TL.x < TR.x`, `BL.x < BR.x`, `TL.y < BL.y`, `TR.y < BR.y` and both top corners above the supporting edge. Unsupported, reversed, vertical or numerically ambiguous orientation is a Geometry refusal, without reordering or fallback. Mathematical validity cannot establish the actual terminal layout or detect every semantically incorrect annotation.

A source cell is the closed rectangle `[x,x+1] × [y,y+1]`. Because `dx > 0`, either top corner touching, entering or uncertainly meeting the prohibited half-plane makes the whole cell opaque black **before** filtering. Both top corners must be provably below to retain it. This includes bilinear neighbours outside the nominal crop and every area-filter source cell. A panel with no corner provably below the boundary is refused as PrivacyExtent; partly black panels remain possible. Enlargement uses the unchanged replicated-edge bilinear sampler; minification uses the unchanged projective source-cell area sampler. Source pixels, transforms and resource budgets are unchanged.

The private numerical enclosure rounds every subtraction and multiplication outwards to the adjacent representable binary64 values (`next_down`/`next_up`). Exact input coordinates start as singletons. A difference encloses the rounded result between its two neighbours; interval products take the extrema of all four endpoint products and expand both endpoints; final subtraction uses opposite endpoints and expands again. Induction over these operations encloses the real cross product on the supplied f64 operands, including cancellation and rounding of the bounds themselves. Adjacent-value expansion also encloses underflow through zero and subnormal values, unlike a relative-only epsilon. No fixed tolerance, slope division or final-result-only bound is used. Nonfinite arithmetic cannot prove safety; construction refuses nonfinite enclosures at source-domain extremes before sampling. Valid u32 raster coordinates and differences have magnitude at most `2^32`, and products at most `2^64`, far inside f64's finite range; interval expansion leaves ample range. The public thin-raster fixture actually allocates 262,144 × 8 pixels and tests translated small offsets; it does not claim a valid near-u32-sized allocated raster. The caller must independently review all supplied extents.

Shared projective mathematics and sampling live in `shared-kernel`. `DetectionBox::rectify` uses those same primitives while retaining its source-scale, integer-centred text-line samples and anticlockwise tall-line rotation. Recognition lines still come from the original raster; normalised panels are layout evidence only. The existing pixel and BGR tensor goldens remain unchanged.

## Offline crop dump

Run the existing eval package with explicit files and a new absolute output directory:

```powershell
cargo run --locked --offline -p ocr-eval -- prepare-panels --image <capture> --geometry <supplied.json> --output <new-absolute-directory>
```

Keep scratch inputs and dumps under the task's `D:/Coding/Agents/Temp/codex/` directory on the owner's Windows machine. No image or raw pixel data is printed. The small stdout receipt reports one capture, two crops, automatic locate not run and recognition not run. A successful dump contains only `shop.png`, `location.png` and `geometry.json`. The directory is created exclusively beneath an existing directory without links/reparse points; an existing destination is refused. Unix directories are owner-only; Windows inherits the selected private parent's ACL. An output failure removes the newly owned files/directory or reports incomplete cleanup. Encoding and byte-budget failures occur before directory creation.

The independent input manifest is separate from `expected.json` and has exactly these fields; replace the hash and coordinates with reviewed evidence:

```json
{
  "schemaVersion": 2,
  "imageSha256": "<64 lower-case hexadecimal characters>",
  "rasterDimensions": [8, 8],
  "provenance": "manual",
  "shop": {"corners": [[2,2],[6,2],[6,6],[2,6]], "output": [4,4]},
  "location": {"corners": [[0,2],[2,2],[2,5],[0,5]], "output": [4,6]},
  "header": [[0,0],[8,0],[8,2.25],[0,2.25]],
  "limits": {
    "maxSide": 64,
    "maxTotalPixels": 1024,
    "maxSampledPixels": 4096,
    "maxOutputBytes": 65536
  }
}
```

`provenance` accepts only `manual` (explicitly supplied geometry) or `profile`; neither means owner approval or automatic anchor/frame success. The exact geometry file hash binds the supplied conditions. `imageSha256` binds the exact source bytes passed to the existing bounded decoder; `rasterDimensions` must match its decoded raster. Input and output use schemaVersion 2, with output policy `full_width_upward_through_header_supporting_bottom_edge`. Schema-one input is refused with explicit regenerate-and-review guidance. Historical schema-one sidecars, hashes and dumps remain immutable evidence: regenerate versioned source-bound geometry and review it; changing only the version does not validate a proposal. Output metadata preserves source/geometry hashes, intake reduction provenance, supplied corners/header, the full-width upward-closed mask policy, output sizes and both homogeneous matrices. Pixel coordinates, original source-scale evidence and normalised layout pixels remain separate.

Each invocation admits exactly one capture, two crops and one metadata file. Geometry input/output metadata is bounded at 64 KiB; image input uses the existing 200 MiB/100 MP intake limits and 20 MP decoded pixel budget. All supplied limits must be positive and may only tighten the initial hard bounds: 4,096 output pixels per side, 4,194,304 combined output pixels, 128,000,000 candidate source cells across both filters and 32 MiB combined PNG/metadata bytes. These are bounded instrument defaults, not calibrated panel-size or performance acceptance. The current command does not batch a corpus or choose working dimensions from a calibrated reference-glyph profile.

## Evidence and remaining gates

The tool explicitly reports text size as unavailable/not measured and tone as unavailable/uncalibrated. Missing source price-cap measurements, profile SDR references and paired tone thresholds cannot become absolute-RGB classifications, clipped-highlight findings on ordinary SDR glyphs, resolution coverage or confidence changes. Images, Golden values, verified flags, Fan Kit assets, licences and dependency pins are unchanged.

The next meaningful source step is bounded full-width coarse/native tile scheduling with source-coordinate mapping and a native retry decision, followed by anchor-constrained luma-gradient frame fitting. Automatic anchor OCR, frame edges, profile target-size selection, viewport/card layout and complete-card text measurements remain open. Tone/reference calibration, confidence, repair, fusion, stitching, Scan persistence and capture lifecycle are outside this source slice.

Before a new raw-read sweep, independently place/review geometry and inspect dumps for **all 174 available public captures**, recording missing/failed/refused cases rather than selecting successful crops. Start development review with all 89 complete/partial observations in the 19 MIC-L2/Orbituary additions (62 complete, 27 partial); they are inspected development data, not an untouched holdout. Cargo-only clipping stays partial. No corpus-wide crop review or model inference has been performed by this implementation, and no geometry proposal is automatically promoted to verified Golden data.