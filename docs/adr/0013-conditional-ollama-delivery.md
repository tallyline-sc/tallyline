# ADR-0013: Conditional Ollama delivery and requested model downloads

> **Doc type:** Decision record — Accepted conditional product policy; engine, payload and implementation unselected. Last reviewed: 2026-11-04.

- **Status:** Accepted owner constraints (2026-11-04); exact runtime and delivery acceptance pending
- **Decider:** @greluc, in chat: Ollama and its model downloads are permitted if custom implementation becomes excessive; user installation is permitted as a last resort if bundled delivery cannot be implemented acceptably
- **Supersedes:** ADR-0012 only for its absolute no-separate-Ollama/no-later-setup clauses and unconditional complete-initial-profile delivery promise
- **Related:** O-65; R-VLM-6/7/11/12; ADR-0005/0007/0008/0009; S-23/24/25/27

## Decision

Prefer supplying the selected recognition runtime and a complete offline-ready profile with Tallyline. If the custom integration cost is excessive, Ollama may supply local inference and model management. Prefer a bundled private CLI; only if bundled delivery cannot be implemented acceptably may the selected CLI instead come from a user installation. Explicitly requested model downloads through Ollama may supply the first selected AI profile as well as additional profiles. Before acquisition completes, AI is unavailable; do not promise a usable initial AI profile offline on that route. This conditional policy is already authorised and requires no repeat policy-permission request.

No engine, version, archive, model, registry, native exception or final installation route is selected by this decision. Record the evidence for invoking the fallback and the chosen route with its exact admission record. The source, native, licence, provenance, integrity and network gates still precede adoption and execution; existing task authorisations for read-only public source/payload acquisition and hashing remain valid, subject to their source, licence, integrity and privacy controls. This policy grants no additional authority to install programmes, execute unapproved native components or alter foreign state.

Strictly local inference and no per-use inference fees remain binding. Crops, prompts and answers stay on this computer; no LAN/cloud inference or consent override is introduced. A loopback address alone is insufficient. Disable cloud execution and fail closed on unknown, remote or changed runtime/profile identity. AI stays optional and Off by default; manual entry and classic OCR remain usable without it. Missing, changed or incompatible AI pauses its queue with an actionable Settings state and never causes a startup error.

For either Ollama delivery route, launch the admitted CLI by verified absolute path as an app-owned private child with its own home/profile, keys/configuration, model store, cache, logs, temporary roots and literal loopback endpoint. Confirm endpoint ownership through the OS. Never attach to, stop, reconfigure or reuse a foreign listener, service, home or model store, and never change global environment or updater settings. Recheck the actual executable/native closure and complete evaluated profile before use; installed-programme replacement or updater drift pauses AI. Process ownership, cancellation, unloading and whole-process-tree resource release need acceptance evidence.

Model acquisition is separate from inference. Before enabling it, record fixed approved registry/CDN/authentication/redirect destinations and TLS policy, every manifest/config/layer/projector/template/runner identity, licence, disk/temporary-space requirement and deletion/rollback behaviour. Show sources, metadata disclosed (including source IP, requested model and client identity), bandwidth/storage needs, progress, cancellation and supported resume before the explicit request. No capture, prompt, answer or UEX credential belongs in a download request. Mutable tags and a successful upstream pull do not establish an evaluated profile. No silent model refresh, runtime-code download or automatic runtime adoption is allowed.

## Unchanged constraints and remaining evidence

ADR-0012's locality, independent-reader evaluation and measured hardware-fit requirements remain binding. Compare useful single readers and pairs/triples without supplying reference answers, UEX priors or another first reader's answer; measure shared errors, invalid outputs, abstention, coverage and review burden. Strong-host limits and 4/8-GiB VMs do not prove physical weak-PC support. Resource knobs do not establish hard CPU/RAM/VRAM caps.

ADR-0009 remains unchanged: Tallyline may not ship Microsoft's runtime DLLs or redistributable installer. A user-installed Ollama does not authorise copying its DLLs or broadening Tallyline's DLL search and does not satisfy or remove the separate ORT prerequisite. ADR-0012's remaining whole-application OCR delivery blocker still needs its own verified resolution. No proprietary redistribution, cloud-privacy consent or additional native exception is granted.

Clean-install/offline behaviour, actual Windows/Linux payload closure, private paths/listener ownership, model-download destination policy, replacement races, five-second cancellation/unloading and measured hardware/recognition acceptance remain open. This decision changes the permitted product policy only; it implements no listener, download, store, UI or recognition backend.
