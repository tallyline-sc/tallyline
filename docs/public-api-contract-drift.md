# Public UEX contract drift check

> **Doc type:** Living reference — implementation checkpoint; hosted activation pending. Last reviewed: 2026-11-02.

The M1 API-spike CLI observes five fixed public routes on `https://api.uexcorp.uk/2.0/`: `game_versions/`, `commodities_status/`, `data_parameters/`, `commodities/` and `terminals/?type=commodity`. Every request is GET-only. Neither command reads account credentials, an app-token file or screenshot input from the environment; alternative hosts, mixed account commands and extra arguments are refused before a request. Existing HTTPS, no-redirect, timeout and capped-body transport rules apply. The five requests run sequentially without retries.

```sh
cargo run --locked -p tallyline-adapter-uex --bin tallyline-uex-spike -- contract-check
cargo run --locked -p tallyline-adapter-uex --bin tallyline-uex-spike -- contract-snapshot
```

`contract-check` compares complete metadata against the reviewed `crates/adapter-uex/contracts/public-v1.json` baseline. It returns a failing exit code for observed field additions/removals, new non-null primitive kinds, changed buy/sell status-code sets, failed or unobservable responses, malformed metadata or transport failures. Diagnostics contain endpoint names, field paths, primitive kinds, HTTP codes and bounded technical UEX error identifiers. They exclude response messages, vocabulary labels, prices, quantities, versions, account information and raw bodies. A technical rejection identifier must be at most 64 ASCII bytes, start with a letter or underscore, and contain only letters, digits or underscores; other strings produce a closed refusal instead.

`contract-snapshot` emits only metadata after all five samples succeed. The payload's debug representation is redacted, and parsed response values are discarded before the next request. JSON projection runs synchronously outside the asynchronous I/O runtime. Bounds are 16 levels, 4,096 distinct field paths, 256 bytes per path, 64 bytes per field name and 1,000,000 visited JSON nodes, in addition to the existing response-byte cap. Baseline input is limited to 8 MiB and requires all five distinct supported routes and nonempty reference objects/rows.

## Observed baseline

On 2026-11-02 an actual local run returned HTTP 200 for all five credential-free requests. The retained metadata contains respectively 7, 25, 34, 45 and 62 field paths: 173 in total. Both status vocabularies contain codes 1 through 7. The exact 11,788-byte baseline has SHA-256 `4fa17a62405c63c97548b2a3963d2ffdba1e9adbc1c5c9f8ad03bbb4bd455eb1`. Raw response bodies were not saved. The capture used the in-progress sampler based on integration commit `d78f5b7`; it does not claim that the sampler already existed in that parent commit. Source and executable identities are retained with the local experiment evidence.

The baseline is an observation, rather than a complete server schema. Null values cannot establish a non-null kind, and null/empty nested arrays cannot prove that child fields were removed. Ordinary scalar changes, list ordering and vocabulary label changes are intentionally ignored. Authenticated endpoints, submit behaviour and every possible optional field remain outside this credential-free observation. No synthetic error code in a local contract test is described as a live observation.

A subsequent actual local `contract-check` completed with exit code 0 and reported no observed field/type/code changes across the same five public samples. This is a local read-only result, separate from hosted workflow execution.

## Opt-in workflow

`.github/workflows/api-contract-drift.yml` schedules a weekly observation on Monday at 06:31 UTC. Its job requires the `tallyline-sc/tallyline` repository, `refs/heads/main` and repository variable `TALLYLINE_UEX_DRIFT_ENABLED` equal to `true`. An explicitly approved `workflow_dispatch` on `main` can run without that variable. There is no pull-request or push trigger, and a dispatch selecting another ref skips the job. GitHub schedules may run later than their nominal time.

The workflow has only `contents: read`, does not request secrets, disables persisted checkout credentials, uses the existing full-SHA checkout pin and installs/verifies the exact Rust toolchain in `rust-toolchain.toml`. It builds the existing spike and publishes its value-free diagnostic output in the job log and step summary; it does not upload response dumps, rewrite the baseline, create an issue or make a UEX write. Its whole-job timeout is 15 minutes. Enabling the repository variable or dispatching it remains an owner-controlled external action under `CLAUDE.md`; this implementation does neither. Hosted execution is pending until the workflow reaches `main` and is intentionally activated.

## Review an observed change

Keep a failure visible and first distinguish transport failure or insufficient samples from an actual contract change. For a structural change, run `contract-snapshot` locally, inspect its metadata against the documented API and adapter contract, and update [the UEX notes](plan/06-uex-api.md) and affected parser contracts before reviewing an intentional baseline change. Never accept a new baseline merely to silence CI. No credential, raw response, screenshot or private export belongs in that review.

Local public-API contracts verify every route's GET method and credential-free headers, value redaction, refused envelopes, complete baseline admission, technical-code handling, structural comparison and failing CLI output for drift. Their network boundary is a local HTTP mock server. The affected adapter package passed 66 library tests, nine binary tests and its existing subprocess contract; its private subprocess entrypoint remains visibly ignored. Strict adapter Clippy and the unchanged pinned actionlint 1.7.12 passed. The actual five-route observation remains separate from those deterministic tests and from hosted-workflow acceptance (R-API-5, R-DOC-2; O-69; S-14).
