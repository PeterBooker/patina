# Patina bench report — phase10-sanitize-title

- Git SHA: `e1f5ec3`
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
| archive_category | 200 | 27.8 → 27.6 | -0.6 | 28.0 → 27.9 | -0.4 | paired p=0.161  | -1.4 ↓ |
| archive_tag | 200 | 26.1 → 26.0 | -0.4 | 26.3 → 26.2 | -0.4 | paired p=0.1  | -0.7 |
| homepage | 200 | 42.9 → 42.8 | -0.3 | 43.2 → 43.2 | +0.0 | paired p=0.735  | +0.2 |
| rest_posts | 200 | 42.8 → 42.6 | -0.7 | 43.0 → 43.0 | -0.2 | paired p=0.842  | +1.7 ↑ |
| search | 200 | 47.1 → 46.9 | -0.3 | 47.4 → 47.3 | -0.2 | paired p=0.151  | -1.2 ↓ |
| single_classic | 200 | 33.8 → 33.8 | -0.2 | 34.2 → 33.9 | -0.6 | paired p=0.0778  | -1.2 ↓ |
| single_commented | 200 | 44.5 → 44.2 | -0.8 | 44.9 → 44.6 | -0.6 | paired p=0.0297 * | -1.6 ↓ |
| single_long | 200 | 36.0 → 36.0 | -0.1 | 36.4 → 36.3 | -0.0 | paired p=0.738  | -1.4 ↓ |
| single_short | 200 | 34.6 → 34.4 | -0.4 | 34.9 → 34.7 | -0.4 | paired p=0.42  | +0.3 |

## full_patina

Active overrides: `esc_html, esc_attr, wp_kses, parse_blocks, sanitize_title_with_dashes`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 27.8 → 27.6 | -0.7 | 28.0 → 27.5 | -1.6 ↓ | paired p=1.4e-05 *** | -5.7 ↓ |
| archive_tag | 200 | 26.1 → 25.9 | -0.6 | 26.3 → 26.0 | -1.3 ↓ | paired p=0.000297 *** | -5.6 ↓ |
| homepage | 200 | 42.9 → 42.8 | -0.3 | 43.2 → 42.9 | -0.8 | paired p=0.00227 ** | -4.0 ↓ |
| rest_posts | 200 | 42.8 → 42.6 | -0.6 | 43.0 → 42.8 | -0.5 | paired p=0.0552  | -0.1 |
| search | 200 | 47.1 → 46.8 | -0.7 | 47.4 → 46.9 | -1.1 ↓ | paired p=0.000434 *** | -3.3 ↓ |
| single_classic | 200 | 33.8 → 33.6 | -0.7 | 34.2 → 33.7 | -1.4 ↓ | paired p=0.000445 *** | -5.5 ↓ |
| single_commented | 200 | 44.5 → 44.3 | -0.4 | 44.9 → 44.4 | -1.1 ↓ | paired p=0.00228 ** | -2.8 ↓ |
| single_long | 200 | 36.0 → 35.8 | -0.5 | 36.4 → 35.9 | -1.2 ↓ | paired p=0.000638 *** | -5.6 ↓ |
| single_short | 200 | 34.6 → 34.4 | -0.4 | 34.9 → 34.5 | -1.2 ↓ | paired p=0.00017 *** | -6.0 ↓ |

## kses_only

Active overrides: `wp_kses`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 27.8 → 27.7 | -0.2 | 28.0 → 27.9 | -0.3 | paired p=0.19  | -2.3 ↓ |
| archive_tag | 200 | 26.1 → 26.2 | +0.2 | 26.3 → 26.3 | -0.3 | paired p=0.339  | -2.0 ↓ |
| homepage | 200 | 42.9 → 43.1 | +0.5 | 43.2 → 43.3 | +0.2 | paired p=0.583  | -0.6 |
| rest_posts | 200 | 42.8 → 42.9 | +0.0 | 43.0 → 43.1 | +0.1 | paired p=0.858  | +1.7 ↑ |
| search | 200 | 47.1 → 47.1 | +0.0 | 47.4 → 47.4 | -0.1 | paired p=0.832  | -0.5 |
| single_classic | 200 | 33.8 → 33.9 | +0.2 | 34.2 → 34.1 | -0.2 | paired p=0.899  | +0.7 |
| single_commented | 200 | 44.5 → 44.7 | +0.3 | 44.9 → 44.8 | -0.2 | paired p=0.251  | -2.4 ↓ |
| single_long | 200 | 36.0 → 36.2 | +0.5 | 36.4 → 36.4 | +0.0 | paired p=0.819  | -1.7 ↓ |
| single_short | 200 | 34.6 → 34.6 | +0.2 | 34.9 → 34.8 | -0.2 | paired p=0.453  | -1.6 ↓ |

## parse_blocks_only

Active overrides: `parse_blocks`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 27.8 → 27.3 | -1.5 ↓ | 28.0 → 27.4 | -2.2 ↓ | paired p=7.57e-09 *** | -8.1 ↓ |
| archive_tag | 200 | 26.1 → 25.7 | -1.3 ↓ | 26.3 → 25.8 | -2.1 ↓ | paired p=5e-09 *** | -7.4 ↓ |
| homepage | 200 | 42.9 → 42.6 | -0.7 | 43.2 → 42.7 | -1.3 ↓ | paired p=2.65e-07 *** | -6.4 ↓ |
| rest_posts | 200 | 42.8 → 42.5 | -0.9 | 43.0 → 42.5 | -1.2 ↓ | paired p=4.77e-06 *** | -4.1 ↓ |
| search | 200 | 47.1 → 46.5 | -1.2 ↓ | 47.4 → 46.6 | -1.7 ↓ | paired p=2.45e-09 *** | -6.8 ↓ |
| single_classic | 200 | 33.8 → 33.4 | -1.3 ↓ | 34.2 → 33.5 | -2.0 ↓ | paired p=4.27e-09 *** | -7.7 ↓ |
| single_commented | 200 | 44.5 → 44.0 | -1.2 ↓ | 44.9 → 44.0 | -1.9 ↓ | paired p=2.97e-10 *** | -7.6 ↓ |
| single_long | 200 | 36.0 → 35.6 | -1.2 ↓ | 36.4 → 35.6 | -2.0 ↓ | paired p=5.99e-10 *** | -8.8 ↓ |
| single_short | 200 | 34.6 → 34.3 | -0.8 | 34.9 → 34.3 | -1.7 ↓ | paired p=5.3e-07 *** | -7.4 ↓ |

## sanitize_title_only

Active overrides: `sanitize_title_with_dashes`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 27.8 → 27.6 | -0.5 | 28.0 → 27.7 | -1.0 | paired p=0.00116 ** | -5.3 ↓ |
| archive_tag | 200 | 26.1 → 26.1 | -0.1 | 26.3 → 26.1 | -1.0 ↓ | paired p=0.000746 *** | -6.8 ↓ |
| homepage | 200 | 42.9 → 42.8 | -0.2 | 43.2 → 42.9 | -0.8 | paired p=0.000294 *** | -5.4 ↓ |
| rest_posts | 200 | 42.8 → 42.8 | -0.1 | 43.0 → 42.8 | -0.5 | paired p=0.00304 ** | -3.6 ↓ |
| search | 200 | 47.1 → 46.9 | -0.4 | 47.4 → 47.0 | -1.0 | paired p=9.6e-05 *** | -5.6 ↓ |
| single_classic | 200 | 33.8 → 33.7 | -0.4 | 34.2 → 33.8 | -1.1 ↓ | paired p=9.03e-05 *** | -6.0 ↓ |
| single_commented | 200 | 44.5 → 44.3 | -0.4 | 44.9 → 44.4 | -1.2 ↓ | paired p=6.11e-06 *** | -7.2 ↓ |
| single_long | 200 | 36.0 → 36.1 | +0.1 | 36.4 → 36.1 | -0.7 | paired p=0.00497 ** | -6.2 ↓ |
| single_short | 200 | 34.6 → 34.5 | -0.3 | 34.9 → 34.6 | -1.0 | paired p=0.000656 *** | -5.4 ↓ |

