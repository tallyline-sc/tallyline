# OCR loader fixture failure record

> **Doc type:** Living reference — recorded. Last reviewed: 2026-11-02.

The independent audit of hosted mutation run `41434761330` at integration revision `d78f5b7b5e2332a9f52151141c74e4de7edd0dbb` found 23 nominally caught variants with `ExecutableFileBusy` / `Text file busy` in the OCR fixture's child start, before the intended core test result. Those outcomes do not establish genuine mutation detection. The separate cause guard must reject such evidence; passing the fixture later does not repair that historical measurement.

## Observations and limits

A dependency-free Cargo reproduction in the existing pinned Rust 1.99.0 Linux SDK copies its own executable, holds the copy open for writing and attempts to execute it. The kernel returns error 26 (`ETXTBSY`). Closing that writer permits execution. This is a deterministic reproduction of the writable-inode mechanism, not proof that a particular historical sibling fork inherited that descriptor. An attempted hard-link alternative failed its safety assertion on the Windows-backed Docker filesystem: opening the link for writing succeeded. It is not used.

The already verified Rust source archive has SHA-256 `3f1f9b7ed48f4596fc87889b7b3c61747336a55c9c22db1ab0c697e0aadb77aa`. Within its `library/std/src/`, `sys/fs/unix.rs` lines 2190 to 2199 own the copy writer through the copy call. `sys/process/unix/unix.rs` lines 55 to 165 use `posix_spawn` or a fork/exec error handshake. These sources support a single fixture publication boundary; they do not identify which spawning path or writable handle caused the audited failure.

A separate Windows mutation log reports linker `LNK1104` for `image_resources-80e2b2eb1729ac63.exe`. The inspected `adapter-files/tests/image_resources.rs` contains three ignored resource contracts and no process creation. No orphaned child, antivirus intervention or common cause has been established. That outcome is not a genuine Rust compile rejection or a valid completed mutation measurement.

## Correction and regression evidence

The runtime fixture serialises executable copy, sibling preparation and process creation with every probe spawn, including the startup-mitigation probe. Successful spawn releases the publication lock before waiting and collecting output; standard input remains closed. Tests still use their ordinary concurrent harness settings. There is no retry or suppression of a failing result.

The owned fixture now cleans up after executable-copy, sibling-setup and spawn failures as well as child failure. The original I/O kind and diagnostic remain primary; any cleanup failure is secondary. A real missing-source contract first failed because its directory remained, then passed after correction. A real missing-executable spawn contract also verifies `NotFound` and removal. The existing deliberately failing child preserves its own diagnostic.

All eight focused contracts pass on Windows and the pinned offline Linux SDK, including simultaneous prepared and startup probes (24 children in that concurrent contract). Every loader child asserts `native_load_attempted() == false`; verified bytes reach only the injected search-policy refusal. No ONNX Runtime or model is initialised. These passing contracts support the bounded fixture correction, not a claim that the intermittent historical race has been deterministically reproduced or that later mutation measurements are valid automatically.

No dependency, feature, native/assembly scope, asset identity, product loader policy or accepted ADR changes. Fresh cache, target and temporary outputs are confined to the assigned Agents temporary root; approved vendored sources are read-only. Full canonical verification and signed handoff are recorded separately after the automatic output-root support is available.
