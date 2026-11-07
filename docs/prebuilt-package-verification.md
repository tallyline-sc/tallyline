# Prebuilt package verification

> **Doc type:** Historical record — completed development experiments; current release and installation acceptance pending. Last reviewed: 2026-11-03.

## Historical Development fixture

The first complete application-package experiment uses the actual Windows and Linux Development `0.0.0` applications built at source revision `10ec53bcf6a479eee4d0a0ce4ec25c37f993e167`. These are not current-head applications, authenticated releases or signed-tag evidence. The separately verified source capsule has SHA-256 `17eb58c11357cf5c09399a8acce6cfabbbfaddd4afb85caa1860146c7630797b`; its project, locked vendor and reviewed native/source-duty inventory was verified before fixture insertion.

| Artefact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows MSI | 40565365 | `278e387f4936a188736cb46b6311b41b3fde6771599e80280e1827c24a69473f` |
| Windows ZIP | 83414056 | `d4d245c24f7cbbad3c9cb76a7a694f2379f6d6c3ee252f3b0fc647ca5d0dbd9f` |
| Linux deb | 52302710 | `c389eff19a9c9e8f7c55e0afa308c54b508240edc81ed674a2850079a3c4b5bd` |
| Linux tar.gz | 51998080 | `d2971405b9620c1844ed7e40895a1358759033d5fdb5cde6d3fe51a2efcc56df` |

Both native assembly jobs exited successfully using verified prebuilt helpers and approved tools. Fresh harmless fixtures were created after compilation and inserted only as the separate token data file. No real credential or UEX token was read. The fixture satisfies the application's graphic-ASCII envelope; the real UEX lexical format remains unverified.

The Linux protected container used the existing runtime image `sha256:c84a0d691a0b1f80e3523e0c450c3ad45ec53f00946847a924d5488dc96331c2`, network mode `none`, a read-only root, dropped capabilities and no-new-privileges. Its only mounts were the sealed stage read-only, fixture read-only and owned output writable. No project source, source capsule, Cargo cache, Cargo or compiler was mounted or available in the audited runtime image. The same prebuilt helper performed insertion and final assembly; hooks were null and packaging tools were sealed before fixture access.

The Windows helper ran outside the checkout with a cleared environment and empty outer PATH; its handoff contained only approved data and prebuilt code. Its checked WiX child used the private runtime and fixed system-tool path. This does not prove operating-system denial of host source/compiler access or networking. That Windows isolation gap remains open.

## Independent inspection

The ZIP reader checked all 428 regular payload files against the sealed inventory, including exact bytes, safe unique paths, normalised times and modes. A separate read-only Windows Installer query found 428 files/components, x64 component flags, zero custom actions, zero embedded storages and only the cabinet and summary-information streams. An independent cabinet parser verified every member name and size before the built-in cabinet reader extracted them; all extracted hashes matched. The private fixture occurred only in its designated data file. Retained text/logs were clean, and intermediate cabinets matched the independently checked cabinet.

The actual MSI's 362 launch conditions include the 361 authored CRT rows and standard downgrade refusal. Restricted read-only engine evaluation passed 467 canonical positive/negative cases, totalling 169054 actual condition evaluations, with zero mismatches. Stored messages include the official Microsoft download page. This did not execute AppSearch, installer action/sequence APIs, installation or UI. A forward-slash controller path was refused; independently normalising the Windows path fixed the controller, and both attempts remain in the external evidence.

Independent ar/tar readers checked the Linux payloads, root ownership, fixed timestamps, data/control modes, sole fixed `usr/bin/tallyline` symlink, every control checksum and the 114292 KiB installed-size accounting. The Debian control archive contains only control metadata and checksums, with no maintainer scripts. The standard `dpkg-deb --info` reader also accepted it. All 271 component notice sets and their source scopes survived DEP-5 folding; original delivered legal-file hashes also matched. This is not Debian archive-maintainer or clean-OS installation acceptance.

Actual prebuilt scanner calls accepted independent ZIP/cabinet extractions and refused controlled bare, UTF-16LE and UTF-16BE leaks without disclosing fixture bytes. Repeated plain ZIP and tar.gz assembly from the same payload and fixture produced byte-identical archives. This establishes neither compiler nor MSI reproducibility. Corrected 2026-11-02: known compiled apps may execute the bounded headless metadata query during secret-free preparation; no desktop bootstrap, model execution or installation occurred.

## Metadata corrections

The historical MSI build exposed unnecessary `DefaultLanguage=0` warnings for unversioned files. Removing that fallback preserves WiX's actual file metadata and makes the real corrected build quiet. Its separately recorded corrected helper used the historical application/source payload; it is not a new current-source application build.

The historical Debian desktop entry referenced `Icon=tallyline` although no such icon was packaged. The corrected adapter writes a fixed desktop data file without that optional reference and disables the packager's default template. A real protected Debian/tar rebuild and independent inspection passed: launcher target, existing executable/resources, modes, checksums, source bindings and full legal notices remained intact. Both corrections have authentic artefact-level red/green evidence rather than comparisons against their own generators. Initial independent-auditor COM null-row and empty-log assumptions were rejected and corrected; failed controllers/receipts remain retained.

## Signed-source Development rebuild — 2026-11-02

A separate secret-free build froze signed source `92829f93081f4544721522a5e97a5eb20ea849c9`, tree `34dff281dc6fe6cb8a7a8005f1e3f38ce5e364b4`. Genuine own-source Cargo builds used Rust 1.99.0 for both applications and helpers. Both actual headless reports returned Development `0.0.0`, null release tag/commit, exit zero and no stderr, before desktop bootstrap. Windows returned 156 bytes; Linux returned 158 bytes in the pinned runtime with only the app mounted and networking denied. No release metadata was injected and no Cargo package-version equality was imposed.

| Prebuilt input | SHA-256 |
| --- | --- |
| Windows application | `8eb5abfa553cc55c97b227faa07400a538f02b85ff1ccff311d9dd77e592e15b` |
| Windows helper | `660d1b047bee606c8b0bfbb9ea96ebf6952c7ce6a66402d4eb7f020bd903080f` |
| Linux application | `f0c450513b88cdae2550df9b7084de1726522c6b503c81613529dc40a6273b28` |
| Linux helper | `1fcfc77faa1d6d7ea6c52fe8d741a99fd5a6f71ff13ac5301d3b4f660db2f1a9` |

The new preferred-source capsule is 690965453 bytes, SHA-256 `b18ac7781ca2d1ff4f7bb3a55f7863a2768d0f73699252873d0a694ca65ff9ef`. Independent readers checked all 28286 regular files and 5346 directories: every one of the 825 project entries matches the actual signed Git tree or its exact LFS pointer identity, including 178 materialised LFS objects; all 501 vendor trees match the protected vendor hash list; the five approved native/toolchain source artefacts match their explicit retained hashes. Modes are 0644/0755, ownership/timestamps are normalised, and links, special files and PAX extensions are absent. No archived code was executed. An initial archive-derived Git-blob record was rejected because `git archive` materialised LFS data; raw `cat-file` identities and a separate `ls-tree` comparison established the corrected record. Rejected records remain outside the capsule.

Actual notice generation succeeded for both target graphs. All 411 Windows and 516 Linux physical legal files independently matched the delivered material, including catalogue hashes `62d886ee7350a0d5abcf0c749901512b2d78a3c470b0699a4c1114f58e071fb5` and `1254db6de5be4d0291aaef437e87d0018af9fc2613d5c752aa8807894cff538b`, respectively. Canonical app receipts, current source-map/archive verification and strict target legal admission passed before fresh harmless fixture creation. The Windows stage seal is `59c63eaddbc4cad774006b20404f39582651e92c5a8a1b6ecf7ba6d3691be502`; the Linux seal is `a43a79653bedf3657a885d90dbe957a425c3ab9f6d21fd74d8f5cd8f3655f955`.

| Artefact | Bytes | SHA-256 |
| --- | ---: | --- |
| Windows MSI | 40565364 | `557d453048704b36fda96bfa8e4ded5cf24253964f00539c3224c41695af3d72` |
| Windows ZIP | 83457703 | `f6cc361b3ab810704e8adc310ead3250cd25f5403b121080c1c9509e1ce5b131` |
| Linux deb | 52302550 | `d5e5d7a908a1e9575201668bd02b2715e00d1f1cb17a391afa8649898297b95c` |
| Linux tar.gz | 51998094 | `19f0be5b8934c9f84469776bc56acef5e00cdf0d388441c51a70f75c83343e5e` |

Both protected assemblies succeeded. Linux used the same pinned runtime and isolation profile above, with exactly stage read-only, fixture read-only and output writable; no source/archive/Cargo/compiler mount or download path was available. Windows used the sealed prebuilt helper outside the checkout with a cleared environment and empty outer PATH; host operating-system denial remains unproved. The actual UEX lexical format remains unverified; these are harmless graphic-ASCII fixtures, never real credentials.

Independent readers verified all 428 Windows payload files, exact MSI components/streams and cabinet members, and sole-data-file fixture placement. The actual MSI passed the same 467-case, 169054-evaluation read-only condition campaign, with zero custom actions and no file-language fallback warning. Linux inspection checked all 533 portable and 535 Debian files, 271 complete DEP-5 notice sets, safe paths/modes/timestamps, the fixed symlink, control checksums and 114294 KiB installed-size accounting. The standard Debian reader accepted the unchanged package after receiving an explicit owned scratch directory; its initial read-only-root temporary-directory refusal remains retained. Both actual prebuilt scanners accepted independent extractions and refused all three controlled UTF-8/UTF-16LE/UTF-16BE leak files using count-only diagnostics.

A premature Linux reader observed an incomplete gzip while assembly was still active; the completed independent audit ran only after the single package process exited zero with PID zero. Initial host executable-lookup and JSON-date controller refusals occurred before that container's first start. A separate read-only MSI inspection request timed out in automatic approval review before execution and succeeded on the one permitted retry. None caused a package-process rerun or a substituted artefact. All failed controllers and receipts remain retained.

## Portable archive permission correction — 2026-11-02

An additional same-payload repeat found that the Windows fallback inferred execute permission for `libonnxruntime.so.1.31.0`, while the Linux portable stage correctly sealed it as data mode 0644. The native Debian adapter independently uses its documented 0755 runtime profile. The correction removes only the non-Unix shared-library filename inference; Unix input permissions and Debian modes are unchanged. An independent public ZIP/tar reader contract failed before the fix and passed afterwards on Windows.

A newly Cargo-built Windows helper, SHA-256 `396f0de9bf311b7c4531c455e9fb5ca0b954be0511cdbe99e8b7e8b2ead4ed2b`, compiled from public `2b150faab7e7fe5ce6aeda77e9c6bd0b1b0fa1d1` plus the exact permission correction, recreated the preserved Linux tar.gz and Windows ZIP byte for byte with their original payloads and fixtures. This diagnostic helper experiment does not relabel the `92829f9` app/source/protected-job tuples or establish compiler/MSI reproducibility. The optional focused Linux SDK unit test compiled but could not launch its absent Python reader; that environment refusal is retained, with no skipped test, tool installation or image change. The actual Linux-produced archives separately passed independent inspection.

## Caller-owned Debian permissions — 2026-11-03

Corrected 2026-11-03: the full Windows check exposed a regression in the portable correction: the shared tar writer also applied its non-Unix fallback to Debian, making the runtime 0644 despite the Debian adapter's required 0755 profile. The unchanged independent Debian reader test failed. Both Debian tar calls now receive the adapter's explicit mode policy, shared with its filesystem normalisation; the portable writer retains its own input/fallback policy. All seven focused packaging contracts pass, including the original Debian and portable permission expectations.

A fresh genuine Windows helper, SHA-256 `8be4d12080f65c3672f2d7907d872d44ad7e91b6520f37aa17fbe6bb03cbe952`, compiled from public `2b150faab7e7fe5ce6aeda77e9c6bd0b1b0fa1d1` plus the portable correction and caller-owned policy, recreated the same historical Linux tar.gz and Windows ZIP with their original hashes. Its actual prebuilt Debian repack command also passed independent ar/tar inspection: application/runtime 0755, control files 0644, normalised ownership/timestamps and the fixed launcher link. These outside-checkout commands used an empty outer PATH. The Debian input was a harmless synthetic tree; the archive repeats reused the recorded `92829f9` payloads and fixtures. This is new helper evidence only, not a new application build, Linux helper test, protected installer run or release certificate.
## Remaining gates

These Development experiments cover only their exact recorded source/input tuples. A final qualified release must freeze and revalidate its own application/helper/source/legal inputs; historical or mixed-helper fixture receipts must not be relabelled. Authenticated signed-tag/CI provenance and publication belong to issue #98. Real UEX format confirmation, Windows OS isolation, clean-OS install/upgrade/uninstall, native Windows/X11/Wayland display and applicable permissions/prerequisite acceptance remain open. Windows OCR still requires the separately installed Visual C++ Redistributable; forbidden Microsoft runtime DLLs are not bundled. The unresolved no-extra-prerequisite and legal System Library review gates remain unchanged. No release, tag, upload or workflow dispatch follows from this record.
