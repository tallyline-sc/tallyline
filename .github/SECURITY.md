# Security policy

Thank you for taking the time to look at the security of Tallyline. This
document explains how to report a vulnerability, which versions
receive fixes, and what you can expect from us in return.

> **Project status: planning.** There is no release and no runnable code yet.
> The repository holds the plan ([`docs/plan/`](../docs/plan/)), the
> [release process](../docs/release-process.md) and a small public OCR test
> corpus ([`corpus/`](../corpus/README.md)). This policy already covers them,
> and it will cover every release.

## Reporting a vulnerability

**Please do NOT open a public GitHub issue or pull request for anything you
believe is security-sensitive.** Public disclosure before a fix is available
puts every user of the client at risk.

The preferred channel is GitHub's **private vulnerability reporting**:

> **[Report a vulnerability](https://github.com/tallyline-sc/tallyline/security/advisories/new)**

That form gives us a private place to triage the finding, work on a fix with
you, request a CVE and coordinate disclosure.

A good report includes:

- A clear description of the issue and its impact.
- The affected version or commit SHA, the package (Windows MSI or ZIP, Linux
  `.deb` or `tar.gz`, portable mode or not) and the operating system.
- The affected area, for example the storage of the UEX secret key, the
  upload screenshot and its redaction, the optional AI re-check via Ollama,
  the update notice, or the build and release workflows.
- Reproduction steps, a proof of concept, or a minimal test case.
- Your assessment of severity (a CVSS vector is welcome but not required).
- Whether you intend to publish your own write-up, and on what timeline.

**Never put secrets or private screenshots into a report**, not even into the
private form:

- No UEX secret key and no UEX app token, neither yours nor anybody else's.
  Say where the value appeared and replace it with `<redacted>`.
- No unredacted screenshot or panel crop. Black out the balance
  ("CURRENT BALANCE"), player names and chat windows first, as
  [`corpus/README.md`](../corpus/README.md) describes for the test corpus.
- No diagnostics export unless we ask for one. The export is designed to
  remove secrets, your UEX username and your OS user name (R-NF-6), but check
  it before you send it.

If you cannot use GitHub's form, e-mail the maintainer at
[lucas.greuloch@gmail.com](mailto:lucas.greuloch@gmail.com) with the subject
*"Security report: Tallyline"*. As a last resort, open a minimal
public issue that says only *"I would like to report a security issue, please
contact me"*, without any technical detail, and we will contact you privately
to arrange a channel.

Problems in the services the client talks to belong to their operators, not
here; see [Out of scope](#out-of-scope).

## What to expect

This is an unofficial community project without a commercial SLA, but we aim
for the following turnaround on every report:

| Step                                                   | Target                    |
|--------------------------------------------------------|---------------------------|
| Acknowledge receipt of the report                      | within 7 days             |
| Initial triage and severity assessment shared with you | within 14 days            |
| Fix in `main` for High / Critical issues               | within 90 days            |
| Coordinated public disclosure after a fix is available | within 14 days of release |

The 90 days are an upper limit. Once a report is confirmed, the
[release process](../docs/release-process.md#security-release) aims to
publish a security release within 7 days.

If we cannot meet one of these targets, we will tell you why and propose a
new date in the advisory thread. We will credit you in the published advisory
and in the changelog unless you ask us not to.

## Supported versions

No version has been released yet, so no version receives security fixes yet.

From the first release on, only the latest release and `main` receive
security fixes; this also applies to `0.x` releases before `1.0`. Older
releases get no backports: the fix ships in the next release. The client does
not update itself; it only shows a notice with a link to the release page
(R-NF-7, S-25), so please install the fixed release yourself.

| Version                                   | Supported            |
|-------------------------------------------|----------------------|
| No release yet                            | –                    |
| Latest release and `main` (once released) | :white_check_mark:   |
| Every older release                       | :x: (please upgrade) |

## Scope

The requirement (`R-…`) and measure (`S-…`) IDs below refer to
[`docs/plan/01-requirements.md`](../docs/plan/01-requirements.md) and
[`docs/plan/10-supply-chain-security.md`](../docs/plan/10-supply-chain-security.md).
They describe how the client is meant to behave; a way to break that
behaviour is a finding.

### In scope

- **UEX secret key handling.** The secret key (and a UEX app token the user
  entered instead of the built-in one) is stored only in the OS keystore: Windows Credential Manager
  or the Linux Secret Service. Only on Linux, and only if the Secret Service
  is unavailable, a fallback file with permissions 0600 is used after an
  explicit warning (R-NF-4). The key must never appear in a log line, in the
  diagnostics export (R-NF-6), in a screenshot or in this repository, and it
  must never go to a host other than the UEX hosts that R-API-3 allows (the
  built-in base host, or a custom host the user confirmed as an
  expert setting). Any path that leaks the key, stores it elsewhere or sends
  it elsewhere is in scope.
- **Screenshot and balance leakage.** The upload screenshot holds only the
  perspective-corrected shop panel crop and the location field, and the
  balance is always redacted. If the crop geometry is unconfirmed, and for
  every manual attachment, a report is released with the screenshot only
  after the user confirms a full-size preview (R-SUB-7). The AI re-check
  receives panel crops only, never the whole screenshot (R-VLM-9). Working copies are redacted panel crops with a
  limited retention (R-CAP-7), and the "Report a misread" export contains
  only redacted crops (R-QA-5). In scope: the balance or other personal data
  reaching UEX, an Ollama host, an export, a log or the public corpus in this
  repository (`corpus/public/`).
- **Network and local exposure.** The client sends no telemetry. It
  connects only to the destinations in the data-flow inventory
  ([`docs/plan/02-architecture.md`](../docs/plan/02-architecture.md) §9):
  the UEX hosts allowed by R-API-3, GitHub Releases for the update notice,
  the configured Ollama host allowed by R-VLM-6 and, later, the fixed URLs
  of optional OCR model downloads (R-L10N-3).
  It opens no listening TCP or UDP socket; its only local channel is the
  single-instance socket, which accepts nothing but a fixed activation token
  (R-NF-10). It does not access the game process beyond reading the process
  list ([`docs/plan/02-architecture.md`](../docs/plan/02-architecture.md)
  §9). In scope: any other outbound connection, including telemetry from a
  bundled library or native component (ONNX Runtime ships telemetry that the
  client switches off; on Windows its remaining ETW events, which Windows' own
  diagnostic-data service may collect, are an accepted exception,
  ADR-0008),
  any listening socket, and any other access to the game.
- **Supply chain.** Anything that would let an attacker bring code onto
  users' machines unnoticed through this project's build and release
  pipeline: dependency resolution, the lockfile, the vendored crate sources and
  their verification (S-1 to S-6), the pinned Rust toolchain and build tooling
  (S-10), the installer tools (S-11), the GitHub Actions workflows, their pins
  and permissions (S-13 to S-15), the release artefacts with their checksums,
  SBOM and build attestations (S-16 to S-18), the integrity check of the
  bundled OCR models at build time and at load time (S-22, R-SEC-7), and the
  loading of native libraries (ONNX Runtime) from an app-specific instead of a
  shared directory (S-24, R-SEC-8). Native code inside a dependency is allowed
  only as a registered exception; a native part that is not registered is a
  finding.
- **Update notice.** The client checks GitHub Releases at most once a day,
  the check can be disabled, and it only shows a "new version available"
  notice with a link to the GitHub release page; it downloads and starts
  nothing by itself (R-NF-7, S-25). In scope: any way to make the client
  download, install or run something, to make the notice point somewhere
  other than this repository's release page, or to switch off TLS
  certificate or hostname verification for this or any other connection
  (S-26).
- **Ollama host and models.** The optional AI re-check talks to
  `http://localhost:11434` by default. Only a loopback host is used without
  asking; any other host, and any remote (cloud) model, is used only after an
  explicit confirmation with a privacy notice (R-VLM-6). In scope: any way to
  make the client send crops to another host or to a remote model without
  that confirmation; AI results that skip the validation every OCR result
  goes through or that overwrite fields the user has confirmed; and a model
  that matches no entry of the evaluated-model list shipped with the release
  (for example a re-pushed tag with a new digest) but is treated as evaluated
  (S-27).
- **Untrusted input.** Image files and folders from the import paths, and
  responses from the UEX API or from Ollama: a crafted input that leads to
  code execution, to reads through a link outside the configured folder, to
  writes outside the client's data directories, or to a submission that
  bypasses the review and the submission gate.

### Out of scope

- **UEX's services:** the UEX API and website, UEX accounts, DataRunner
  status, and the data held by UEX (including wrong prices or terminals).
  Please report these to UEX Corp ([uexcorp.space](https://uexcorp.space)).
  If the client is involved as well, you are welcome to tell us too.
- **Cloud Imperium Games' services:** Star Citizen, the RSI Launcher, the RSI
  website and RSI accounts. Please report these to Cloud Imperium Games
  through their official channels.
- **Ollama and its models:** vulnerabilities in Ollama itself, or in a model
  downloaded through it. Please report these to the
  [Ollama project](https://github.com/ollama/ollama). The integrity of a model
  download is Ollama's responsibility (S-27).
- Vulnerabilities in third-party dependencies that have no viable fix path in
  this project. Please report those upstream first; you are still welcome to
  let us know so we can track an upgrade.
- Findings that require an already compromised machine, an attacker who
  already runs code as your OS user or with administrator rights, or physical
  access to an unlocked machine.
- Reports generated solely by automated scanners without a demonstrated,
  reproducible impact on this project.
- Social engineering of contributors or users.
- Best-practice suggestions without a concrete vulnerability, and weaknesses
  in the published plan while no code implements it yet. These are very
  welcome as regular issues or pull requests instead.

## Coordinated disclosure

We follow coordinated disclosure. Our default disclosure window is **90 days
from the date we acknowledge the report**, or sooner if a fix and a release
are already public. If a fix is not yet available after 90 days, we will agree
on an extension with you in the advisory thread before any public disclosure.

When the advisory is published, we will:

1. Publish a fixed release.
2. Publish the GitHub Security Advisory with details, affected versions,
   workarounds and credit. Because the client does not update itself, the
   advisory and the release notes say clearly whether users must update.
3. Add a `### Security` entry to the changelog (`CHANGELOG.md`, which follows
   Keep a Changelog, see [`docs/release-process.md`](../docs/release-process.md))
   that refers to the advisory.

## Safe harbour

We will not pursue or support legal action against researchers who:

- Make a good-faith effort to comply with this policy.
- Avoid privacy violations, data destruction and service degradation against
  users of this software.
- Use only their own test data, their own UEX account and key and their own
  screenshots, or data they have explicit permission to use, and stop as soon
  as they have demonstrated the issue.
- Give us reasonable time to remediate before any public disclosure.

If a third party initiates legal action against you for activity that
complied with this policy, we will make our position on safe harbour known.

This policy does not authorise you to test the security of any third-party
service: UEX, Cloud Imperium Games / RSI, GitHub, or Ollama and its model
registry. If reproducing a finding needs requests to the UEX API, follow
UEX's terms, use only your own account and key, and send test submissions
only with `is_production=0`. Never send fabricated or test data with
`is_production=1`: it would end up in the community's shared price data.

## Verifying releases

There is no release yet. Every release will be built only in CI from a
protected tag on protected `main` (S-15) and will publish:

- `SHA256SUMS` for all artefacts (S-17);
- a CycloneDX SBOM (S-18);
- signed build provenance as a GitHub artifact attestation (S-16), which you
  can check before you report a finding against a downloaded file:

  ```bash
  gh attestation verify <downloaded-file> --repo tallyline-sc/tallyline
  ```

Whether the Windows installers will also be Authenticode-signed is still open
(S-21); until then, the attestation and the checksums are how to check an
installer. A finding that requires bypassing these verification steps is
itself in scope.

## Thank you

Security research is real work and we appreciate it. If you report something
that turns out to be valid, we will credit you in the published advisory and
the changelog by name, handle, or anonymously, as you choose.
