# Patina benchmarks — end-to-end HTTP measurements

This document is the authoritative answer to "does patina make a real
WordPress pageload faster?" Unlike the per-function microbenchmarks in
`php/benchmarks/`, these numbers come from full HTTP requests against a
real WordPress install under controlled conditions, with the per-request
activation cost of `patina_activate()` paid on every sample.

The methodology, scenarios, and runner live under `scripts/bench-runner.sh`
and `profiling/k6-workloads.js`. The bench infrastructure itself is
described in `docs/BENCHMARK_PLAN.md`.

## Latest result — `phase6-initial` (2026-04-13)

**Headline**: at n=100 samples per scenario, **no patina configuration
showed a statistically significant delta (p<0.05) from stock WordPress**
on any of the 9 scenarios. Observed Δp95 across all 36 (config, scenario)
pairs sits between −14.9% and +16.4%, but every t-test p-value is between
0.33 and 1.00 — the within-scenario sample jitter (~10–15 ms stddev) is
large enough to swallow any patina effect on this workload.

This is the same gap the plan was designed to measure: per-function
microbenchmarks show 1.5–6.9× speedups, but on a real pageload the
bridge-overhead cost and the amortized-away activation cost bring the
net effect within noise of zero on moderate content.

### Setup

- **Host**: Peter-PC, AMD Ryzen 9 5950X (16 cores / 32 threads), CachyOS
- **Stack**: Docker — nginx:alpine + php-fpm 8.3.30 + mariadb:11
- **WordPress**: 6.9.4 (TwentyTwenty-Five active, FSE default theme)
- **Content**: 90 posts / 93 comments / 6 users — the WordPress
  theme-test-data WXR fixture plus the Phase 2 block-tier corpus
  (`profiling/benchmark-content/`)
- **Patina**: 0.1.0 @ `566ed36`
- **k6**: 100 post-warmup iterations per scenario per config, plus 5
  dropped warmup iterations. Sequential (vus=1), cache-busted query
  string, Host header forced to `localhost:8080` so WP's canonical-URL
  redirects don't fire

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
| `/a-short-block-post/` | ~500 B block single — baseline |
| `/a-long-block-post/` | ~8 KB block single — parse_blocks win zone |
| `/a-classic-html-post/` | Pre-Gutenberg HTML — wpautop + kses |
| `/a-commented-post/` | 20 comments — render-time kses x20 |
| `/category/announcements/` | Category archive |
| `/tag/perf/` | Tag archive |
| `/?s=lorem` | Search results |
| `/wp-json/wp/v2/posts` | REST API |

### Headline numbers (TTFB, ms)

Per-scenario p50 / p95 for the `stock` baseline and `full_patina`
candidate, n=100 each:

| Scenario | stock p50 / p95 | full_patina p50 / p95 | Δp50 | Δp95 |
|---|---:|---:|---:|---:|
| archive_category | 76.2 / 118.9 | 74.4 / 120.7 | −1.8 | +1.8 (+1.6%) |
| archive_tag | 75.5 / 116.5 | 74.9 / 117.8 | −0.6 | +1.3 (+1.2%) |
| homepage | 83.9 / 124.4 | 82.4 / 126.1 | −1.6 | +1.6 (+1.3%) |
| rest_posts | 78.4 / 116.6 | 76.9 / 116.8 | −1.4 | +0.3 (+0.2%) |
| search | 92.7 / 120.3 | 91.8 / 125.2 | −0.9 | +4.9 (+4.1%) |
| single_classic | 80.0 / 120.5 | 79.4 / 125.0 | −0.6 | +4.5 (+3.7%) |
| single_commented | 91.0 / 122.5 | 90.6 / 128.8 | −0.4 | +6.3 (+5.2%) |
| single_long | 81.4 / 119.1 | 81.1 / 135.5 | −0.3 | +16.4 (+13.7%) |
| single_short | 79.3 / 117.7 | 79.4 / 114.2 | +0.1 | −3.5 (−3.0%) |

**Every Welch's t-test p-value in the full per-override breakdown
exceeds 0.33.** The full matrix is in
`fixtures/baselines/phase6-initial/report.md` with confidence intervals
for all 36 (config, scenario) pairs.

### Analysis

1. **Patina's true effect on this workload is smaller than the noise
   floor at n=100.** The stock `stock` config has p95 jitter of ~10 ms
   on an ~80 ms TTFB, which is 12% relative noise. A 2–5% patina gain
   or loss cannot be detected without substantially more samples,
   lower-jitter infrastructure, or a workload that spends a larger
   share of each request in the functions patina overrides.

2. **p50 trends weakly favor full_patina across most scenarios**
   (−0.3 to −1.8 ms), but none of these cross significance either. The
   sign of the effect is consistent enough that a larger-n rerun might
   surface a 1–2% win, but this is speculation until measured.

3. **The `single_long` p95 regression (+13.7%, still p=0.77) is the most
   eye-catching cell but is almost certainly noise.** A single outlier
   request near the top of the sample set shifts p95 dramatically at
   n=100; p50 moved by only −0.3 ms on the same cell.

4. **This matches the April 2026 live-site measurements** recorded in
   `docs/BENCHMARK_PLAN.md` § "Related project history": bridge overhead
   on the esc_html / esc_attr / wp_kses paths costs roughly what the
   Rust implementations save on moderate content, leaving net effect
   near zero unless a request spends serious time inside `parse_blocks`
   (which none of these scenarios do — even `/a-long-block-post/` only
   has ~8 KB of body).

### What this means for the project

- **The microbenchmark numbers in `README.md` are still accurate** —
  the Rust implementations are genuinely 1.5–7× faster per function
  call — but they overstate the end-to-end effect because they amortize
  away the activation cost and don't include the bridge overhead.
- **The bridge-overhead issues listed in `docs/BENCHMARK_PLAN.md`
  § "Related project history" need fixing before the next bench run is
  meaningful**: `apply_filters` round-trips on escaping paths,
  `has_filter` checks on every `wp_kses` call, and the
  `patina_activate()`-on-every-request cost from the mu-plugin. All
  three are fixable; none have been fixed yet.
- **The next re-baseline should land after those fixes**, not before.
  Measuring noise repeatedly is not informative.

### Action items (in priority order)

1. **Cache `patina_activate()` after the first call per FPM worker.**
   A static flag in the bridge mu-plugin, plus idempotency at the Rust
   level, should remove the per-request activation cost entirely.
2. **Move `apply_filters('esc_html', ...)` back into the PHP shim.**
   Rust→PHP `call_user_func` is ~10× slower than PHP→PHP dispatch; a
   shim-level `apply_filters` call costs nothing but a user-function
   frame.
3. **Cache `has_filter()` results for `wp_kses_allowed_html`,
   `kses_allowed_protocols`, and `wp_kses_uri_attributes`** in a Rust
   `OnceCell` keyed per-request. These fire on every `wp_kses` call
   today and dominate the kses path's overhead.
4. **Re-baseline** at n=200 per scenario, with the three fixes landed.
   A 2× bump in samples cuts the confidence interval by √2, and the
   fixes should lift the true effect enough to peek above the noise.

## How to reproduce

Full matrix, ~15 minutes on the Ryzen above:

```sh
./profiling/setup-wordpress.sh                # one-time: seed the corpus
ITERATIONS=100 make bench-full                # full per-config matrix
make bench-compare RUN=/tmp/patina-bench/<ts>/  # render the report
```

Baselines (persisted for regression-checking):

```sh
make bench-baseline NAME=phase7-after-fixes   # writes to fixtures/baselines/
git add fixtures/baselines/phase7-after-fixes # review, then commit
make bench-compare RUN=fixtures/baselines/phase6-initial \
                   TO=fixtures/baselines/phase7-after-fixes
```

With SPX flame graphs (adds ~10% overhead to the one profiled sample
per scenario, rest of samples are clean):

```sh
PROFILE=1 make bench-full
./scripts/spx-ui.sh /tmp/patina-bench/<ts>/full_patina
```

## Historical results

| Run | Date | Patina | Host | Headline |
|---|---|---|---|---|
| [phase6-initial](../fixtures/baselines/phase6-initial/report.md) | 2026-04-13 | 0.1.0 (`566ed36`) | Ryzen 9 5950X | No config distinguishable from stock at n=100 |
