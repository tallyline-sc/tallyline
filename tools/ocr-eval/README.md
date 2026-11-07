# Corpus inspection and annotation

The `ocr-eval` workspace package inspects schema-version-two corpus entries, image availability, expected values, conditions, measured bands and frozen reference metadata. Run it with `cargo run -p ocr-eval --locked -- inspect`; use `--public <directory>` to inspect another public fixture directory. Private inputs come only from `TALLYLINE_CORPUS_DIR`.

`cargo run -p ocr-eval --locked -- annotate <expected.json> <anchors.json> <proposal.json>` writes a separate, exclusive-create proposal. See [the contributor capture guide](../../docs/user/corpus-capture.md) for anchor coordinates, privacy rules and report interpretation. Fresh recognition, reference snapshot replay and held-out accuracy measurements remain separate roadmap work.

## Explicit Game.log witness inspection

`cargo run -p ocr-eval --locked -- witness --channel <directory>` explicitly opts into a local read-only inspection of the selected channel's manifest and Game.log. It prints sanitised facts separately from corpus accuracy output; it is not recognition, a Reader or a submission path. The output uses closed reasons for unavailable inputs and a finite build/mapping allowlist. It contains no raw lines, source path, player fields, prices, quantities or status values. See [the validation record](../../docs/game-log-validation.md) and [the desktop guide](../../docs/user/desktop.md).

## Isolated Linux complementary OCR research

The developer-only `tesseract-research` binary and public safe Rust `isolation-probe` belong to this package. They add no application backend or new dependency. See [the controller record](../../docs/tesseract-linux-research-controller.md) for exact approval hashes, CLI sequence, root-only native execution gates, private evidence, licence/native boundaries and resource limitations. `cargo run -p ocr-eval --bin tesseract-research --locked -- contract deadline` runs a recorded own-process contract without a model. Specifying `--bin ocr-eval` continues to select the existing corpus CLI when using `cargo run`.

The [completed Linux pilot](../../docs/local-ocr-tesseract-experiment.md) separately records source-b019's native build, four blank-image gates and twenty fixed PSM 11 reads. It retains literal post-hoc field errors and complementary readings, with unchanged references and no calibrated accuracy or production-profile claim.

## Explicit SCU policy sensitivity

`cargo run -p ocr-eval --locked -- scu-policy --public corpus/public` reads only expected metadata and prints input hashes, complete whole-SCU quantity counts and finite synthetic digit-edit comparisons for 30 explicit percentage/absolute candidates. It uses no images, inference, HTTP or private corpus variable, never writes annotations and never reports recognition accuracy. It separately counts repeated rows and distinct scalar quantities. Limits are 64 expected files, 4 MiB per file and 65,536 cards; unsupported quantity types are refused visibly. See [the provisional calibration note](../../docs/scu-policy-calibration.md) for the 20 % / 16 SCU recommendation, its worked cases and empirical limits. Production fractional quantities are unaffected by this whole-quantity research mode.

## Retained-reader replay

`cargo run -p ocr-eval --locked --offline -- reader-replay --public corpus/public --packet <file> --sha256 <digest>` explicitly reads a prepared, immutable local observation packet. It emits aggregate single-reader, all-pair and all-triple unanimous-agreement counters, including wrong/invalid, missing and original alignment-unscorable outcomes. It performs no inference, HTTP or annotation write and does not read `TALLYLINE_CORPUS_DIR`. See the [verified-reference development reassessment](../../docs/local-reader-verified-reassessment.md) for the frozen policies, private evidence pointers, exact input/result hashes and interpretation limits.

The packet is schema version 1 with `readers` (unique identities in slot order) and `rows`. Each row names `entry`, `capture`, `screen_order`, `field` (`pricePerScu`, `scu` or `status`), `reference_sha256` and exactly one `observations` slot per reader. Current reference values are read from the verified corpus by this exact identity; duplicate fields, partial/ambiguous cards, changed hashes and omitted slots are refused.

Every observation preserves `literal` and `normalised_literal` as strings or explicit nulls. `outcome` is `value`, `invalid`, `missing` or `unscorable`. A supported value additionally has a canonical `value`: exact non-negative decimal text for its field's already checked units, or the original normalised status literal. Invalid nonempty outputs count wrong and remain in the denominator; matching invalid strings are never emitted. Missing slots require null/empty literals. Unscorable requires `reason: alignment`, or `reference_unknown` when the current reference is unknown; malformed output is not an unscorable reason. Exact decimal comparison removes representational leading/trailing zeroes without floats; it never parses or repairs raw OCR units or glyphs.

The canonical packet is trusted adapter evidence. The CLI does not prove that its raw literals were parsed or aligned honestly; freeze and audit that adapter and its inputs before interpreting scores. All first answers, alignments, parser policies, profile combinations and eligibility must be fixed separately. A packet hash is an identity check, not independent verification, held-out accuracy, calibrated confidence or permission to increase sendability.

Bounds are 16 MiB per packet, 4 MiB per reference, 4096 unique fields, 2–16 readers, 64 referenced entries, 4096 bytes per literal and 128 bytes per canonical decimal. Reference decimals must be exact strings or unsigned JSON integers; fractional JSON floating-point values are refused. Output includes source hashes, field/card/capture counts, per-field and per-capture counters and every unordered pair/triple; generated results and raw private packets belong outside the repository.
