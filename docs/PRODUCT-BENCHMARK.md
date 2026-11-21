# Third-party interference product benchmark

Research 2026-11-20 gaps, backlog B1005. Module `src/product_bench.rs`, command
`kshana-pro product-bench` (`src/cli/bench_cmd.rs`), tests `tests/product_bench.rs`.

Uses the open engine's referee unchanged: `kshana jammertest locate-score`
(`kshana::jammertest::location::score`). It is called on truth events **with no position** and on
product reports **with no point and no region**, so it only attributes reports to events by time
(with the protocol's guard) and counts unattributed reports per hour of clean time. It is never
asked to score a location.

1. **Register** the protocol before the data: `product-bench register <protocol.toml>` (free)
   prints the SHA-256 of its canonical JSON. The protocol fixes the attribution guard, the
   affected-area cell size and optional pass bars (`min_detection_rate`, `max_median_latency_s`,
   `max_false_per_hour`, `min_area_iou`).
2. **Score**: `product-bench score <request.toml> --out <dir> --signer <name> --key-file <file>`
   (`pro-interference-product-benchmark`, Pro). The request carries the protocol and its
   registered hash (a changed protocol is refused), the truth file and one reports file per
   product.

Truth: `observation_start_utc`, `observation_end_utc` and events with `start_utc`, `end_utc` and
`victims_affected` (positions of victim receivers affected). Product reports: JSON records of
`utc` and `affected_cells` only; any other field, such as an emitter estimate, is refused.

Per product: detection rate (events with an attributed report), latency from onset to the first
attributed report (median, worst), false reports and false reports per clean hour, and
affected-area precision, recall and IoU of the product's cells against the truth cells (cells of
victim receivers), per event and pooled; each pass bar met or not. Products are listed in request
order and not ranked.

**Never scored**: a product's emitter-position estimate, bearing or source region. Fixtures name
no vendor (`PRODUCT-A`, `PRODUCT-B`); results for named vendors exist only in a Pro scoring run on
the owner's data, and publishing them is the owner's decision. Metric arithmetic checked against
hand-computed values on the SYNTHETIC fixture (`product-bench synth`). No JammerTest or TEXBAT data
are bundled.
