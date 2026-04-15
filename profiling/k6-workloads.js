// Patina HTTP benchmark workloads.
//
// Phase 1 of docs/BENCHMARK_PLAN.md — per-scenario TTFB and total-duration
// measurement against the profiling stack's nginx. A single `per-vu-iterations`
// executor with vus=1 cycles through the scenario list in order, which
// guarantees strictly sequential requests (no queueing against
// pm.max_children). Previously each scenario had its own executor with no
// startTime, which k6 interprets as "start simultaneously" — with 9
// scenarios and 5 FPM workers that caused 4 requests per round to queue,
// adding tens of ms of queue-wait variance that swamped any patina signal.
//
// Run it via `make bench-http`, which wires in --out json= and the right
// BASE_URL / ITERATIONS env vars. Direct invocation also works:
//
//   k6 run --env BASE_URL=http://nginx --env ITERATIONS=100 \
//     --out json=/tmp/k6-output.json profiling/k6-workloads.js
//
// Each request carries:
//   - A cache-bust query arg so nginx fastcgi_cache / WP object cache don't
//     return a cached body between samples.
//   - A `scenario` tag so downstream analysis (Phase 4 bench-compare) can
//     group samples by scenario cleanly.
//
// TTFB (`http_req_waiting`) is the primary metric — it isolates server-side
// work from network/body-transfer time. `http_req_duration` is kept as the
// secondary metric for total wall-clock comparisons.

import http from 'k6/http';
import { check, fail } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const BASE_URL = __ENV.BASE_URL || 'http://nginx';
// WordPress was installed with --url=http://localhost:8080, so it
// canonicalizes requests to that host. Inside the compose network we
// reach nginx via the service name but still need WP to think the
// request is for the canonical host, otherwise it 301-redirects.
const HOST_HEADER = __ENV.HOST_HEADER || 'localhost:8080';
const ITERATIONS = parseInt(__ENV.ITERATIONS || '100', 10);
const WARMUP = parseInt(__ENV.WARMUP || '5', 10);

// SPX profile trigger (Phase 5 of docs/BENCHMARK_PLAN.md). When SPX_KEY is
// set in the environment, the Nth post-warmup iteration of each scenario
// is sent with the SPX HTTP cookies, which causes php-spx to profile that
// single request server-side and drop a profile file into /tmp/spx inside
// the container. SPX_PROFILE_ITER selects which iteration — defaults to the
// first post-warmup sample so the profile corresponds to a "cold" request.
// Leaving SPX_KEY empty disables profiling entirely (the default for
// regular bench runs — SPX adds ~10% overhead even on requests it doesn't
// fully instrument).
const SPX_KEY = __ENV.SPX_KEY || '';
const SPX_PROFILE_ITER = parseInt(__ENV.SPX_PROFILE_ITER || String(WARMUP), 10);

// Scenarios — (name, path) pairs. The path is relative; cache-bust is appended
// at request time. Phase 2 will expand this list once the realistic content
// corpus lands; for Phase 1 we work against whatever setup-wordpress.sh
// currently seeds (one post + homepage + REST).
// Scenario URLs match the stable slugs created by seed-benchmark-content.sh.
// If you rename these, update the seeder in lockstep — the bench runner has
// no way to know a slug moved.
const SCENARIOS = [
    { name: 'homepage', path: '/' },
    { name: 'single_short', path: '/a-short-block-post/' },
    { name: 'single_long', path: '/a-long-block-post/' },
    { name: 'single_classic', path: '/a-classic-html-post/' },
    { name: 'single_commented', path: '/a-commented-post/' },
    { name: 'archive_category', path: '/category/announcements/' },
    { name: 'archive_tag', path: '/tag/perf/' },
    { name: 'search', path: '/?s=lorem' },
    { name: 'rest_posts', path: '/wp-json/wp/v2/posts?per_page=10' },
];

const ttfb = new Trend('patina_ttfb_ms', true);
const total = new Trend('patina_total_ms', true);
const errors = new Counter('patina_errors');

// Single scenario, single VU. __ITER cycles through SCENARIOS in order
// so requests are strictly sequential and every scenario shares the same
// minute-scale time slice within a chunk. Total iterations = (WARMUP +
// ITERATIONS) × SCENARIOS.length: the outer round-robin means one "cycle"
// hits every URL once, and we emit the first WARMUP cycles to the
// drop-bucket. Metrics are still tagged per-scenario so bench-aggregate
// buckets them the same way as before.
const TOTAL_ITERS = (ITERATIONS + WARMUP) * SCENARIOS.length;

export const options = {
    scenarios: {
        sequential: {
            executor: 'per-vu-iterations',
            vus: 1,
            iterations: TOTAL_ITERS,
            maxDuration: '30m',
            exec: 'runAll',
        },
    },
    // No global thresholds — the bench-compare tool in Phase 4 owns
    // pass/fail decisions against a persisted baseline.
    discardResponseBodies: false,
    summaryTrendStats: ['min', 'avg', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

export function runAll() {
    // Round-robin: cycle[c] = __ITER // N, scenarioIdx = __ITER % N.
    // cycle 0..WARMUP-1 are dropped; cycles WARMUP..(WARMUP+ITERATIONS-1)
    // are the measured samples.
    const scnIdx = __ITER % SCENARIOS.length;
    const cycle = Math.floor(__ITER / SCENARIOS.length);
    const scn = SCENARIOS[scnIdx];
    const name = scn.name;
    const path = scn.path;
    const sep = path.includes('?') ? '&' : '?';
    const url = `${BASE_URL}${path}${sep}t=${__ITER}_${__VU}_${Date.now()}`;

    const params = {
        tags: { scenario: name },
        timeout: '30s',
        headers: { Host: HOST_HEADER },
    };
    // SPX profile trigger — one profiled cycle only, picked to be the
    // first post-warmup cycle (so the profile captures a steady-state
    // request, not a cold-opcache outlier).
    if (SPX_KEY && cycle === SPX_PROFILE_ITER) {
        params.headers = {
            ...params.headers,
            'SPX-Enabled': '1',
            'SPX-Key': SPX_KEY,
            'SPX-Report-Key': `patina_bench_${name}`,
        };
    }
    const res = http.get(url, params);

    const ok = check(res, {
        'status is 2xx': (r) => r.status >= 200 && r.status < 300,
        'body not empty': (r) => r.body && r.body.length > 0,
    });

    if (!ok) {
        errors.add(1, { scenario: name });
        console.warn(`${name} failed: status=${res.status} len=${res.body ? res.body.length : 0}`);
    }

    // Drop the first WARMUP cycles — every scenario's first N samples are
    // cold-opcache / cold-realpath-cache outliers we don't want in the
    // summary statistics.
    if (cycle >= WARMUP) {
        ttfb.add(res.timings.waiting, { scenario: name });
        total.add(res.timings.duration, { scenario: name });
    }
}

export function handleSummary(data) {
    // Let k6 still print its stdout summary, and also emit a compact
    // machine-readable blob to stderr that bench-runner.sh can pick up
    // if it wants a quick sanity check without parsing the full JSON.
    return {
        stdout: defaultTextSummary(data),
    };
}

function defaultTextSummary(data) {
    const lines = ['', 'Patina HTTP bench — per-scenario TTFB', ''];
    const trends = data.metrics.patina_ttfb_ms;
    if (!trends || !trends.values) {
        lines.push('  (no samples recorded — seed + reachability?)');
        return lines.join('\n') + '\n';
    }
    lines.push(`  overall p50=${trends.values.med.toFixed(1)}ms  p95=${trends.values['p(95)'].toFixed(1)}ms  avg=${trends.values.avg.toFixed(1)}ms`);
    lines.push('');
    lines.push('  (per-scenario breakdown is in the JSON output — see --out json=)');
    lines.push('');
    return lines.join('\n');
}
