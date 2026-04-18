# Patina bench report — phase7-paired

- Git SHA: `c3cb04a`
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
| archive_category | 200 | 28.3 → 28.5 | +0.7 | 28.3 → 28.6 | +0.8 | paired p=4.46e-05 *** | +1.9 ↑ |
| archive_tag | 200 | 26.6 → 26.8 | +0.8 | 26.7 → 26.9 | +0.8 | paired p=2.77e-05 *** | +2.8 ↑ |
| homepage | 200 | 43.9 → 44.1 | +0.6 | 43.9 → 44.1 | +0.6 | paired p=0.00712 ** | +1.5 ↑ |
| rest_posts | 200 | 43.8 → 44.0 | +0.5 | 43.8 → 44.0 | +0.4 | paired p=0.00628 ** | +1.1 ↑ |
| search | 200 | 48.2 → 48.5 | +0.7 | 48.2 → 48.5 | +0.6 | paired p=0.00024 *** | +1.5 ↑ |
| single_classic | 200 | 34.6 → 34.7 | +0.4 | 34.6 → 34.8 | +0.6 | paired p=0.000338 *** | +1.8 ↑ |
| single_commented | 200 | 45.5 → 45.7 | +0.5 | 45.5 → 45.8 | +0.7 | paired p=0.000138 *** | +1.6 ↑ |
| single_long | 200 | 36.9 → 37.1 | +0.7 | 36.9 → 37.1 | +0.6 | paired p=0.00056 *** | +3.0 ↑ |
| single_short | 200 | 35.2 → 35.5 | +0.8 | 35.3 → 35.6 | +0.7 | paired p=1.6e-05 *** | +2.1 ↑ |

## full_patina

Active overrides: `esc_html, esc_attr, wp_kses, parse_blocks`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 28.3 → 28.6 | +1.0 | 28.3 → 28.7 | +1.1 ↑ | paired p=1.79e-07 *** | +1.7 ↑ |
| archive_tag | 200 | 26.6 → 26.8 | +0.7 | 26.7 → 26.9 | +0.7 | paired p=0.000178 *** | +1.2 ↑ |
| homepage | 200 | 43.9 → 43.9 | -0.0 | 43.9 → 43.9 | +0.0 | paired p=0.852  | -0.1 |
| rest_posts | 200 | 43.8 → 43.7 | -0.3 | 43.8 → 43.7 | -0.2 | paired p=0.156  | -0.3 |
| search | 200 | 48.2 → 48.2 | -0.0 | 48.2 → 48.4 | +0.2 | paired p=0.0675  | +1.4 ↑ |
| single_classic | 200 | 34.6 → 35.0 | +1.2 ↑ | 34.6 → 35.0 | +1.2 ↑ | paired p=4.71e-09 *** | +1.5 ↑ |
| single_commented | 200 | 45.5 → 45.9 | +0.9 | 45.5 → 45.9 | +1.0 | paired p=3.45e-07 *** | +1.8 ↑ |
| single_long | 200 | 36.9 → 37.0 | +0.4 | 36.9 → 37.0 | +0.4 | paired p=0.0178 * | +1.2 ↑ |
| single_short | 200 | 35.2 → 35.6 | +1.1 ↑ | 35.3 → 35.7 | +1.0 ↑ | paired p=5.72e-08 *** | +2.3 ↑ |

## kses_only

Active overrides: `wp_kses`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 28.3 → 28.5 | +0.6 | 28.3 → 28.6 | +0.9 | paired p=1.69e-05 *** | +3.3 ↑ |
| archive_tag | 200 | 26.6 → 26.9 | +1.0 | 26.7 → 26.9 | +0.8 | paired p=0.000405 *** | +3.3 ↑ |
| homepage | 200 | 43.9 → 44.2 | +0.7 | 43.9 → 44.2 | +0.8 | paired p=0.000477 *** | +2.2 ↑ |
| rest_posts | 200 | 43.8 → 44.0 | +0.5 | 43.8 → 44.0 | +0.4 | paired p=0.0335 * | +0.8 |
| search | 200 | 48.2 → 48.4 | +0.4 | 48.2 → 48.4 | +0.4 | paired p=0.0178 * | +2.3 ↑ |
| single_classic | 200 | 34.6 → 34.8 | +0.6 | 34.6 → 34.8 | +0.5 | paired p=0.00167 ** | +3.3 ↑ |
| single_commented | 200 | 45.5 → 45.7 | +0.5 | 45.5 → 45.7 | +0.5 | paired p=0.00859 ** | +2.9 ↑ |
| single_long | 200 | 36.9 → 37.1 | +0.6 | 36.9 → 37.1 | +0.6 | paired p=0.00142 ** | +3.5 ↑ |
| single_short | 200 | 35.2 → 35.5 | +0.7 | 35.3 → 35.5 | +0.6 | paired p=0.000643 *** | +2.4 ↑ |

## parse_blocks_only

Active overrides: `parse_blocks`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 28.3 → 28.5 | +0.6 | 28.3 → 28.5 | +0.7 | paired p=0.000483 *** | +1.6 ↑ |
| archive_tag | 200 | 26.6 → 26.7 | +0.4 | 26.7 → 26.9 | +0.7 | paired p=0.000922 *** | +3.6 ↑ |
| homepage | 200 | 43.9 → 43.8 | -0.1 | 43.9 → 43.8 | -0.3 | paired p=0.332  | +0.8 |
| rest_posts | 200 | 43.8 → 43.6 | -0.3 | 43.8 → 43.7 | -0.2 | paired p=0.336  | -0.1 |
| search | 200 | 48.2 → 48.1 | -0.1 | 48.2 → 48.3 | +0.1 | paired p=0.336  | +0.9 |
| single_classic | 200 | 34.6 → 34.9 | +0.8 | 34.6 → 34.9 | +0.9 | paired p=6.43e-06 *** | +2.0 ↑ |
| single_commented | 200 | 45.5 → 45.7 | +0.5 | 45.5 → 45.8 | +0.6 | paired p=0.00134 ** | +1.5 ↑ |
| single_long | 200 | 36.9 → 37.0 | +0.5 | 36.9 → 37.1 | +0.7 | paired p=0.00116 ** | +2.6 ↑ |
| single_short | 200 | 35.2 → 35.6 | +1.1 ↑ | 35.3 → 35.7 | +1.1 ↑ | paired p=1.34e-08 *** | +3.4 ↑ |

