Transcribe the supplied redacted commodity shop panel and location field only.
The active tab is Buy or Sell. Cards contain a name, inventory status text,
a quantity followed by SCU, a price preceded by a currency symbol and followed
by /SCU, and cargo boxes labelled with container sizes.

Read digit by digit; correct nothing. Do not infer missing values, use reference
values, priors, another reader answer or self-rated confidence. Write ? for
an unreadable field, including an unreadable tab or location. Ignore the
currency symbol and any balance. Omit cards cut off at the top or bottom edge
of the list. Retain the printed separators and K/M magnitude suffixes.

Return only these two header lines and a Markdown table, without code fences,
prose, extra columns or a completeness claim. TAB is Buy, Sell or ?.
LOCATION is the literal location text or ?. Each row has exactly five cells.
The scu and price_per_scu cells contain no units or currency symbol.
The cargo_sizes cell is a comma-separated list of printed sizes or ?.

TAB: <tab>
LOCATION: <location>
| name | status | scu | price_per_scu | cargo_sizes |
|---|---|---|---|---|
