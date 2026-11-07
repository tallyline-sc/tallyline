# Public UEX contract metadata

`public-v1.json` is a metadata-only observation of five credential-free primary UEX routes, captured on 2026-11-02 through the explicit `contract-snapshot` spike command. It contains field paths, primitive kinds, HTTP codes and the buy/sell status-code sets. It contains no response messages, vocabulary labels, prices, quantities, account information, screenshots or raw response bodies.

The five HTTP-200 samples contain 7, 25, 34, 45 and 62 field paths respectively for `game_versions`, `commodities_status`, `data_parameters`, `commodities` and `terminals?type=commodity`. The complete 11,788-byte metadata file has SHA-256 `4fa17a62405c63c97548b2a3963d2ffdba1e9adbc1c5c9f8ad03bbb4bd455eb1`.

Comparison reports observed field additions/removals, new non-null kinds and changed status-code sets. Null samples and empty nested arrays cannot prove that a type or child field was removed. These observations do not define every possible nullable shape or test authenticated endpoints. Every complete baseline requires all five distinct public routes and nonempty reference samples.

The `contract-check` command compares actual public samples with this baseline and fails on observed drift or an unusable sample. Technical UEX rejections expose only their HTTP code and a bounded identifier, never message or data values. [The drift guide](../../../docs/public-api-contract-drift.md) describes the opt-in main-only workflow and remaining hosted acceptance.

Regenerate a proposal only through the explicit read-only `contract-snapshot` command, inspect its metadata and the adapter's documented wire contracts, then review an intentional baseline change. The scheduled checker never rewrites this file.
