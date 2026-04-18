# Patina bench report — phase8-activation-cached

- Git SHA: `47e115f`
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
| archive_category | 200 | 28.4 → 28.3 | -0.4 | 28.4 → 28.3 | -0.2 | paired p=0.965  | +0.7 |
| archive_tag | 200 | 26.7 → 26.6 | -0.3 | 26.7 → 26.6 | -0.4 | paired p=0.388  | -0.8 |
| homepage | 200 | 43.9 → 43.8 | -0.2 | 43.9 → 43.8 | -0.2 | paired p=0.807  | +0.1 |
| rest_posts | 200 | 43.7 → 43.7 | -0.1 | 43.7 → 43.7 | -0.1 | paired p=0.441  | +0.1 |
| search | 200 | 48.1 → 48.0 | -0.1 | 48.1 → 48.1 | -0.0 | paired p=0.385  | +0.0 |
| single_classic | 200 | 34.5 → 34.5 | -0.0 | 34.5 → 34.5 | -0.2 | paired p=0.805  | +0.2 |
| single_commented | 200 | 45.4 → 45.3 | -0.1 | 45.4 → 45.4 | +0.1 | paired p=0.0939  | +1.3 ↑ |
| single_long | 200 | 36.8 → 36.6 | -0.6 | 36.8 → 36.7 | -0.5 | paired p=0.0682  | -1.6 ↓ |
| single_short | 200 | 35.2 → 35.1 | -0.3 | 35.2 → 35.2 | -0.2 | paired p=0.549  | -1.7 ↓ |

## full_patina

Active overrides: `esc_html, esc_attr, wp_kses, parse_blocks`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 28.4 → 28.3 | -0.4 | 28.4 → 28.3 | -0.2 | paired p=0.652  | +2.5 ↑ |
| archive_tag | 200 | 26.7 → 26.6 | -0.4 | 26.7 → 26.6 | -0.5 | paired p=0.0684  | -0.9 |
| homepage | 200 | 43.9 → 43.8 | -0.4 | 43.9 → 43.8 | -0.3 | paired p=0.35  | -0.7 |
| rest_posts | 200 | 43.7 → 43.6 | -0.2 | 43.7 → 43.6 | -0.2 | paired p=0.0713  | -0.7 |
| search | 200 | 48.1 → 48.0 | -0.1 | 48.1 → 48.1 | -0.1 | paired p=0.203  | -0.4 |
| single_classic | 200 | 34.5 → 34.5 | -0.1 | 34.5 → 34.5 | -0.0 | paired p=0.679  | +0.6 |
| single_commented | 200 | 45.4 → 45.3 | -0.2 | 45.4 → 45.4 | -0.1 | paired p=0.72  | +0.5 |
| single_long | 200 | 36.8 → 36.8 | +0.0 | 36.8 → 36.8 | -0.0 | paired p=0.463  | -2.5 ↓ |
| single_short | 200 | 35.2 → 35.2 | -0.1 | 35.2 → 35.2 | +0.0 | paired p=0.606  | -1.5 ↓ |

## kses_only

Active overrides: `wp_kses`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 28.4 → 28.4 | -0.1 | 28.4 → 28.4 | +0.2 | paired p=0.19  | +2.3 ↑ |
| archive_tag | 200 | 26.7 → 26.7 | +0.0 | 26.7 → 26.7 | +0.1 | paired p=0.93  | +0.8 |
| homepage | 200 | 43.9 → 44.0 | +0.3 | 43.9 → 44.0 | +0.3 | paired p=0.393  | -0.0 |
| rest_posts | 200 | 43.7 → 43.9 | +0.5 | 43.7 → 43.9 | +0.3 | paired p=0.295  | -0.2 |
| search | 200 | 48.1 → 48.1 | +0.1 | 48.1 → 48.2 | +0.1 | paired p=0.773  | +0.7 |
| single_classic | 200 | 34.5 → 34.6 | +0.3 | 34.5 → 34.6 | +0.1 | paired p=0.988  | -1.6 ↓ |
| single_commented | 200 | 45.4 → 45.4 | +0.0 | 45.4 → 45.5 | +0.2 | paired p=0.224  | +1.3 ↑ |
| single_long | 200 | 36.8 → 36.8 | +0.0 | 36.8 → 36.9 | +0.2 | paired p=0.532  | -0.9 |
| single_short | 200 | 35.2 → 35.3 | +0.3 | 35.2 → 35.4 | +0.5 | paired p=0.134  | +1.2 ↑ |

## parse_blocks_only

Active overrides: `parse_blocks`

| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |
|---|---:|---|---:|---|---:|---|---:|
| archive_category | 200 | 28.4 → 28.4 | -0.1 | 28.4 → 28.4 | +0.2 | paired p=0.0173 * | +9.6 ↑ |
| archive_tag | 200 | 26.7 → 26.6 | -0.4 | 26.7 → 26.7 | -0.2 | paired p=0.218  | +3.9 ↑ |
| homepage | 200 | 43.9 → 43.9 | +0.0 | 43.9 → 44.0 | +0.2 | paired p=0.0565  | +3.3 ↑ |
| rest_posts | 200 | 43.7 → 43.8 | +0.2 | 43.7 → 43.9 | +0.3 | paired p=0.0162 * | +3.1 ↑ |
| search | 200 | 48.1 → 48.2 | +0.3 | 48.1 → 48.3 | +0.3 | paired p=0.0619  | +6.3 ↑ |
| single_classic | 200 | 34.5 → 34.6 | +0.5 | 34.5 → 34.7 | +0.5 | paired p=0.0173 * | +5.5 ↑ |
| single_commented | 200 | 45.4 → 45.3 | -0.2 | 45.4 → 45.5 | +0.3 | paired p=0.00841 ** | +5.8 ↑ |
| single_long | 200 | 36.8 → 36.8 | +0.0 | 36.8 → 36.9 | +0.2 | paired p=0.123  | +3.8 ↑ |
| single_short | 200 | 35.2 → 35.2 | -0.2 | 35.2 → 35.2 | -0.0 | paired p=0.36  | +4.0 ↑ |

