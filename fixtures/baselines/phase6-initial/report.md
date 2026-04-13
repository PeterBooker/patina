# Patina bench report — phase6-initial

- Git SHA: `566ed36`
- Patina version: 0.1.0
- PHP: 8.3.30 · WP: 6.9.4
- Host: Peter-PC (AMD Ryzen 9 5950X 16-Core Processor)
- Iterations per scenario: 100
- Baseline config: **stock**

Metric legend: `p50 / p95` in ms. `Δp95 %` is (candidate − baseline) / baseline × 100. `*`=p<0.05, `**`=p<0.01, `***`=p<0.001 (Welch's t-test, normal-approx p-value). ↓ means faster than baseline, ↑ means slower.

## esc_only

Active overrides: `esc_html, esc_attr`

| Scenario | baseline p50 / p95 | candidate p50 / p95 | Δp50 | Δp95 | stats |
|---|---|---|---:|---:|---|
| archive_category | 76.2 / 118.9 | 73.6 / 116.1 | -2.6 | -2.8 (-2.4%) ↓ | p=0.333 df=198  |
| archive_tag | 75.5 / 116.5 | 74.5 / 118.1 | -1.0 | +1.6 (+1.4%) ↑ | p=0.651 df=198  |
| homepage | 83.9 / 124.4 | 81.3 / 115.5 | -2.7 | -8.9 (-7.2%) ↓ | p=0.557 df=198  |
| rest_posts | 78.4 / 116.6 | 77.7 / 120.5 | -0.7 | +3.9 (+3.4%) ↑ | p=0.795 df=197  |
| search | 92.7 / 120.3 | 91.2 / 119.0 | -1.5 | -1.3 (-1.0%) ↓ | p=0.751 df=198  |
| single_classic | 80.0 / 120.5 | 79.3 / 117.6 | -0.7 | -2.8 (-2.4%) ↓ | p=0.618 df=198  |
| single_commented | 91.0 / 122.5 | 89.5 / 123.4 | -1.5 | +0.9 (+0.7%) | p=0.799 df=195  |
| single_long | 81.4 / 119.1 | 78.9 / 128.2 | -2.5 | +9.1 (+7.6%) ↑ | p=0.664 df=198  |
| single_short | 79.3 / 117.7 | 79.2 / 114.1 | -0.1 | -3.6 (-3.0%) ↓ | p=0.815 df=198  |

## full_patina

Active overrides: `esc_html, esc_attr, wp_kses, parse_blocks`

| Scenario | baseline p50 / p95 | candidate p50 / p95 | Δp50 | Δp95 | stats |
|---|---|---|---:|---:|---|
| archive_category | 76.2 / 118.9 | 74.4 / 120.7 | -1.8 | +1.8 (+1.6%) ↑ | p=0.709 df=198  |
| archive_tag | 75.5 / 116.5 | 74.9 / 117.8 | -0.6 | +1.3 (+1.2%) ↑ | p=0.847 df=195  |
| homepage | 83.9 / 124.4 | 82.4 / 126.1 | -1.6 | +1.6 (+1.3%) ↑ | p=0.853 df=197  |
| rest_posts | 78.4 / 116.6 | 76.9 / 116.8 | -1.4 | +0.3 (+0.2%) | p=0.418 df=198  |
| search | 92.7 / 120.3 | 91.8 / 125.2 | -0.9 | +4.9 (+4.1%) ↑ | p=0.933 df=198  |
| single_classic | 80.0 / 120.5 | 79.4 / 125.0 | -0.6 | +4.5 (+3.7%) ↑ | p=0.932 df=198  |
| single_commented | 91.0 / 122.5 | 90.6 / 128.8 | -0.4 | +6.3 (+5.2%) ↑ | p=0.847 df=196  |
| single_long | 81.4 / 119.1 | 81.1 / 135.5 | -0.3 | +16.4 (+13.7%) ↑ | p=0.773 df=194  |
| single_short | 79.3 / 117.7 | 79.4 / 114.2 | +0.1 | -3.5 (-3.0%) ↓ | p=0.766 df=195  |

## kses_only

Active overrides: `wp_kses`

| Scenario | baseline p50 / p95 | candidate p50 / p95 | Δp50 | Δp95 | stats |
|---|---|---|---:|---:|---|
| archive_category | 76.2 / 118.9 | 76.8 / 120.5 | +0.6 | +1.6 (+1.3%) ↑ | p=0.904 df=196  |
| archive_tag | 75.5 / 116.5 | 74.2 / 120.8 | -1.3 | +4.3 (+3.7%) ↑ | p=0.831 df=196  |
| homepage | 83.9 / 124.4 | 83.8 / 124.0 | -0.1 | -0.4 (-0.3%) | p=0.648 df=198  |
| rest_posts | 78.4 / 116.6 | 80.4 / 124.4 | +2.1 | +7.8 (+6.7%) ↑ | p=0.8 df=198  |
| search | 92.7 / 120.3 | 91.8 / 130.0 | -0.9 | +9.7 (+8.1%) ↑ | p=0.998 df=198  |
| single_classic | 80.0 / 120.5 | 79.3 / 127.2 | -0.6 | +6.7 (+5.5%) ↑ | p=0.936 df=197  |
| single_commented | 91.0 / 122.5 | 89.6 / 129.4 | -1.4 | +6.8 (+5.6%) ↑ | p=0.987 df=197  |
| single_long | 81.4 / 119.1 | 82.2 / 126.8 | +0.8 | +7.7 (+6.4%) ↑ | p=0.771 df=197  |
| single_short | 79.3 / 117.7 | 80.9 / 121.5 | +1.6 | +3.8 (+3.2%) ↑ | p=0.502 df=198  |

## parse_blocks_only

Active overrides: `parse_blocks`

| Scenario | baseline p50 / p95 | candidate p50 / p95 | Δp50 | Δp95 | stats |
|---|---|---|---:|---:|---|
| archive_category | 76.2 / 118.9 | 75.8 / 114.6 | -0.4 | -4.3 (-3.6%) ↓ | p=0.658 df=198  |
| archive_tag | 75.5 / 116.5 | 75.2 / 116.8 | -0.3 | +0.3 (+0.3%) | p=0.891 df=197  |
| homepage | 83.9 / 124.4 | 81.9 / 109.5 | -2.0 | -14.9 (-12.0%) ↓ | p=0.782 df=198  |
| rest_posts | 78.4 / 116.6 | 78.5 / 118.3 | +0.2 | +1.8 (+1.5%) ↑ | p=0.771 df=198  |
| search | 92.7 / 120.3 | 89.9 / 119.2 | -2.9 | -1.1 (-0.9%) | p=0.556 df=198  |
| single_classic | 80.0 / 120.5 | 78.8 / 118.1 | -1.1 | -2.3 (-1.9%) ↓ | p=0.68 df=194  |
| single_commented | 91.0 / 122.5 | 88.2 / 131.2 | -2.9 | +8.7 (+7.1%) ↑ | p=0.537 df=198  |
| single_long | 81.4 / 119.1 | 79.6 / 118.5 | -1.8 | -0.7 (-0.6%) | p=0.937 df=198  |
| single_short | 79.3 / 117.7 | 79.0 / 132.5 | -0.3 | +14.8 (+12.6%) ↑ | p=0.849 df=193  |

