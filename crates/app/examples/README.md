# Headless OCR native diagnostic

`ocr_runtime_probe.rs` is an explicit developer composition-root example. It prepares telemetry before one named, joined below-normal CPU worker and owns one non-clone native load token. It opens no desktop shell, elects no instance and reads no user application storage. It uses only the exact accepted library and explicitly selected hash-bound OCR data.

Build with `cargo build --locked -p tallyline-app --example ocr_runtime_probe`. Stage the executable in an owned installation directory with the matching regular library beside it (`onnxruntime.dll` on Windows, or the versioned Linux file after host verification is implemented). Put the three unchanged `crates/adapter-ocr/assets/small/` files in `models/small/`. Preserve the complete runtime/model legal texts. Runtime paths selected from the environment, shared extraction, extra providers and shipped CRT files are excluded by the existing boundary.

The explicit modes are:

- `--load-only`: verify private loading and mapped CRT prerequisites.
- `--small-tensors`: run normalised-zero detector 64×64 and recogniser 48×320 tensors with numerical output evidence.
- `--small-reader-selftest`: use a fixed grey synthetic raster to exercise recognition repeat, detection, both session types resident, monotonic-clock refusal, inactivity boundaries, session reopening and explicit release. The app load token must still refuse another native load.
- `--small-reader <absolute-crop-path>`: detect text boxes and recognise their original-raster crops sequentially.
- `--small-textline <absolute-line-path>`: recognise one known original-raster text line.

Image modes accept only already balance-free, hash-bound development inputs staged by the operator. Intake decoding, hashing, native inference and geometry stay on the owned worker; no raster is prefetched. Images requiring detector tiling are refused by this checkpoint. Raw-image and line modes return profile, dimensions, counts and residency status on stdout, without raw text or image paths.

For explicit private evaluation, append `--raw-output <absolute-new-private-json-path>` to an image/line mode. The example renders public OCR text, source corners, detector probability when applicable, and CTC label probabilities/frame spans as JSON. The Files adapter writes the supplied bytes exclusively, never overwrites an existing file/link and reports closed errors. Unix files are created with mode 0600; on Windows select a private directory with the user's inherited ACL. A failed write may leave an incomplete private file, which is not evidence. The output does not claim a checked source-image hash: the operator's staged-input receipt binds the immutable development input and the executable/runtime/models. Raw output is an explicit developer artifact, not a production log, Capture validation result, field confidence, fusion answer or changed privacy-release workflow.

The typed raw-reader profile `ppocr-small-portable-db-bilinear-v1` records the safe Rust geometry and bilinear crop variant. Its complete bounds and source-qualified differences from OpenCV/pyclipper arithmetic are in [07](../../../docs/plan/07-ocr-concept.md). The recogniser keeps `ppocr-bgr48-linear8u-v1`; hashes and dictionary ordering are unchanged. Sessions are lazy and reused, with typed intra-op threads and explicit inactivity cleanup retaining the verified environment. The worker owner must schedule its idle tick; this example creates no pool, timer or extra runtime.

A successful native probe is bounded developer-host evidence. Corpus accuracy, packaged Linux telemetry controls, clean-OS/no-prerequisite delivery, native thread counts, process memory, production Ready admission/timer/cancellation/tiling and the complete capture pipeline remain separate acceptance work. Linux currently refuses before native loading until its host proof is implemented. Existing older native observations are bound to their original executables; they do not establish this reader's behaviour.

## Explicit complementary research modes

The same Cargo example accepts nine bounded typed detector/recogniser pairs, without replacing Small filenames or changing the runtime loader. Choose `tiny`, `small` or `medium` independently for `<detector-family>` and `<recogniser-family>`:

```text
cargo run --locked -p tallyline-app --example ocr_runtime_probe -- --research-reader-selftest <detector-family> <recogniser-family> <absolute-detector-onnx> <absolute-recogniser-onnx> <absolute-canonical-dictionary>
cargo run --locked -p tallyline-app --example ocr_runtime_probe -- --research-reader <detector-family> <recogniser-family> <absolute-detector-onnx> <absolute-recogniser-onnx> <absolute-canonical-dictionary> <absolute-frozen-crop> --raw-output <absolute-new-private-json>
cargo run --locked -p tallyline-app --example ocr_runtime_probe -- --research-textline <detector-family> <recogniser-family> <absolute-detector-onnx> <absolute-recogniser-onnx> <absolute-canonical-dictionary> <absolute-frozen-line> --raw-output <absolute-new-private-json>
```

Stage the exact approved native library beside the executable even when invoking through Cargo; the model paths supply data only. The operator supplies already downloaded weights matching the [fixed catalogue](../../adapter-ocr/assets/research/provenance.json), not arbitrary ONNX or a plugin. Normal builds/CI never download experimental weights. Tiny uses `assets/research/ppocrv6_tiny_dict.txt`; Small/Medium use the unchanged `assets/small/ppocrv6_dict.txt`. Paths need not use Small filenames. Each file is size/SHA-256 verified once and its exact model snapshot retained before session creation; later path changes cannot replace the native input.

The research selftest uses the same fixed grey raster as the existing Small selftest. It checks repeat equality, both sessions resident, monotonic refusal, idle boundaries, reopening, explicit release and the retained single-load token. Shape/probability/alphabet checks remain in the actual native reader; the separate frozen crop runs exercise nonconstant image content. This is synthetic lifecycle evidence only. The existing Small selftest/input, identifier and raw schema remain unchanged.

Research stdout includes only dimensions/counts/lifecycle and public profile/model/dictionary identities; the private JSON adds the same identities beside unmodified raw text, corners and CTC evidence. No image path or text goes to stdout. In textline mode the receipt still identifies the complete selected pair, but detection is not run; session residency shows that distinction. Resource receipts must count retained verified model snapshots as well as native session memory.

Run individual Tiny/Tiny, Small/Small and Medium/Medium first on the same frozen balance-free crops, then explicit cross pairs to isolate localisation from recognition changes and resource trade-offs. Nine pairs contain only three recogniser weight sets, often with shared detections and related family/training; they are not nine independent witnesses or majority votes. Compare independent OCR/VLM first answers separately without feeding another reader's answer, priors or reference transcriptions into inference. Do not tune against these five development cases, repair digits, uplift confidence or treat agreement as truth. Preserve the source-input/executable/runtime receipt and every raw profile identity; no golden flag or measured field-accuracy claim follows from this checkpoint.

The completed 2026-11-02 Windows run is recorded in the [nine-pair experiment](../../../docs/local-ocr-nine-pair-experiment.md), bound to its frozen source and executable. All ten lifecycle and 50 image phases succeeded; the earlier gradient-fixture campaign remains incomplete. Its process samples and post-hoc literal comparison establish development evidence only. The product default remains Small and production scheduling/field parsing remain open.
