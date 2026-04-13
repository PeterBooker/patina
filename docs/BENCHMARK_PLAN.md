# Benchmark Refactor Plan

Status: **complete** — Q1–Q5 decided 2026-04-13, Phases 1–6 implemented, first baseline committed under `fixtures/baselines/phase6-initial/`. See `docs/BENCHMARKS.md` for the headline results and follow-up work
Author: drafted 2026-04-12 by Claude in a planning session with Peter
Supersedes: parts of `docs/IMPLEMENTATION_PLAN.md` that reference benchmarking

## Decisions (2026-04-13)

| # | Question | Decision |
|---|---|---|
| Q1 | Theme | **TwentyTwenty-Five** (FSE, default block theme) |
| Q2 | Content source | **WordPress/theme-test-data** (the canonical WXR fixture upstream WP uses for theme QA) |
| Q3 | CI integration | **On-demand only** for v1 (manual `gh workflow run`) |
| Q4 | Baseline persistence | **Commit JSON baselines to the repo** under `fixtures/baselines/` |
| Q5 | Consumer model | **Design for CI compatibility from the start**, wire up local-only for v1 |

## Why this plan exists

Patina's existing benchmarks are **function-level microbenchmarks only**. They run tight CLI loops (`hrtime(true)` around 10 000–50 000 iterations of `wp_kses_post` / `esc_html` / etc.) inside a single PHP process. The per-request activation cost of `patina_activate()` is amortized to near-zero across those iterations, so the reported speedups (2.7–6.9× for `wp_kses_post`, 4.2–5.8× for `esc_html`) overstate what you see on a real pageload — a real request pays the full activation cost once, pays the per-call filter-bridge crossings once per call (not amortized), and spends most of its time on work the microbenchmark doesn't measure at all (DB queries, template rendering, theme chrome).

This was caught the hard way during an April 2026 measurement session against `peterbooker.com`:

| Configuration | Live-site TTFB mean | vs stock |
|---|---:|---:|
| Stock WordPress | 163.3 ms | baseline |
| Patina (esc + kses only, 0.1.0 first release) | 197.8 ms | **+34.5 ms (slower)** |
| Patina (esc + kses + `parse_blocks`) | 175.4 ms (at a 185 ms baseline) | −9.6 ms (faster) |

The delta between "micro-benchmarks say 3–7× faster" and "real pageload is 5–20% faster, after being 20% slower with fewer overrides" is the gap this plan exists to close. We need to measure end-to-end HTTP request times on a realistic WordPress install, under controlled conditions, with the ability to isolate individual overrides.

## Goals

1. Answer "does patina make a real WordPress pageload faster, and by how much?" with statistical confidence, on reproducible workloads.
2. Decompose patina's effect into per-override contributions so we know which are net wins, which are losses, and on which workloads.
3. Catch regressions automatically — a new override that looks good in micro-benchmarks but tanks a real request should fail a bench check.
4. Make all of this reproducible from a fresh clone in <10 minutes of local dev time.

## Scope

**In scope:**
- Seeded WP install with realistic content (multiple posts with blocks, pages, comments, users, menus, widgets, a default FSE theme)
- HTTP-level load generation for measuring TTFB and total time per scenario
- Per-override runtime toggles (no rebuild required) so we can A/B individual overrides
- SPX profiler integration so flame graphs can accompany bench runs
- Persisted JSON results with a comparison/diff tool
- `make bench-full` / `make bench-compare` targets

**Out of scope for v1:**
- Multisite, WooCommerce, non-English, custom heavy themes
- Grafana/Prometheus dashboards (k6's JSON output is the interface)
- Full CI integration — will be added after we've validated variance characteristics
- Media uploads (images/video) in the seed corpus — adds complexity without clear benchmark signal

## Current state inventory

### What exists

- **Docker stack** at `profiling/docker-compose.yml` — nginx + php-fpm + mariadb, SPX 0.4.17 installed with HTTP access via `?SPX_UI_URI=/&SPX_KEY=dev` or `Cookie: SPX_ENABLED=1; SPX_KEY=dev`
- **Manual WP seeding** at `profiling/setup-wordpress.sh` — downloads WP, runs `wp core install`, creates **one** hand-written post with ~200 bytes of mixed HTML
- **Existing bench scripts:**
  - `php/benchmarks/run.php` — CLI microbench for `esc_html` / `esc_attr` / `wp_sanitize_redirect`
  - `php/benchmarks/bench-kses.php` — still CLI, but loads WP via `wp-load.php`; respects `PATINA_DISABLE` env var
  - `php/benchmarks/Runner.php` — harness class for paired PHP-vs-Rust measurement
  - `php/benchmarks/reference/` — verbatim PHP implementations of overridden functions for side-by-side comparison
- **Makefile targets:** `make bench` (runs `run.php`), `make bench-wp` (runs `bench-kses.php` inside the profiling stack), `make bench-jit` (same with JIT enabled), `make bench-rust` (Criterion in patina-bench), `make fixtures` (runs the fixture generator)
- **`profiling/k6-workloads.js`** — a decent k6 script skeleton listing 5 scenarios (homepage, single post, archive, search, REST API) and defining profiling + load-test executors. **But k6 itself is not installed in `Dockerfile.profiling`** — the file is orphaned.
- **`profiling/conf/spx.ini`** — SPX enabled, HTTP key `dev`, IP whitelist `*`. Ready for use.

### What's missing

- **Realistic content**: no blocks, no comments, no archive pages with >1 post, no theme, no menus, no sidebar widgets. Bench scenarios that need these can't run against the current seed.
- **HTTP-level measurement**: every bench is a CLI loop. Patina's per-request activation is amortized to zero and the per-call bridge crossings are measured once per million rather than once per call.
- **Per-override toggles**: there's only `PATINA_DISABLE=1` (all-or-nothing). No way to measure "esc-only" or "parse_blocks-only" contributions without rebuilding the `.so`.
- **Persistence**: no bench results are saved anywhere. No historical comparison possible. No baseline committed to the repo.
- **Comparison tooling**: no script that takes two runs and says "scenario X regressed by Y ms, here's the confidence interval."
- **Regression detection**: no way for CI (or a human) to know if a patina change made a specific workload worse.

## Proposed architecture

Four layers, build bottom-up.

```
┌──────────────────────────────────────────────────────────┐
│ Layer 4 — Reporting                                      │
│ compare JSON runs → markdown delta report                │
│ regression detection (CI-friendly exit codes)            │
├──────────────────────────────────────────────────────────┤
│ Layer 3 — Measurement                                    │
│ k6 HTTP runner (per-scenario, sequential mode)           │
│ SPX per-request profiles (flame graphs for deep dives)   │
│ JSON output with {config, scenario, sample, ttfb, ...}   │
├──────────────────────────────────────────────────────────┤
│ Layer 2 — Control                                        │
│ per-override runtime toggles (PATINA_DISABLE_ESC, etc.)  │
│ bench runner iterates through configs                    │
├──────────────────────────────────────────────────────────┤
│ Layer 1 — Environment                                    │
│ Docker stack, seeded WP with realistic content corpus    │
│ one command from empty volume to ready-to-bench          │
└──────────────────────────────────────────────────────────┘
```

## Phased build

Each phase is an independent PR. No phase depends on a later phase landing first — the system is usable after Phase 4, with Phases 5 and 6 as polish / dogfood.

### Phase 1 — Foundation: HTTP runner that actually works

**Goal:** replace CLI loops with HTTP-level measurement on the existing manually-seeded WP.

**Deliverables:**
- Install k6 in `profiling/Dockerfile.profiling` (single binary, `RUN apt-get install -y gnupg && curl -fsSL ... | apt-key add - && echo "deb https://dl.k6.io/deb stable main" > /etc/apt/sources.list.d/k6.list && apt-get update && apt-get install -y k6`)
- Rewrite `profiling/k6-workloads.js`:
  - Use `per-vu-iterations` executor only, drop the `load_test` scenario (conflates throughput with latency — bad for pointwise comparisons)
  - Cache-bust every request via `?t=${__ITER}_${__VU}_${Date.now()}`
  - Track `http_req_waiting` (TTFB) and `http_req_duration` (total) as **separate** trends
  - Emit JSON via `--out json=` at run time, one file per run
  - One trend per scenario with `tags: { scenario: '...' }` so downstream analysis can filter
  - Remove `sleep(0.1)` — it pollutes timings and isn't needed for this workload
- `make bench-http` target: starts stack if needed, runs k6 against the current content, writes JSON to `/tmp/patina-bench/<timestamp>/k6-output.json`
- Short README update explaining how to run it

**Not in this phase:** content seeding changes, per-override toggles, diff reporting. Just get HTTP-level measurement working end-to-end against the current one-post seed.

**Effort:** ~0.5 day. **Risk:** low.

### Phase 2 — Content corpus: realistic WordPress

**Goal:** replace the one-hand-written-post seed with a realistic WP site so measurements reflect real workloads.

**Deliverables:**
- `profiling/benchmark-content/` directory:
  - `block-test-data.xml` — vendored copy of WordPress's own `64-block-test-data.xml` from [WordPress/theme-test-data](https://github.com/WordPress/theme-test-data). This is what WP core itself uses for theme QA — canonical realistic block fixture.
  - `posts-short.html`, `posts-medium.html`, `posts-long.html` — hand-curated block markup in three size tiers, for consistent-shape archive testing
  - `config.yaml` (or just a bash list) recording what the seeder creates
- `profiling/seed-benchmark-content.sh`:
  1. Install TwentyTwenty-Five (FSE, default block theme)
  2. Wipe existing content (simpler than tracking idempotency — `wp post delete $(wp post list --format=ids)`, `wp comment delete --force`, etc.)
  3. Import the WXR block-test fixture via `wp import` (requires wordpress-importer plugin, install on the fly)
  4. `wp post generate --count=10 --post_type=post --post_content="$(cat posts-short.html)"` for each size tier
  5. Create a page or two via `wp post create --post_type=page`
  6. Generate users via `wp user generate --count=5`
  7. Generate comments via `wp comment generate --count=40 --post_id=<post_id>` (spread across several posts)
  8. Create nav menu + assign locations
  9. Activate default widgets in the sidebar
  10. Flush rewrites, warm opcache with 3 curl requests
- Modify `setup-wordpress.sh` to call the seeder after core install
- Make the seeder idempotent-ish: always wipes first, then seeds — running twice is safe, just wastes time

**Target content shape** — "realistic" = what a small personal blog looks like:

| Entity | Count | Notes |
|---|---:|---|
| Posts (short, block-based) | 10 | ~500 B body |
| Posts (medium, block-based) | 10 | ~3 KB body |
| Posts (long, block-based) | 5 | ~8 KB body, nested groups, columns, formatting |
| Posts (classic HTML) | 3 | exercises `wpautop` / render-time kses |
| Pages | 2 | about, contact |
| Categories | 3 | announcements, perf, dev |
| Tags | 5 | |
| Comments | 40 | across 8 commented posts (~5 per post) |
| Users | 5 | to make author archives work |
| Nav menu | 1 | 4 items |
| Widgets | 2 | categories + recent posts in sidebar |
| Media | 0 | skipped for v1 |

**Scenarios this unlocks:**

| URL | What it exercises |
|---|---|
| `/` | Homepage / latest-posts loop (10 posts shown) |
| `/a-short-block-post/` | Small single post with blocks — baseline single |
| `/a-long-block-post/` | Large single post — **parse_blocks win zone** |
| `/a-classic-html-post/` | Non-block content — **wpautop, render-time kses** |
| `/category/announcements/` | Archive loop — multiple posts rendered |
| `/tag/perf/` | Tag archive |
| `/?s=lorem` | Search results |
| `/a-commented-post/` | Single with 20+ comments — **wp_kses at render on each comment** |
| `/wp-admin/edit.php` | Admin list table — **esc_attr/esc_html heavy** (requires auth, may defer to v2) |
| `/wp-json/wp/v2/posts?per_page=10` | REST API — different render path |

**Effort:** ~1.5 days. **Risk:** medium (WXR imports have failure modes, idempotency is fiddly).

### Phase 3 — Per-override runtime toggles

**Goal:** ability to A/B individual overrides without rebuilding the binary.

**Deliverables:**
- `crates/patina-ext/src/lib.rs`: change `SHIM_OVERRIDES` handling in `patina_activate()` to consult PHP constants / env vars before swapping each entry. Proposed constants:
  - `PATINA_DISABLE` (existing, all-or-nothing) — keep
  - `PATINA_DISABLE_ESC` — disables `esc_html` + `esc_attr`
  - `PATINA_DISABLE_KSES` — disables `wp_kses`
  - `PATINA_DISABLE_PARSE_BLOCKS` — disables `parse_blocks`
- `php/bridge/patina-bridge.php`: read the constants/env before calling `patina_activate()`, and either pass them as options to activate OR check them and call per-override activation helpers.
- Implementation note: simplest approach is probably a `skip_list: Vec<&str>` parameter to `patina_activate()` that filters `SHIM_OVERRIDES`. Add a helper fn `should_skip(name: &str) -> bool` that reads PHP constants via `ext_php_rs`.
- Integration tests in `php/tests-integration/`: verify each constant actually disables its target, verify `patina_status()` reflects the current state
- `CLAUDE.md` update: document the per-override toggles alongside `PATINA_DISABLE`

**Effort:** ~0.5 day. **Risk:** low, isolated change.

### Phase 4 — Bench runner: automate the whole thing

**Goal:** one command that sets up, seeds, runs all configurations, collects results.

**Deliverables:**

**`scripts/bench-runner.sh`** (shell is fine for v1, Python if it grows):
1. Ensure stack is up, content is seeded (idempotent)
2. For each configuration in a list — `{ name: "stock", env: { PATINA_DISABLE: 1 } }`, `{ name: "parse_blocks_only", env: { PATINA_DISABLE_ESC: 1, PATINA_DISABLE_KSES: 1 } }`, `{ name: "full_patina", env: {} }`, etc. — restart php-fpm with those env vars and run k6 against all scenarios
3. Save JSON output per config, tagged with config name, git HEAD, timestamp
4. Combine into a single summary artifact under `/tmp/patina-bench/<timestamp>/`

**`scripts/bench-compare.py`** (Python chosen for statistics libs):
- Take 2+ JSON runs OR one run with multiple configs
- Compute per-scenario TTFB deltas with confidence intervals (simple two-sample Welch's t-test)
- Output markdown report with tables, highlighting statistically significant changes
- Exit nonzero if p95 regressed by >X% on any scenario (configurable threshold, off by default in v1)

**Makefile targets:**
- `make bench-full` — run the full sequence
- `make bench-compare FROM=<run> TO=<run>` — diff two runs
- `make bench-baseline` — special: run the bench and write to `fixtures/baselines/` for committing

**JSON output schema** (proposed, each run produces one file):

```json
{
  "meta": {
    "timestamp": "2026-04-12T21:00:00Z",
    "git_sha": "abc123",
    "patina_version": "0.1.0",
    "php_version": "8.3.30",
    "wp_version": "6.9.4",
    "host": "profiling-docker",
    "cpu": "AMD Ryzen 9 5950X",
    "iterations_per_scenario": 100
  },
  "config": {
    "name": "full-patina",
    "env": {},
    "active_overrides": ["esc_html", "esc_attr", "wp_kses", "parse_blocks"]
  },
  "scenarios": {
    "homepage": {
      "ttfb_ms": {
        "min": 143.4,
        "mean": 163.3,
        "p50": 162.3,
        "p90": 173.8,
        "p95": 177.2,
        "p99": 185.1,
        "max": 188.6,
        "stddev": 8.5,
        "samples": [177.1, 162.4, 168.5, ...]
      },
      "total_ms": { "min": ..., "mean": ..., ... }
    },
    "single_post_long": { ... },
    ...
  }
}
```

**Why include raw samples:** it's only a few KB per scenario and it enables post-hoc analysis (percentile recomputation, outlier trimming, statistical tests) that pre-aggregated summaries can't support. Cheap insurance.

**Effort:** ~1.5 days. **Risk:** medium. Highest value — this is where everything comes together.

### Phase 5 — SPX integration (optional but high-signal)

**Goal:** when a bench run shows something surprising, capture a flame graph to diagnose.

**Deliverables:**
- k6 script option: tag the Nth iteration of each scenario with SPX cookie (`SPX_ENABLED=1; SPX_KEY=dev`), causing SPX to profile that single request server-side
- Script collects the resulting profile from SPX's data directory inside the container after the run finishes
- Bench report links to profile files: "full-patina single_post_long request — see `/tmp/patina-bench/.../spx/single_post_long.json`"
- Optional: a helper script that opens the SPX web UI pointing at the collected profile

**Effort:** ~0.5 day. **Risk:** low. Marginal v1 utility but unlocks the "why did this happen" debug loop when bench numbers surprise us.

### Phase 6 — Baseline, dogfood, and report

**Goal:** use the new system to generate the first real baseline of where patina actually helps and hurts.

**Deliverables:**
- Run the full `make bench-full` against the current patina HEAD with 5 configurations:
  1. `stock` — no patina (kill switch)
  2. `esc_only` — only esc_html + esc_attr active
  3. `kses_only` — only wp_kses active
  4. `parse_blocks_only` — only parse_blocks active
  5. `full_patina` — all overrides active
- Commit the baseline JSON to `fixtures/baselines/phase6-initial.json` (or similar)
- Write `docs/BENCHMARKS.md`:
  - Methodology
  - Hardware / OS / PHP / WP versions
  - Per-scenario deltas table for each configuration vs stock
  - Analysis: which overrides are net wins, which are losses, on which workloads
  - Action items for optimization (if the bridge overhead is still dominant, prioritize its fix)
- Update `README.md` status table with real end-to-end numbers and per-scenario percentages, replacing the current microbench-derived numbers

**Effort:** ~0.5 day of running + analysis. Assume the first run will surface bugs in Phases 1–5 that need fixing — budget another 0.5 day of wiggle.

## Open questions — decide before starting

### Q1: Theme choice

Proposed: **TwentyTwenty-Five** (FSE, default block theme). ~70% of new WP installs use FSE, and it exercises the block rendering pipeline that `parse_blocks` targets.

Alternative: TwentyTwenty-One (classic PHP templates, non-block theme). More representative if the sites we care about are classic.

**What does `peterbooker.com` currently use?** If classic, we should test classic. If FSE, we should test FSE. If we have time, both — but default to one for v1.

### Q2: Content scale

Proposed: **25 posts / 40 comments / 5 users**, per the table above.

Larger (100 posts / 500 comments) stresses the DB more and makes archive pages a tougher test. Smaller (10 posts / 20 comments) keeps bench runs faster. Trade-off between signal and iteration speed.

### Q3: CI integration

Options:
- **On every PR** — catches regressions early, but costs CI minutes and may be flaky
- **Nightly only** — cheaper, slower feedback
- **On-demand only** — manual `gh workflow run`, simplest to start

Default recommendation: **on-demand only** for v1, revisit once variance characteristics are known.

### Q4: Baseline persistence

Options:
- **Commit JSON baselines to the repo** — simple, versioned, diffable, offline-friendly, revertable
- **External storage** (S3, Grafana) — scales better but adds infrastructure

Strongly prefer **committing to the repo**. The files are small (~10 KB each), they're version history we actually care about, and it means the bench results are Git-blamed next to the code changes that caused them.

### Q5: Consumer model

Is this just for Peter running locally, or should it work for GitHub Actions too?

If local only, we can skip some plumbing (CI-friendly output, exit codes). If CI-compatible, we should design for it from Phase 4.

Default: **design for CI compatibility but only wire up local for v1**. The cost of "make it work in CI later" is small if we keep the runner parameterized from the start.

## Risks and known unknowns

1. **Variance is the limiting factor.** The live-site test against `peterbooker.com` showed 8–20 ms stddev at n=30. For a 10 ms true delta to be statistically significant at p<0.05, we need ~n=100 samples per config per scenario under current noise, or we need to reduce noise. The Docker environment should be more stable (no concurrent traffic, no network variance), but until we measure we don't know. If Docker variance is still >5 ms stddev, we need longer runs, interleaved configs (A/B/A/B), or Welch's t-test with unequal variances.
2. **WP admin URLs require auth.** Benchmarking `/wp-admin/edit.php` means k6 needs a login flow and has to carry the session cookie. Scriptable but adds complexity. **Deferred to v2** unless it turns out to be important for characterization.
3. **Content seeding idempotency.** WXR imports don't dedupe — a second run would duplicate posts. The seeder wipes existing content first to keep this simple. Alternative: track what was imported, skip duplicates. Wipe-first is simpler.
4. **First bench-full run will be flaky.** Cross-phase integration always surfaces timing issues, missing env vars, stale FPM workers, etc. Budget half a day of debugging in Phase 6.
5. **Bridge overhead might still dominate all results.** We already know from the live-site test that patina's per-request overhead eats per-call savings on fast functions. The new bench system needs to be able to measure and decompose this so we can actually work on fixing it — if the bench just says "patina is 5% faster" without telling us *where*, it hasn't done its job.

## Effort summary

| Phase | Effort | Risk | Value |
|---|---|---|---|
| 1. HTTP runner foundation | 0.5 day | Low | Immediate — finally measuring real requests |
| 2. Realistic content corpus | 1.5 days | Medium | High — invalidates all prior micro-bench-only conclusions |
| 3. Per-override toggles | 0.5 day | Low | High — enables A/B of individual overrides |
| 4. Runner + comparison | 1.5 days | Medium | Highest — everything comes together here |
| 5. SPX integration | 0.5 day | Low | Medium — debug tool, nice to have |
| 6. Baseline + report | 0.5 day (+0.5 debug buffer) | Low | Highest — is the whole point |

**Total**: ~5 days of focused work, shippable in 6 small PRs (one per phase).

## Resuming this work

When picking this back up:

1. **Answer the open questions first** (§Q1–Q5). The theme choice and content scale decisions affect everything downstream.
2. **Verify the current state still matches the inventory** — patina's code moves, the profiling stack might have been tweaked. `ls profiling/` and re-read `profiling/setup-wordpress.sh` before relying on this plan.
3. **Start with Phase 1** (HTTP runner). It's the smallest independent piece and gives you something measurable immediately, even against the current single-post seed. If Phase 1 results already show what you need, you might not need Phase 2.
4. **Don't batch**. Each phase is an independent PR. Ship Phase 1, confirm it works, then move on.
5. **Keep the plan in sync**. If you make a design decision that changes something in this document, update the doc as part of the same PR. This is a living plan, not a fossil.

## Relevant files in the current repo

As built, by phase:

- **Phase 1 (HTTP runner)**: `profiling/Dockerfile.profiling` (k6 apt
  install), `profiling/k6-workloads.js` (per-scenario executors,
  cache-bust, TTFB/total trends, warmup discard, `Host` header override),
  `Makefile` target `bench-http`.
- **Phase 2 (content corpus)**: `profiling/benchmark-content/` (three
  block-tier HTMLs, classic-post HTML, `WXR_PIN.env`, README),
  `profiling/seed-benchmark-content.sh` (TT25 + WXR fetch + stable
  slugs), `profiling/setup-wordpress.sh` (calls the seeder).
- **Phase 3 (runtime toggles)**: `crates/patina-ext/src/lib.rs`
  (`patina_activate(skip_list: Option<&Zval>)`),
  `php/bridge/patina-bridge.php` (reads `PATINA_DISABLE_ESC/KSES/PARSE_BLOCKS`
  env + constants), `php/tests-integration/OverrideTogglesTest.php`.
- **Phase 4 (runner + comparator)**: `scripts/bench-runner.sh` (5-config
  matrix, mu-plugin injection, php-fpm restart between configs),
  `scripts/bench-aggregate.py` (k6 NDJSON → summary schema),
  `scripts/bench-compare.py` (intra- or cross-run diff with Welch's
  t-test + markdown report), `Makefile` targets `bench-full`,
  `bench-compare`, `bench-baseline`.
- **Phase 5 (SPX integration)**: k6 cookie injection on one sample per
  scenario when `SPX_KEY` is set, runner tar-out of `/tmp/spx` into
  `<run>/<config>/spx/`, comparator lists profile files in the report,
  `scripts/spx-ui.sh` helper.
- **Phase 6 (baseline)**: `fixtures/baselines/phase6-initial/` (committed
  manifest + per-config summaries + report), `docs/BENCHMARKS.md`
  (methodology + headline numbers + action items), `README.md` status
  table re-labeled as microbench with a pointer to BENCHMARKS.md for
  end-to-end reality.

Supporting files unchanged by this plan but relevant:

- `profiling/conf/spx.ini` — SPX config (HTTP key `dev`)
- `php/benchmarks/bench-kses.php`, `php/benchmarks/run.php` — CLI
  microbenches, kept as a complement (not replaced)
- `crates/patina-ext/src/lib.rs` — activation / swap machinery and
  `SHIM_OVERRIDES` table
- `docs/IMPLEMENTATION_PLAN.md` — historical plan; bench sections
  superseded by this doc

## References

- WordPress theme test data (WXR fixtures): https://github.com/WordPress/theme-test-data
- k6 metrics reference: https://grafana.com/docs/k6/latest/using-k6/metrics/reference/
- SPX profiler: https://github.com/NoiseByNorthwest/php-spx
- `wp post generate` docs: https://developer.wordpress.org/cli/commands/post/generate/
- FakerPress (v0.9.0, REST-API-first): https://github.com/bordoni/fakerpress

## Related project history (for context)

- **April 2026 live-site measurements** on `peterbooker.com` (TTFB, n=30):
  - Stock WordPress: 163.3 ms (round 1), 185.0 ms (round 2, drifted baseline)
  - Patina esc/kses only (no parse_blocks): 197.8 ms → **+34.5 ms regression** vs stock
  - Patina esc/kses + parse_blocks: 175.4 ms → **−9.6 ms improvement** vs the round-2 baseline
  - Decomposition: parse_blocks saves ~44 ms/request; esc/kses bridge overhead costs ~35 ms/request; net ~9 ms improvement when both are active
- **Known bridge-overhead issues** (not solved, relevant to Phase 4 analysis):
  - Rust→PHP `call_user_func` for `apply_filters('esc_html', ...)` is ~10× slower than PHP→PHP dispatch — should move back into the shim layer
  - `has_filter()` checks for `wp_kses_allowed_html` / `kses_allowed_protocols` / `wp_kses_uri_attributes` happen on every `wp_kses` call — should cache per-request in a Rust `OnceCell`
  - `patina_activate()` runs on every request via the mu-plugin — should short-circuit after first call per FPM worker
- **Override architecture** (as of 2026-04-12): all non-pluggable overrides use the PHP user-function shim pattern (`SHIM_OVERRIDES` in `lib.rs`). Pluggable functions (`wp_sanitize_redirect`, `wp_validate_redirect`) stay as direct MINIT-time registration. See `CLAUDE.md` § Function Override Mechanics for the rationale.
