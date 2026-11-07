# ADR-0011: Source-compiled WiX 7 MSI packaging

> **Doc type:** Decision record — Accepted. Last reviewed: 2026-11-01.

- **Status:** Accepted (2026-11-01)
- **Decider:** @greluc
- **Supersedes:** ADR-0005 D7 only
- **Related:** ADR-0010, O-116, R-NF-2:installers, R-DOC-3, R-DOC-4, R-NF-4, R-SEC-8

## Context

The owner requires current stable dependencies and tools and selected independently compiled WiX 7.0.1. The latest stable cargo-packager 0.11.8 still invokes WiX 3 candle/light and offers no modern MSI backend or plugin. WiX 7.0.1 is the current stable upstream release; its official binaries carry an OSMF agreement which has not been accepted. Section 4 of that agreement permits independent source compilation under the MS-RL without the binary agreement.

## Decision

The Rust `xtask` packaging entry point invokes a prebuilt, source-compiled WiX 7.0.1 MSI CLI directly. Debian continues to use preinstalled cargo-packager 0.11.8, installed at the exact version with `--locked`. The plain ZIP and tar.gz remain Rust xtask outputs. No NSIS, licence acceptance dialogue, signing command, WebView download, arbitrary user authoring or packaging build hook is enabled.

WiX source is pinned to commit `642f1a0f7c857ffa4fc9fa3f9d1433aab895dd29`, its verified source archive and the retained [seven-file patch](../../supply-chain/wix/wix7-self-compiled.patch). The separately branded `TallylineSourceCompiledWiX` profile uses `TALLYLINE_SELF_COMPILED` to honour the independent-source exemption in the CLI and locally built source-package checks, with an explicitly empty acceptance property. `TALLYLINE_MSI_ONLY` excludes Burn registration and non-x64 native references. Upstream default behaviour is preserved. These changes implement the selected independent-source route; they are not acceptance of official binary terms. No acceptance flag, environment value or marker is used.

The profile is an MSI tool, with x64 native processing compiled from source and a private .NET runtime. It retains MSI schema, cabinet processing and default and explicit installer validation. The proprietary `mergemod.dll` is omitted. Module authoring remains technically present in upstream MSI core; valid Merge fails when that omitted helper is required. The product Rust wrapper therefore permits only its fixed generated and validated `Package` authoring and refuses `Module`, `Merge`, `MergeRef`, Bundle and arbitrary extensions. The tool is not described as a general authoring sandbox.

Secret-free preparation copies only immutable application resources, legal material, the prebuilt helper and previously verified tool inputs into a new stage, sealing their relative paths, modes and hashes. It never copies mutable application data or an existing token file. The protected phase executes that same prebuilt helper without Cargo, a compiler, source checkout, package hooks or downloads. Token data is added only after the seal is verified, at `tallyline-app-token` next to the actual executable. Exact fixture-byte scanning covers unpacked payloads and UTF-8/UTF-16 representations; logs and secret-free build outputs must remain free of the value. The actual UEX lexical format is not inferred from a real token.

## Provenance and consequences

The recorded local self-build uses isolated .NET SDK 10.0.403 and runtime 10.0.14, installed VS 17.14.44/v143 x64, source-only first-party NuGet packages, explicit isolated feeds and caches, dependency locks and verified signatures for 34 third-party packages. Signature and source-location verification is not a complete source audit. A build-only Microsoft.NETCore.Platforms metadata package has separate .NET Library terms and contains no executable; it is absent from the protected tool. No official first-party WiX binary is used as a bootstrap. Exact source, patch, archive/tool hashes and limits are in [dependency pins](../dependency-pins.md) and the [tool notes](../../supply-chain/wix/README.md).

Keep the original source, complete modified source, patch, MS-RL text, notices and build inputs with the source obligation record. Every actual MSI stream is inventoried rather than assuming linked build-tool assemblies are embedded. The application ZIP remains a separate output. No proprietary Microsoft runtime DLL is added to application packages. Native tool output is not bit-identical between local builds; pin the actual delivered tool bytes for each protected run and preserve the input/recipe provenance. Deterministic plain archives do not establish reproducible MSI bytes.

The self-built tool has passed harmless MSI build with default validation, separate validation and decompilation in an empty profile without an acceptance marker. Protected product packaging, every final payload, native installation and the new shared compiler pin still require the actual spike checks. This decision does not complete O-116 or a release.