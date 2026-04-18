# Benchmark content corpus

Fixture files used by `seed-benchmark-content.sh` to stand up a realistic
WordPress install for HTTP-level benchmarks.

| File | Blocks | Size | Purpose |
|---|---:|---:|---|
| `posts-short.html` | 15 | ~6 KB | baseline single-post scenario — flat block tree, realistic per-block content |
| `posts-medium.html` | 30 | ~13 KB | block variety across a medium-length body |
| `posts-long.html` | 60 | ~25 KB | parse_blocks win zone — long tail of body size |
| `classic-post.html` | — | — | pre-Gutenberg HTML — wpautop / render-time kses scenario |

All three block fixtures are deliberately flat (no group/columns nesting). Real-world
WordPress posts are overwhelmingly flat: nesting mostly comes from FSE templates,
which wrap post content but do not scale with it. Keeping the fixtures flat means the
block count the bench grep reports is the block count `parse_blocks` actually walks.
Per-block content averages ~400 B of prose with inline markup (bold, italic, code,
links, entities) so the render-time kses pass does non-trivial work on every block.

The canonical WXR block fixture (`64-block-test-data.xml` from
[WordPress/theme-test-data](https://github.com/WordPress/theme-test-data))
is **not** committed here. The seed script fetches it at run time, pinned
to the SHA in `WXR_PIN.env`, and caches it under `/tmp` inside the container
for subsequent runs. Bumping the pin is a one-line edit.

Rationale: the WXR is ~300 KB and would double the repo size. Pinning to
a SHA gives the same reproducibility with none of the bloat.
