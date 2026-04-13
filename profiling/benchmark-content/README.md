# Benchmark content corpus

Fixture files used by `seed-benchmark-content.sh` to stand up a realistic
WordPress install for HTTP-level benchmarks.

| File | Purpose |
|---|---|
| `posts-short.html` | ~500 B block markup — baseline single-post scenario |
| `posts-medium.html` | ~3 KB block markup with block variety |
| `posts-long.html` | ~8 KB block markup with deep nesting — parse_blocks win zone |
| `classic-post.html` | Pre-Gutenberg HTML — wpautop / render-time kses scenario |

The canonical WXR block fixture (`64-block-test-data.xml` from
[WordPress/theme-test-data](https://github.com/WordPress/theme-test-data))
is **not** committed here. The seed script fetches it at run time, pinned
to the SHA in `WXR_PIN.env`, and caches it under `/tmp` inside the container
for subsequent runs. Bumping the pin is a one-line edit.

Rationale: the WXR is ~300 KB and would double the repo size. Pinning to
a SHA gives the same reproducibility with none of the bloat.
