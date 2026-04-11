import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Counter } from 'k6/metrics';

const pageLoad = new Trend('page_load_ms');
const errors = new Counter('errors');

// Workload URLs — the benchmark post is created by setup-wordpress.sh
const PAGES = [
    { name: 'homepage', path: '/' },
    { name: 'single_post', path: '/patina-benchmark/' },
    { name: 'archive', path: '/category/uncategorized/' },
    { name: 'search', path: '/?s=lorem' },
    { name: 'rest_api', path: '/wp-json/wp/v2/posts?per_page=5' },
];

export const options = {
    scenarios: {
        // Profiling mode: low concurrency, capture per-request details
        profiling: {
            executor: 'per-vu-iterations',
            vus: 1,
            iterations: PAGES.length * 3, // 3 passes over each page
            maxDuration: '2m',
            tags: { scenario: 'profiling' },
        },
        // Load test mode: sustained traffic for aggregate metrics
        load_test: {
            executor: 'constant-vus',
            vus: 10,
            duration: '60s',
            startTime: '30s', // Start after profiling finishes
            tags: { scenario: 'load_test' },
        },
    },
    thresholds: {
        'http_req_duration': ['p(95)<5000'], // Warn if p95 > 5s
    },
};

export default function () {
    const page = PAGES[Math.floor(Math.random() * PAGES.length)];
    const url = `http://nginx${page.path}`;

    const res = http.get(url, {
        tags: { page: page.name },
        timeout: '10s',
    });

    const ok = check(res, {
        'status is 200': (r) => r.status === 200,
        'body not empty': (r) => r.body && r.body.length > 0,
    });

    if (!ok) {
        errors.add(1);
    }

    pageLoad.add(res.timings.duration, { page: page.name });
    sleep(0.1);
}
