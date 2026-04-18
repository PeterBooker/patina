# Patina bench report — phase9-shim-filters

- Git SHA: `a020e13`
- Patina version: 0.1.0
- PHP: 8.3.30 · WP: 6.9.4
- Host: Peter-PC (AMD Ryzen 9 5950X 16-Core Processor)
- Iterations per scenario: 200
- Baseline config: **stock**

Headline metrics: **p50** (median) and **tmean** (10%-trimmed mean, drops the 10 fastest + 10 slowest samples before averaging). Both are robust to single-request outliers. `Δ %` is (cand − base) / base × 100; ↓ = faster, ↑ = slower. The `p95 Δ %` column is kept as a tail-latency check but is **not** the pass/fail signal — its confidence interval at n≤200 is too wide to base decisions on. `*`=p<0.05, `**`=p<0.01, `***`=p<0.001; intra-run rows use a paired t-test (each sample paired by chunk-matched index against the stock config), cross-run rows use Welch's.

## esc_only

Active overrides: `esc_html, esc_attr`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 33.7 → 33.7 | -0.0 | 33.8 → 33.8 | -0.1 | paired p=0.647  | -0.6 |
| archive_tag | 200 | 32.1 → 32.1 | +0.0 | 32.2 → 32.1 | -0.4 | paired p=0.15  | -0.1 |
| homepage | 200 | 50.3 → 50.3 | -0.1 | 50.4 → 50.4 | +0.1 | paired p=0.802  | +0.7 |
| rest_posts | 200 | 49.9 → 50.0 | +0.1 | 50.0 → 50.0 | +0.0 | paired p=0.558  | +0.1 |
| search | 200 | 56.3 → 56.3 | +0.0 | 56.3 → 56.3 | +0.1 | paired p=0.872  | +0.2 |
| single_classic | 200 | 41.1 → 41.1 | -0.0 | 41.2 → 41.1 | -0.3 | paired p=0.174  | -0.4 |
| single_commented | 200 | 52.9 → 52.9 | +0.0 | 52.9 → 52.9 | +0.0 | paired p=0.897  | +0.3 |
| single_long | 200 | 43.6 → 43.6 | -0.0 | 43.7 → 43.7 | +0.0 | paired p=0.976  | +0.4 |
| single_short | 200 | 41.8 → 41.8 | -0.0 | 41.9 → 41.9 | -0.0 | paired p=0.84  | +0.2 |

## full_patina

Active overrides: `esc_html, esc_attr, wp_kses, parse_blocks`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 33.7 → 33.4 | -1.0 ↓ | 33.8 → 33.5 | -1.0 ↓ | paired p=2.7e-05 *** | -0.6 |
| archive_tag | 200 | 32.1 → 31.7 | -1.4 ↓ | 32.2 → 31.7 | -1.6 ↓ | paired p=3.28e-10 *** | -1.7 ↓ |
| homepage | 200 | 50.3 → 50.0 | -0.6 | 50.4 → 49.9 | -0.9 | paired p=2.72e-06 *** | -0.9 |
| rest_posts | 200 | 49.9 → 49.5 | -1.0 | 50.0 → 49.6 | -0.9 | paired p=4.32e-05 *** | +0.1 |
| search | 200 | 56.3 → 55.8 | -0.8 | 56.3 → 55.9 | -0.8 | paired p=1.38e-06 *** | -0.2 |
| single_classic | 200 | 41.1 → 40.8 | -0.9 | 41.2 → 40.8 | -1.1 ↓ | paired p=2.41e-06 *** | -0.2 |
| single_commented | 200 | 52.9 → 52.4 | -0.9 | 52.9 → 52.5 | -0.8 | paired p=1.89e-05 *** | -0.8 |
| single_long | 200 | 43.6 → 43.2 | -1.0 | 43.7 → 43.2 | -1.1 ↓ | paired p=2.51e-07 *** | -1.1 ↓ |
| single_short | 200 | 41.8 → 41.4 | -0.9 | 41.9 → 41.3 | -1.2 ↓ | paired p=3.42e-06 *** | -0.1 |

## kses_only

Active overrides: `wp_kses`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 33.7 → 34.0 | +0.9 | 33.8 → 34.0 | +0.7 | paired p=0.0207 * | +1.6 ↑ |
| archive_tag | 200 | 32.1 → 32.4 | +0.8 | 32.2 → 32.4 | +0.6 | paired p=0.0808  | -0.0 |
| homepage | 200 | 50.3 → 50.7 | +0.7 | 50.4 → 50.7 | +0.5 | paired p=0.139  | -0.2 |
| rest_posts | 200 | 49.9 → 50.2 | +0.5 | 50.0 → 50.3 | +0.5 | paired p=0.0783  | -0.8 |
| search | 200 | 56.3 → 56.7 | +0.8 | 56.3 → 56.8 | +0.9 | paired p=0.00181 ** | +0.5 |
| single_classic | 200 | 41.1 → 41.6 | +1.3 ↑ | 41.2 → 41.6 | +0.8 | paired p=0.0363 * | +0.3 |
| single_commented | 200 | 52.9 → 53.3 | +0.7 | 52.9 → 53.3 | +0.8 | paired p=0.00176 ** | +0.3 |
| single_long | 200 | 43.6 → 44.1 | +1.1 ↑ | 43.7 → 44.0 | +0.8 | paired p=0.031 * | -0.0 |
| single_short | 200 | 41.8 → 42.1 | +0.6 | 41.9 → 42.1 | +0.5 | paired p=0.212  | -0.3 |

## parse_blocks_only

Active overrides: `parse_blocks`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 33.7 → 33.8 | +0.2 | 33.8 → 33.8 | +0.2 | paired p=0.702  | +0.8 |
| archive_tag | 200 | 32.1 → 32.1 | +0.1 | 32.2 → 32.1 | -0.4 | paired p=0.0538  | +0.9 |
| homepage | 200 | 50.3 → 50.5 | +0.3 | 50.4 → 50.5 | +0.2 | paired p=0.508  | -0.0 |
| rest_posts | 200 | 49.9 → 50.2 | +0.6 | 50.0 → 50.3 | +0.5 | paired p=0.987  | +0.1 |
| search | 200 | 56.3 → 56.5 | +0.4 | 56.3 → 56.6 | +0.5 | paired p=0.919  | +0.7 |
| single_classic | 200 | 41.1 → 41.3 | +0.5 | 41.2 → 41.2 | +0.0 | paired p=0.461  | +1.5 ↑ |
| single_commented | 200 | 52.9 → 53.1 | +0.3 | 52.9 → 53.0 | +0.1 | paired p=0.287  | -0.4 |
| single_long | 200 | 43.6 → 43.7 | +0.1 | 43.7 → 43.7 | +0.1 | paired p=0.254  | +0.4 |
| single_short | 200 | 41.8 → 41.9 | +0.2 | 41.9 → 42.0 | +0.2 | paired p=0.654  | +1.1 ↑ |

