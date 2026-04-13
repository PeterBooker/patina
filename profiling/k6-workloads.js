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
const ITERATIONS = parseInt(__ENV.ITERATIONS || '100', 10);
const WARMUP = parseInt(__ENV.WARMUP || '5', 10);

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

    const res = http.get(url, {
        tags: { scenario: name },
        timeout: '30s',
    });

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
