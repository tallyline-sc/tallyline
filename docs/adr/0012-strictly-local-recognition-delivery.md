# ADR-0012: Strictly local recognition supplied with Tallyline

> **Doc type:** Decision record — Accepted product constraints; runtime selection and delivery proof pending. Last reviewed: 2026-11-01.

- **Status:** Accepted constraints (2026-11-01); delivery clauses partially superseded by ADR-0013 (2026-11-04); locality, evaluation and remaining OCR delivery proof stay binding
- **Decider:** @greluc, in chat: no separately installed recognition programme, no later installation prerequisite, strictly local inference for privacy and without inference fees; compare complementary models and OCR readers on weak and strong hardware
- **Supersedes:** the remote-host/cloud consent option and user-installed Ollama prerequisite in R-VLM-6/7 and O-65
- **Related:** ADR-0005, ADR-0007–0009; R-VLM-5–12, R-NF-12/13; [runtime research](../local-vision-runtime-research.md)

## Decision

Recognition processes images and produces model answers on the user's computer. No LAN or cloud inference, paid inference API or consent override enables a remote recognition path. A loopback endpoint alone does not establish locality: model metadata, assets and runtime configuration must also exclude forwarding and fail closed when classification cannot be established.

An installation profile supplies its recognition engine, chosen model weights, projectors, dictionaries and other required assets as Tallyline components. Users need no separately installed Ollama, Python or model command line, and no later installation to make the selected profile usable. Additional optional model packs do not replace this baseline. Local AI stays optional and Off by default; usable manual entry and classic OCR do not depend on a VLM.

The engine is selected only after a reproducible comparison. A private bundled worker using a library is a candidate; a private bundled Ollama process is another candidate. Both require exact source, licence, native-scope, asset-integrity, lifecycle and network checks before adoption. Testing an existing local runtime is not approval to ship it.

Evaluate readers separately and in useful combinations, including additional classic OCR engines. Inputs and first answers remain independent: prompts contain neither reference transcriptions, priors nor another reader's answer. Recommend a combination only from measured error detection and joint-error behaviour, runtime and peak RAM/VRAM on its hardware profile. Agreement alone proves neither correctness nor calibrated confidence. Simulated limits on a strong machine are not evidence of a real weak PC.

## Delivery evidence still required

ADR-0009 currently makes the Visual C++ Redistributable a Windows OCR prerequisite. That path does not meet the owner's whole-application installation goal. Before release, a separately reviewed runtime/build or delivery decision must supersede that prerequisite and prove operation without a separate user installation. The existing prohibition on redistributing Microsoft's runtime files remains; this decision grants no proprietary redistribution or new native-code exception. Historical official-ORT benchmark results may still use their recorded test-machine prerequisites.

The research compares feasible routes; it does not establish a selected engine, packaged lifecycle, model accuracy or weak-PC support. The M2 runtime and M5 bake-off tickets remain open until their evidence exists.
