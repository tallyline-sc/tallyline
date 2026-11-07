# Ollama v0.40.13: historical build-output provenance follow-up

> **Doc type:** Historical record — static evidence; runtime admission unresolved. Last reviewed: 2026-11-07.

**All 30 retained Windows members match exact adjacent post-signing SHA-256/Path records in the official packaging log, including all 25 candidate native members and the CLI.** The retained files total 83,611,431 bytes; the candidate totals 83,029,576 bytes. Bounded inert-file rehashes agree with the [published static baseline](ollama-v0.40.13-windows-amd64-static-evidence-v01.json), the completed research receipt and the log records. The [claims and proof mapping](ollama-v0.40.13-authenticity-follow-up-v01.json) includes every member's size, digest and original line ordinals, with primary URLs and evidence limits. It includes no runtime payloads or raw logs.

## Observed bridge

The [official run](https://github.com/ollama/ollama/actions/runs/40804853596), attempt 1, reports source commit 04088e7c9d41cba5d33bafbf788d87a465c79d5b. Its [packaging job](https://github.com/ollama/ollama/actions/runs/40804853596/job/123227543258) logs signing at 02:35–02:36 UTC on 2026-10-29, then hashes the retained members at 02:40:13 UTC. The CLI's adjacent records are lines 7742–7743. Its SHA-256 is c1cca2937421d78877d5961746d25e33acd62e194d881f90fb63dca7f3e10f41, size 40,931,208 bytes. Hashes were paired only with the immediately following exact Windows-amd64 archive path; each retained path has one matching record.

The [fixed workflow](https://github.com/ollama/ollama/blob/04088e7c9d41cba5d33bafbf788d87a465c79d5b/.github/workflows/release.yaml#L398) explains the post-signing hash step. The [release job](https://github.com/ollama/ollama/actions/runs/40804853596/job/123231303644), packaging log and [asset metadata](https://api.github.com/repos/ollama/ollama/releases/assets/687142233) agree on ZIP digest 7afa38624fa62ee5d8293422d0ab084c1bb4603134c7f4dafce11a033b57cc19 ; the asset metadata reports 1,601,969,847 bytes. The whole ZIP was not acquired or rehashed; its digest alone does not authenticate partially acquired members.

This is **project-controlled provenance evidence**: source-associated official build-output consistency transported through GitHub. It is stronger than a recipe or self-reported executable metadata. It is not independently verified signed provenance, local Authenticode verification or a reproducible source-to-binary result. A signing-success log is not local signature verification. Run/job identifiers identify the observed records; they are not cryptographic immutability guarantees. The [release metadata](https://api.github.com/repos/ollama/ollama/releases/427123529) reported immutable=false at the research observation.

## Bounded build observations

The [CLI job](https://github.com/ollama/ollama/actions/runs/40804853596/job/123221275658) reports Go 1.26.10 and CGo enabled. Setup-time go env reports CC=gcc, CXX=g++, C/C++ flags -O3, empty CPP flags and linker flags -O2 -g. The build step also prints C/C++ -O3. Compiler output reports Clang 18.1.8, LLVM source 3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff; the workflow names llvm-mingw-20240619-ucrt-x86_64.zip. Those compiler archive bytes and hash were not verified.

The [CPU dependency job](https://github.com/ollama/ollama/actions/runs/40804853596/job/123221275826) reports Clang 18.1.8 C/C++ and Clang assembler, Release configuration and OpenMP -fopenmp=libomp 5.1. The [fixed CLI build function](https://github.com/ollama/ollama/blob/04088e7c9d41cba5d33bafbf788d87a465c79d5b/scripts/build_windows.ps1#L896) specifies trimpath and stripped release/version linker options. These observations do not recover every effective native flag, build tag, patch, object input or exact toolchain byte identity. Setup-time environment is not a trace of every compiler invocation; trimpath does not supply missing native metadata.

The [existing CGo/SQLite/notice delta](ollama-v0.40.13-cgo-sqlite-notice-delta-v01.md) remains the distinct source/module/notice record. This follow-up does not complete native/assembler scope, incorporated notices, provider resolution or dynamic loader closure. Prior Rust SQLite and ORT approvals do not transfer to this candidate.

## Attestation retrieval and stronger optional evidence

Two authenticated, bounded GETs of the documented organisation attestation endpoint, one for the actual CLI digest and one for the published whole-ZIP digest, returned HTTP 404 on 2026-11-06. Neither returned an accessible bundle; neither followed pagination or reached its timeout/output cap. The machine-readable mapping preserves sanitised receipt fields, response sizes and hashes; response bodies and authentication material are excluded. These results establish only that those checks obtained no accessible bundle. They do not prove that no attestation exists.

A verifiable upstream signed manifest or provenance bundle naming the actual retained member digests could strengthen the evidence. Verification would need to establish the publisher/workflow identity and the source/build claims actually signed. A whole-ZIP subject additionally needs a separately established member binding. Provenance can still omit exact toolchain bytes or CGo/native flags. A controlled rebuild is a separate candidate-building task and cannot retrospectively authenticate different signed bytes merely because they share source.

## Existing decision and trial boundary

This historical follow-up selects no package, runtime, model or delivery route and adds no mandatory cryptographic-attestation tool or native gate. [ADR-0013](../../adr/0013-conditional-ollama-delivery.md) and [S-23–S-27](../../plan/10-supply-chain-security.md) remain controlling. For this candidate, exact native/source/licence/integrity qualification, private-child ownership, strict locality, a hash-bound complete profile and a separately reviewable trial remain open. User installation only avoids Tallyline's redistribution of those runtime bytes; it does not bypass execution admission, private ownership or integrity controls.

The [synthetic trial capsule checklist](ollama-v0.40.13-synthetic-trial-capsule-checklist-v01.md) identifies the concrete missing fields without inventing a model or runnable command. No Go, Ollama, model or native payload was executed by this follow-up. No runtime admission, recognition accuracy, complete installation acceptance or physical weak-PC support follows.

Added 2026-11-06: this record narrows earlier missing-per-member-build-output-evidence statements. The original partial-archive evidence and CGo/notice delta remain historical records of their own observations.

Corrected 2026-11-07: the [Windows owned-process primitive](../../owned-process-contract.md) supplies suspended creation, bounded job/pipe lifetime and harmless cleanup contracts through the first twenty approved Win32 APIs and four exact feature additions. The twenty-first TCP ownership API/IpHelper feature remains inactive. This source primitive does not establish this candidate's listener ownership, executable/native/profile admission, controller or Workflows/App composition, inference or product activation.
