# Approved native runtime data

These are the unchanged official Microsoft ONNX Runtime1.31.0 x64 libraries selected by ADR-0007. The Windows and Linux archives were hash-checked before extracting only the fixed regular library files and their complete licence/notice texts. Archive symlinks, provider libraries, Microsoft CRT files and installers are not included.

The self-contained build script hashes both native files before compilation. The loader selects only the current platform's fixed filename next to the actual executable, verifies its bytes and host policy, and never consults ORT_DYLIB_PATH or downloads a replacement. The files in this source directory are build inputs; application packaging must place the matching file next to the executable. A developer probe directory does not establish clean-OS/package acceptance.

The project's licence is MIT; each upstream release also includes a broad ThirdPartyNotices.txt, preserved unchanged under supply-chain/notices/onnxruntime-{windows,linux}. Its optional-component list is not proof that every listed option is linked into these fixed CPU libraries. Corresponding-source and release/legal acceptance retain the existing ADR-0003/O-12 obligations. No new provider, native build or legal decision is selected here.
