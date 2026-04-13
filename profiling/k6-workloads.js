// Patina HTTP benchmark workloads.
//
// Phase 1 of docs/BENCHMARK_PLAN.md — per-scenario TTFB and total-duration
// measurement against the profiling stack's nginx. One `per-vu-iterations`
// executor per scenario so samples are sequential (no contention) and
// directly comparable across configurations.
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

// One k6 scenario per URL. `per-vu-iterations` with vus=1 runs samples
// strictly sequentially so measurements aren't confounded by concurrency.
// Scenarios fire back-to-back in the order declared — k6 interleaves them
// if `startTime` is unset, but per-scenario iteration counts keep the total
// sample count deterministic.
function buildScenarios() {
    const out = {};
    for (const s of SCENARIOS) {
        out[s.name] = {
            executor: 'per-vu-iterations',
            vus: 1,
            iterations: ITERATIONS + WARMUP,
            maxDuration: '10m',
            exec: 'runScenario',
            env: { SCENARIO_NAME: s.name, SCENARIO_PATH: s.path },
            tags: { scenario: s.name },
        };
    }
    return out;
}

export const options = {
    scenarios: buildScenarios(),
    // No global thresholds — the bench-compare tool in Phase 4 owns
    // pass/fail decisions against a persisted baseline.
    discardResponseBodies: false,
    summaryTrendStats: ['min', 'avg', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

export function runScenario() {
    const name = __ENV.SCENARIO_NAME;
    const path = __ENV.SCENARIO_PATH;
    const sep = path.includes('?') ? '&' : '?';
    const url = `${BASE_URL}${path}${sep}t=${__ITER}_${__VU}_${Date.now()}`;

    const params = {
        tags: { scenario: name },
        timeout: '30s',
        headers: { Host: HOST_HEADER },
    };
    // SPX profile trigger — single iteration per scenario. SPX tags the
    // profile with the request URL, and we included the scenario name in
    // the SPX_UI_REPORT_KEY cookie so the runner's post-hoc copy step can
    // match profiles back to their originating scenario.
    if (SPX_KEY && __ITER === SPX_PROFILE_ITER) {
        params.cookies = {
            SPX_ENABLED: '1',
            SPX_KEY: SPX_KEY,
            SPX_UI_URI: '/',
            SPX_REPORT_KEY: `patina_bench_${name}`,
        };
    }
    const res = http.get(url, params);

    const ok = check(res, {
        'status is 2xx': (r) => r.status >= 200 && r.status < 300,
        'body not empty': (r) => r.body && r.body.length > 0,
    });

    if (!ok) {
        errors.add(1, { scenario: name });
        // Keep going — one 500 shouldn't abort the run, but we log it.
        console.warn(`${name} failed: status=${res.status} len=${res.body ? res.body.length : 0}`);
    }

    // Drop warmup iterations from the metrics so opcache / WP object cache
    // warmup doesn't skew the baseline.
    if (__ITER >= WARMUP) {
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
