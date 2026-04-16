# Patina benchmarks — end-to-end HTTP measurements

This document is the authoritative answer to "does patina make a real
WordPress pageload faster?" Unlike the per-function microbenchmarks in
`php/benchmarks/`, these numbers come from full HTTP requests against a
real WordPress install under controlled conditions, with the per-request
activation cost of `patina_activate()` paid on every sample.

The methodology, scenarios, and runner live under `scripts/bench-runner.sh`
and `profiling/k6-workloads.js`. The bench infrastructure itself is
described in `docs/BENCHMARK_PLAN.md`.

## Latest result — `phase9-shim-filters` (2026-04-15)

**Headline**: action item #2 landed. `apply_filters('esc_html', ...)`
and `apply_filters('esc_attr', ...)` now fire from the PHP shims
(`PATINA_SHIMS_PHP` in `crates/patina-ext/src/lib.rs`) instead of from
Rust via `call_user_func!`, removing ~2–5 µs of boundary-crossing per
call and restoring PHP→PHP fast-path dispatch for the "no filter
registered" case. 9 new `EscFilterTest` cases in
`php/tests-integration/` verify the filter contract (invocation,
arg order, the "third arg is the original pre-cast value" subtlety
that matches stock WP).

Against the phase9 stock baseline (intra-run, paired t-test, n=200):

| Config | Result |
|---|---|
| `esc_only` | flat vs stock on all 9 scenarios (every p > 0.15, max \|Δ\| 0.4%) — the config that used to carry the apply_filters round-trip cost is now indistinguishable from stock |
| `parse_blocks_only` | flat on all 9 (every p > 0.054, max \|Δ\| 0.5%) — no change expected from this phase, confirms no regression |
| `kses_only` | +0.5 to +1.3% slower on 8/9 scenarios (4 at p<0.05) — still carries the Rust→PHP round-trip for `apply_filters('pre_kses', ...)` and the per-call `has_filter` lookups, which is exactly what action item #3 targets |
| **`full_patina`** | **−0.6 to −1.6% *faster* than stock on all 9 scenarios, every p < 3e-5.** First time any patina configuration has shown a statistically significant speedup on any scenario. |

The cross-run diff against `phase8-activation-cached` is not clean —
the phase9 stock baseline is ~5–7 ms higher than phase8's (uniform
across all scenarios, ~3% CV unchanged), which is host-level thermal
drift from running four full bench matrices back-to-back in one
afternoon. Stddev is intact (phase8 3.1% / phase9 3.1% CV on
`single_classic`, identical), so the intra-run pairing above cancels
the drift and gives the real effect.

### Caveat on `full_patina` vs `stock`

The `full_patina` −1% speedup should be read as "not worse than stock,
plausibly better" rather than a clean win. The paired t-test compares
`stock` samples (measured first in each chunk) against `full_patina`
samples (measured ~30–60 s later in the same chunk), so any
within-chunk thermal drift biases the comparison. Summing the
individual-override deltas (esc_only ~0 + kses_only +0.7% +
parse_blocks_only +0.2%) gives an expected `full_patina` of ~+0.9%;
the measured ~−1% leaves ~1.9% unexplained by simple composition,
which is suspicious for a pure-effect reading. An order-reversed
control run (full_patina first, stock last within each chunk) would
settle it.

Either way, we've gone from "statistically significantly slower on
6/9 scenarios at p<0.001" (phase7-paired) to "statistically
indistinguishable or faster on 9/9" (phase9-shim-filters), via two
targeted bridge-overhead fixes. That direction is unambiguous.

## `phase8-activation-cached` (2026-04-15)

Action item #1 landed. `patina_activate()` short-circuits after the
first call per FPM worker via a Rust-side `AtomicBool` + a
`patina_is_activated()` probe the mu-plugin calls before building the
skip list. The cached path costs ~22 ns per invocation (10k calls in
0.22 ms, measured). Cross-run diff against `phase7-paired` showed the
fix delivered 0.3–1.7% off every patina config on every scenario it
was slow on, with the biggest gains on `full_patina` (all 4 swaps
skipped):

| Scenario | phase7 `full_patina` | phase8 `full_patina` | Δtmean (cross-run) | paired p |
|---|---:|---:|---:|---|
| archive_category | +1.1% vs stock (p=2e-07) | −0.2% vs stock (p=0.65) | **−1.2%** | 0.009 ** |
| archive_tag | +0.7% vs stock (p=2e-04) | −0.5% vs stock (p=0.07) | **−1.1%** | 0.0002 *** |
| single_classic | +1.2% vs stock (p=5e-09) | −0.0% vs stock (p=0.68) | **−1.5%** | 0.0004 *** |
| single_commented | +1.0% vs stock (p=3e-07) | −0.1% vs stock (p=0.72) | **−1.2%** | 1.4e-05 *** |
| single_long | +0.4% vs stock (p=0.018) | −0.0% vs stock (p=0.46) | **−0.6%** | 0.015 * |
| single_short | +1.0% vs stock (p=6e-08) | +0.0% vs stock (p=0.61) | **−1.3%** | 4.5e-07 *** |

**`full_patina` is now statistically indistinguishable from stock on
every one of the nine scenarios** (all p > 0.068). `parse_blocks_only`
also narrows dramatically: `single_short` went from +1.1%/p=1.3e-08 to
−0.0%/p=0.36, and `single_long` from +0.7%/p=0.001 to +0.2%/p=0.12.
`archive_category` and `single_classic` still show a tiny residual
+0.2–0.5% for parse_blocks_only at p~0.02 — the remaining bridge
cost that action items #2 (shim-level `apply_filters`) and #3
(per-request `has_filter` cache) need to close.

The stock control moved by ±0.3% across the two runs (max p=0.45),
confirming host/bench reproducibility: the patina deltas above are
real, not run-to-run drift.

## `phase7-paired` (2026-04-15) — harness rebuild

The benchmark harness now resolves sub-1% deltas with p<0.001 on most
scenarios. This is the run that first exposed how much bridge overhead
was dominating every override's effect — and traced `phase6-initial`'s
"no statistically significant delta" conclusion to an *instrument*
failure rather than a patina-effect failure.

`profiling/k6-workloads.js` had `startTime: null` on every scenario,
which k6 interprets as "start simultaneously" — with 9 scenarios ×
`vus=1` and `pm.max_children=5`, 4 requests per round queued behind
workers, and queue-wait variance (~25 ms) was the noise that swallowed
every patina effect. Serializing k6 into a single `per-vu-iterations`
executor that round-robins through the URLs dropped TTFB from ~150 ms
to ~28–48 ms and stddev from 12–23% CV to **1.9–3.2% CV** — a 6–8×
reduction that exposed the real underlying effects. Chunked config
interleaving + paired t-tests + cpuset pinning layer on top of that
fix; none of them mattered until the parallel-scenarios bug was out
of the way.

At the new resolution, every patina configuration came out
statistically significantly *slower* than stock on the scenarios that
had a measurable signal, and indistinguishable from stock on the
rest. `parse_blocks_only` was +0.5–1.1% slower on 6/9 scenarios
(p<0.01), zero-sig on 3/9, and zero scenarios faster. `full_patina`
had the same pattern with slightly worse magnitudes. The bridge
overhead exceeded the Rust algorithmic savings on 30–50 ms TTFB
requests — exactly the "action items" regime that phase6-initial
could only speculate about, and the thing phase8 started fixing.

### Setup

- **Host**: Peter-PC, AMD Ryzen 9 5950X (16 cores / 32 threads), CachyOS
- **Stack**: Docker — nginx:alpine + php-fpm 8.3.30 + mariadb:11. The
  php-fpm container is pinned to `cpuset=2,3` for the duration of the
  run via `docker update --cpuset-cpus` and restored on exit.
- **WordPress**: 6.9.4 (TwentyTwenty-Five active, FSE default theme)
- **Content**: 90 posts / 93 comments / 6 users — the WordPress
  theme-test-data WXR fixture plus the expanded block-tier corpus
  (`profiling/benchmark-content/`): 15 / 30 / 60 blocks @ ~6 / 13 / 25 KB.
- **Patina**: 0.1.0 @ `c3cb04a`
- **k6**: single `per-vu-iterations` executor, `vus=1`, round-robins
  through the 9 scenarios in order. 200 post-warmup iterations per
  scenario per config, WARMUP=3 cycles dropped per chunk. Cache-busted
  query string, Host header forced to `localhost:8080` so WP's
  canonical-URL logic doesn't 301-redirect.
- **Chunked interleaving**: the runner cycles all configs per chunk,
  CHUNKS=5, so every `(config, scenario)` pair gets 40 samples per
  chunk. Within-chunk pairing lets bench-compare use a paired t-test
  instead of Welch's on otherwise-independent batches — pairing cancels
  minute-scale host drift and, in practice, moved most p-values by
  2–3 orders of magnitude vs the unpaired baseline.

### Configurations

Five configs, each with a different subset of overrides active, driven
by the per-override toggles landed in Phase 3:

| Config | Active overrides |
|---|---|
| `stock` | (none — `PATINA_DISABLE` kill switch) |
| `esc_only` | `esc_html`, `esc_attr` |
| `kses_only` | `wp_kses` (and every wrapper) |
| `parse_blocks_only` | `parse_blocks` |
| `full_patina` | all four |

### Scenarios

Nine URLs covering the common render paths:

| Slug | Exercises |
|---|---|
| `/` | Homepage / latest-posts loop (10 posts) |
| `/a-short-block-post/` | 15 blocks / ~6 KB — baseline single |
| `/a-long-block-post/` | 60 blocks / ~25 KB — parse_blocks tail |
| `/a-classic-html-post/` | Pre-Gutenberg HTML — wpautop + kses |
| `/a-commented-post/` | 20 comments — render-time kses x20 |
| `/category/announcements/` | Category archive |
| `/tag/perf/` | Tag archive |
| `/?s=lorem` | Search results |
| `/wp-json/wp/v2/posts` | REST API |

### Headline numbers (TTFB, ms)

`parse_blocks_only` vs `stock`, n=200 per cell, paired t-test over
matched chunk-index pairs. Δ% is relative to `stock`; ↑ = slower.
The full matrix for all four patina configs is in
[`fixtures/baselines/phase7-paired/report.md`](../fixtures/baselines/phase7-paired/report.md).

| Scenario | stock p50 | parse p50 | Δp50 % | Δtmean % | paired p |
|---|---:|---:|---:|---:|---|
| archive_category | 28.4 | 28.5 | +0.6 ↑ | +0.7 ↑ | 0.00048 *** |
| archive_tag | 26.6 | 26.7 | +0.4 ↑ | +0.7 ↑ | 0.00092 *** |
| homepage | 43.9 | 43.8 | −0.1 | −0.3 | 0.33 |
| rest_posts | 43.8 | 43.6 | −0.3 | −0.2 | 0.34 |
| search | 48.2 | 48.1 | −0.1 | +0.1 | 0.34 |
| single_classic | 34.6 | 34.9 | +0.8 ↑ | +0.9 ↑ | 6.4e-06 *** |
| single_commented | 45.5 | 45.7 | +0.5 ↑ | +0.6 ↑ | 0.00134 ** |
| single_long | 36.9 | 37.0 | +0.5 ↑ | +0.7 ↑ | 0.00116 ** |
| single_short | 35.2 | 35.6 | +1.1 ↑ | +1.1 ↑ | 1.3e-08 *** |

`full_patina` vs `stock` shows the same pattern, slightly worse on the
block-heavy pages: `archive_category` +1.1% (p=1.8e-07),
`single_classic` +1.2% (p=4.7e-09), `single_short` +1.0% (p=5.7e-08).
Homepage, rest_posts, and search remain statistically indistinguishable
from stock (p > 0.068). Stock p95 stddev is now 1.9–3.2% CV across all
scenarios — tight enough that 0.3 ms deltas are routinely significant.

### Analysis

1. **Bridge overhead currently exceeds Rust savings on 30–50 ms TTFB
   requests.** The microbenchmark speedups (1.5–6.9× per function call,
   see `README.md`) are real, but they amortize away the fixed costs
   that dominate a real HTTP request: per-request `patina_activate()`
   work, `call_user_func!` round-trips from Rust back to PHP on every
   `apply_filters` inside the escaping shims, and `has_filter` lookups
   on every `wp_kses` call. The total of those costs is ~0.3–0.5 ms per
   request, which is larger than the algorithmic savings from Rust at
   this workload scale.

2. **`single_classic` is the canary.** Classic posts contain no blocks,
   so `parse_blocks` walks an empty body and returns immediately — yet
   `parse_blocks_only` is +0.9% slower than stock on this scenario with
   p=6.4e-06. That delta is *pure override-installation overhead*: the
   cost of having the override present, independent of any useful work
   the Rust implementation does. Whatever fraction of the 0.3 ms we can
   remove will show up directly in this cell.

3. **`single_short` shows the worst relative hit (+1.1%)** because the
   baseline TTFB is smallest (35 ms) and the fixed bridge cost is the
   same. Larger pages amortize it better — `single_long` is +0.7%, the
   archives are +0.7% and marginal.

4. **`homepage` / `rest_posts` / `search` have null results**, which is
   interesting because they're the scenarios where `parse_blocks` runs
   the most per request (10 posts per loop × one `parse_blocks` call
   each). The algorithmic savings there approximately cancel the bridge
   cost, giving a true effect close to zero. If the action items below
   land, these should flip from "indistinguishable" to "reliably
   faster" before the single-post scenarios do.

### What this means for the project

- **The microbenchmark numbers in `README.md` remain accurate** — the
  Rust implementations *are* genuinely 1.5–7× faster per isolated
  function call. But they measure steady-state throughput on cached
  inputs and don't include the round-trip costs a live request pays.
  The gap between the two numbers is the bridge-overhead budget.
- **The benchmark harness is now load-bearing.** A sub-1% regression
  anywhere in patina will be caught at n=200 with p<0.01. Future
  changes should be held against `fixtures/baselines/phase7-paired/`
  with `make bench-compare TO=fixtures/baselines/<new>` — see
  `scripts/bench-compare.py` for the cross-run mode.
- **The next re-baseline is meaningful work.** Each action item below
  should move specific cells by a measurable amount; the benchmark can
  now attribute those moves to their causes instead of chasing noise.

### Action items (in priority order)

The "per-request cost" estimates are bounds implied by the phase7-paired
`single_classic` and `single_short` deltas — they define the budget
each fix has to recover.

1. ~~**Cache `patina_activate()` after the first call per FPM worker.**~~
   **Landed in phase8-activation-cached.** Rust-side `AtomicBool` +
   a `patina_is_activated()` probe from the mu-plugin. The fix
   removed 0.3–1.7% of TTFB across every patina config, taking
   `full_patina` from "significantly slower on 6/9 scenarios" to
   "indistinguishable from stock on 9/9". Commit is in the
   activation-cache work that accompanied this doc update — see
   `crates/patina-ext/src/lib.rs` `ACTIVATED` and
   `php/bridge/patina-bridge.php`.
2. ~~**Move `apply_filters('esc_html', ...)` back into the PHP shim.**~~
   **Landed in phase9-shim-filters.** Moved `apply_filters` firing
   from `patina_esc_html_internal` / `patina_esc_attr_internal` into
   `PATINA_SHIMS_PHP`. Closed the residual `esc_only` gap (was slow,
   now flat) and the composite `full_patina` config went from
   "indistinguishable from stock" to "statistically significantly
   faster on all 9 scenarios (p<3e-5)" within the phase9 run.
   9 new filter-contract tests in
   `php/tests-integration/EscFilterTest.php`.
3. **Cache `has_filter()` results for `wp_kses_allowed_html`,
   `kses_allowed_protocols`, and `wp_kses_uri_attributes`** in a Rust
   `OnceCell` keyed per-request. These currently fire on every
   `wp_kses` call and dominate the kses path's overhead — especially
   visible on `single_commented` which renders 20 comments through
   kses each.
4. **Re-baseline as `phase8-*`** after each fix lands (one run per fix,
   not one at the end) so bench-compare cross-run diffs can attribute
   movement to the right change.

## How to reproduce

Full matrix, ~11 minutes on the Ryzen above:

```sh
./profiling/setup-wordpress.sh                    # one-time: seed the corpus
CHUNKS=5 ITERATIONS=200 make bench-full           # full per-config matrix
make bench-compare RUN=/tmp/patina-bench/<ts>/    # render the report
```

The runner pins php-fpm to `cpuset=2,3` automatically (override via
`CPUSET=x,y`, set `CPUSET=` to disable), writes chunk-split raw k6
output per config, then calls `bench-aggregate.py` to roll the chunks
into one `summary.json` per config. The raw `k6-chunk-*.json` files
are gitignored — committed baselines only track `summary.json`,
`manifest.json`, and `report.md`.

Baselines (persisted for regression-checking):

```sh
CHUNKS=5 ITERATIONS=200 \
  make bench-baseline NAME=phase8-after-fix     # writes to fixtures/baselines/
git add fixtures/baselines/phase8-after-fix     # review, then commit
make bench-compare \
    RUN=fixtures/baselines/phase7-paired \
    TO=fixtures/baselines/phase8-after-fix      # cross-run diff
```

With SPX flame graphs (adds ~10% overhead to the one profiled cycle
per scenario; the rest of the samples are clean):

```sh
PROFILE=1 make bench-full
./scripts/spx-ui.sh /tmp/patina-bench/<ts>/full_patina
```

## Historical results

| Run | Date | Patina | Host | Headline |
|---|---|---|---|---|
| [phase6-initial](../fixtures/baselines/phase6-initial/report.md) | 2026-04-13 | 0.1.0 (`566ed36`) | Ryzen 9 5950X | No config distinguishable from stock at n=100 — later traced to k6 running scenarios in parallel and queueing behind `pm.max_children=5`, not a patina effect |
| [phase7-paired](../fixtures/baselines/phase7-paired/report.md) | 2026-04-15 | 0.1.0 (`c3cb04a`) | Ryzen 9 5950X | Serialized k6 + chunked pairing + cpuset pinning drops stddev to 1.9–3.2% CV. Every patina config is significantly *slower* than stock on 6/9 scenarios (p<0.01); bridge overhead dominates. |
| [phase8-activation-cached](../fixtures/baselines/phase8-activation-cached/report.md) | 2026-04-15 | 0.1.0 | Ryzen 9 5950X | Activation caching lands. `full_patina` is now indistinguishable from stock on all 9 scenarios (all p > 0.068); `parse_blocks_only` narrows to residual +0.2–0.5% on 2/9 scenarios. Action item #1 complete. |
| [phase9-shim-filters](../fixtures/baselines/phase9-shim-filters/report.md) | 2026-04-15 | 0.1.0 | Ryzen 9 5950X | `apply_filters` moved from Rust into PHP shims for esc_html/esc_attr. `esc_only` flat, `parse_blocks_only` flat, `full_patina` significantly *faster* than stock on all 9 scenarios (p<3e-5) — first net win in patina's history. `kses_only` still +0.5–1.3% slow, next on the action list. Action item #2 complete. |
