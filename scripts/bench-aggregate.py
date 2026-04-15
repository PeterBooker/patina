#!/usr/bin/env python3
"""Aggregate a raw k6 JSON stream into the Patina bench summary schema.

The runner (`scripts/bench-runner.sh`) invokes us once per configuration
with the path to k6's `--out json=` output and the metadata k6 doesn't
have access to (config name, env map, active-override list). We parse
the newline-delimited JSON, bucket per-scenario samples for the two
`patina_*` trends, and emit one JSON file matching the schema pinned in
`docs/BENCHMARK_PLAN.md` § "JSON output schema".

Raw samples are kept alongside the summary statistics — they're only a
handful of KB per scenario and they enable post-hoc analysis (percentile
recomputation, outlier trimming, the Welch's t-test the comparator
needs) that pre-aggregated summaries can't support.
"""
from __future__ import annotations

import argparse
import json
import math
import pathlib
import sys
from typing import Any

# k6 samples we care about. Anything else (http_req_connecting, iterations,
# vus, etc.) is recorded by k6 but dropped here — the bench is about TTFB
# and total response time, not k6's own plumbing metrics.
TRACKED_METRICS = {
    "patina_ttfb_ms": "ttfb_ms",
    "patina_total_ms": "total_ms",
}


def load_samples(
    k6_json_path: pathlib.Path,
    per_scenario: dict[str, dict[str, list[float]]] | None = None,
) -> dict[str, dict[str, list[float]]]:
    """Return {scenario: {metric: [values]}} from a k6 --out json= stream.

    If `per_scenario` is passed in, samples are appended to that dict so
    the caller can fold multiple chunk files into one stream. The runner
    relies on this to keep chunk order intact across the per-config
    k6-chunk-XX.json files."""
    if per_scenario is None:
        per_scenario = {}

    with k6_json_path.open("r", encoding="utf-8") as fh:
        for line_num, raw in enumerate(fh, start=1):
            raw = raw.strip()
            if not raw:
                continue
            try:
                rec = json.loads(raw)
            except json.JSONDecodeError as e:
                print(
                    f"warning: {k6_json_path}:{line_num}: bad JSON: {e}",
                    file=sys.stderr,
                )
                continue

            if rec.get("type") != "Point":
                continue
            metric_name = rec.get("metric")
            if metric_name not in TRACKED_METRICS:
                continue

            data = rec.get("data", {})
            value = data.get("value")
            tags = data.get("tags", {}) or {}
            scenario = tags.get("scenario")
            if scenario is None or value is None:
                continue

            bucket = per_scenario.setdefault(scenario, {})
            bucket.setdefault(TRACKED_METRICS[metric_name], []).append(float(value))

    return per_scenario


def percentile(sorted_values: list[float], pct: float) -> float:
    """Linear-interpolated percentile. Matches numpy's default behavior."""
    if not sorted_values:
        return float("nan")
    if len(sorted_values) == 1:
        return sorted_values[0]
    k = (len(sorted_values) - 1) * (pct / 100.0)
    lo = math.floor(k)
    hi = math.ceil(k)
    if lo == hi:
        return sorted_values[int(k)]
    return sorted_values[lo] * (hi - k) + sorted_values[hi] * (k - lo)


def summarize(values: list[float]) -> dict[str, Any]:
    if not values:
        return {
            "count": 0,
            "min": None,
            "mean": None,
            "p50": None,
            "p90": None,
            "p95": None,
            "p99": None,
            "max": None,
            "stddev": None,
            "samples": [],
        }
    srt = sorted(values)
    n = len(srt)
    mean = sum(srt) / n
    if n > 1:
        var = sum((x - mean) ** 2 for x in srt) / (n - 1)
        stddev = math.sqrt(var)
    else:
        stddev = 0.0
    return {
        "count": n,
        "min": srt[0],
        "mean": mean,
        "p50": percentile(srt, 50),
        "p90": percentile(srt, 90),
        "p95": percentile(srt, 95),
        "p99": percentile(srt, 99),
        "max": srt[-1],
        "stddev": stddev,
        "samples": values,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--k6-json",
        required=True,
        type=pathlib.Path,
        nargs="+",
        help="One or more k6 JSON output files. Pass chunks in order — "
             "aggregator records per-sample chunk index for the paired "
             "analysis in bench-compare.",
    )
    ap.add_argument("--chunks", type=int, default=1)
    ap.add_argument("--output", required=True, type=pathlib.Path)
    ap.add_argument("--config-name", required=True)
    ap.add_argument(
        "--config-env",
        default="{}",
        help="JSON-encoded env map used to spawn php-fpm for this config",
    )
    ap.add_argument(
        "--active-overrides",
        default="",
        help="Comma-separated list of active override names",
    )
    ap.add_argument("--git-sha", default="")
    ap.add_argument("--patina-version", default="")
    ap.add_argument("--php-version", default="")
    ap.add_argument("--wp-version", default="")
    ap.add_argument("--host", default="")
    ap.add_argument("--cpu", default="")
    ap.add_argument("--timestamp", default="")
    ap.add_argument("--iterations", type=int, default=0)
    args = ap.parse_args()

    # Each chunk file contributes samples in scenario-k6-execution order.
    # Track the sample count per (scenario, metric) before and after each
    # file so we know which samples belong to which chunk — the comparator
    # needs this to pair baseline[i] with candidate[i] within the same
    # chunk boundary.
    per_scenario: dict[str, dict[str, list[float]]] = {}
    chunk_ids: dict[str, dict[str, list[int]]] = {}
    for chunk_idx, path in enumerate(args.k6_json):
        if not path.exists():
            print(f"error: k6 JSON not found: {path}", file=sys.stderr)
            return 1
        before: dict[str, dict[str, int]] = {
            scn: {m: len(v) for m, v in metrics.items()}
            for scn, metrics in per_scenario.items()
        }
        load_samples(path, per_scenario)
        for scn, metrics in per_scenario.items():
            for m, values in metrics.items():
                prev = before.get(scn, {}).get(m, 0)
                added = len(values) - prev
                chunk_ids.setdefault(scn, {}).setdefault(m, []).extend(
                    [chunk_idx] * added
                )

    scenarios_out: dict[str, Any] = {}
    for scenario, metrics in sorted(per_scenario.items()):
        ttfb_vals = metrics.get("ttfb_ms", [])
        total_vals = metrics.get("total_ms", [])
        ttfb_summary = summarize(ttfb_vals)
        total_summary = summarize(total_vals)
        ttfb_summary["chunk_ids"] = chunk_ids.get(scenario, {}).get("ttfb_ms", [])
        total_summary["chunk_ids"] = chunk_ids.get(scenario, {}).get("total_ms", [])
        scenarios_out[scenario] = {
            "ttfb_ms": ttfb_summary,
            "total_ms": total_summary,
        }

    try:
        env_map = json.loads(args.config_env) if args.config_env else {}
    except json.JSONDecodeError:
        env_map = {}

    overrides = [
        s.strip() for s in args.active_overrides.split(",") if s.strip()
    ]

    summary = {
        "meta": {
            "timestamp": args.timestamp,
            "git_sha": args.git_sha,
            "patina_version": args.patina_version,
            "php_version": args.php_version,
            "wp_version": args.wp_version,
            "host": args.host,
            "cpu": args.cpu,
            "iterations_per_scenario": args.iterations,
            "chunks": args.chunks,
        },
        "config": {
            "name": args.config_name,
            "env": env_map,
            "active_overrides": overrides,
        },
        "scenarios": scenarios_out,
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8") as fh:
        json.dump(summary, fh, indent=2, sort_keys=False)
        fh.write("\n")

    # Brief stdout recap so the bench runner's log shows per-config deltas
    # without the user having to open the JSON.
    print(f"  config={args.config_name}")
    for name, block in scenarios_out.items():
        ttfb = block["ttfb_ms"]
        if ttfb["count"] == 0:
            print(f"    {name:20s} (no samples)")
            continue
        print(
            f"    {name:20s} n={ttfb['count']:3d}  "
            f"p50={ttfb['p50']:7.2f}ms  "
            f"p95={ttfb['p95']:7.2f}ms  "
            f"avg={ttfb['mean']:7.2f}ms"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
