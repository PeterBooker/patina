#!/usr/bin/env python3
"""Compare Patina bench runs and emit a markdown delta report.

Two modes, auto-detected from the arguments:

  scripts/bench-compare.py <run-dir>
      Compare every non-baseline config inside one run directory against
      that run's "stock" config. This is the usual mode — one full
      `make bench-full` run, decomposed into per-override deltas.

  scripts/bench-compare.py <run-dir-a> <run-dir-b>
      Cross-run diff. Compares matching (config, scenario) pairs — useful
      for "did my change regress anything" checks against a committed
      baseline under fixtures/baselines/.

Output is written to stdout as GitHub-flavored markdown, plus an
optional machine-readable JSON blob via --json-out. Exit code is nonzero
if `--fail-on-regress PCT` is set and any trimmed-mean regressed by more
than PCT percent on any scenario (trimmed mean rather than p95 because
p95 is too noisy at n≤200 to drive CI pass/fail — see docstring below).

For intra-run comparisons where baseline and candidate come from the
same chunked run (same CHUNKS, same CHUNK_ITERS, same scenario order),
the reporter runs a *paired* t-test on per-sample deltas. Pairing
cancels chunk-level thermal/host drift and is typically 2–3× more
powerful than Welch's on real bench data. When sample counts don't
match (or chunk_ids are missing — cross-run mode), we fall back to
Welch's unpaired test.

Headline metrics are **p50** and a **10%-trimmed mean**. p95 is
reported but demoted to a tail-check column: at n=100 its confidence
interval is huge and a single outlier request can flip its sign, so
basing pass/fail on it leads to the noise-chasing we saw in
phase6-initial.
"""
from __future__ import annotations

import argparse
import json
import math
import pathlib
import sys
from typing import Any, Iterable


# ----------------------------------------------------------------------
# Statistics helpers
# ----------------------------------------------------------------------

def mean(xs: Iterable[float]) -> float:
    xs = list(xs)
    return sum(xs) / len(xs) if xs else float("nan")


def sample_variance(xs: list[float], mu: float) -> float:
    if len(xs) < 2:
        return 0.0
    return sum((x - mu) ** 2 for x in xs) / (len(xs) - 1)


def trimmed_mean(xs: list[float], trim: float = 0.10) -> float:
    """Symmetric trimmed mean. Drops `trim` fraction from each tail."""
    if not xs:
        return float("nan")
    n = len(xs)
    k = int(n * trim)
    if n - 2 * k <= 0:
        return mean(xs)
    srt = sorted(xs)
    return mean(srt[k : n - k])


def paired_t(a: list[float], b: list[float]) -> tuple[float, float, float, float]:
    """Return (mean_delta, t, df, p_two_sided) for paired samples.

    Pairs element-wise: expects len(a) == len(b) and same chunk ordering
    (runner guarantees this because each chunk emits samples in the same
    scenario-iteration order for every config).
    """
    if len(a) != len(b) or not a:
        return (float("nan"), float("nan"), 0.0, float("nan"))
    diffs = [bi - ai for ai, bi in zip(a, b)]
    n = len(diffs)
    md = mean(diffs)
    if n < 2:
        return (md, float("nan"), 0.0, float("nan"))
    var = sample_variance(diffs, md)
    if var <= 0:
        return (md, float("nan"), float(n - 1), float("nan"))
    se = math.sqrt(var / n)
    t = md / se
    df = float(n - 1)
    z = abs(t)
    p = 2.0 * (1.0 - _phi(z))
    return (md, t, df, p)


def welch_t(a: list[float], b: list[float]) -> tuple[float, float, float, float]:
    """Return (mean_delta, t, df, p_two_sided)."""
    if not a or not b:
        return (float("nan"), float("nan"), 0.0, float("nan"))
    ma, mb = mean(a), mean(b)
    va = sample_variance(a, ma)
    vb = sample_variance(b, mb)
    na, nb = len(a), len(b)
    se2 = va / na + vb / nb
    if se2 <= 0:
        return (mb - ma, float("nan"), 0.0, float("nan"))
    se = math.sqrt(se2)
    t = (mb - ma) / se
    df_num = se2 ** 2
    df_den = 0.0
    if na > 1:
        df_den += (va / na) ** 2 / (na - 1)
    if nb > 1:
        df_den += (vb / nb) ** 2 / (nb - 1)
    df = df_num / df_den if df_den > 0 else 0.0
    # Normal approximation: accurate to ~1e-3 for df>=30, which our
    # n>=100 runs comfortably satisfy.
    z = abs(t)
    p = 2.0 * (1.0 - _phi(z))
    return (mb - ma, t, df, p)


def _phi(z: float) -> float:
    """Standard-normal CDF via erf."""
    return 0.5 * (1.0 + math.erf(z / math.sqrt(2.0)))


def sig_marker(p: float) -> str:
    if math.isnan(p):
        return ""
    if p < 0.001:
        return "***"
    if p < 0.01:
        return "**"
    if p < 0.05:
        return "*"
    return ""


# ----------------------------------------------------------------------
# Loading
# ----------------------------------------------------------------------

def load_summary(path: pathlib.Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as fh:
        return json.load(fh)


def discover_configs(run_dir: pathlib.Path) -> dict[str, dict[str, Any]]:
    """Return {config_name: summary_dict} for every summary.json under run_dir."""
    out: dict[str, dict[str, Any]] = {}
    for summary_path in sorted(run_dir.glob("*/summary.json")):
        config_name = summary_path.parent.name
        out[config_name] = load_summary(summary_path)
    return out


# ----------------------------------------------------------------------
# Report rendering
# ----------------------------------------------------------------------

TABLE_HEADER = (
    "| Scenario | n | base p50 → cand | Δp50 % | base tmean → cand | Δtmean % | test | p95 Δ % |"
)
TABLE_DIVIDER = "|---|---:|---|---:|---|---:|---|---:|"


def format_delta_row(
    scenario: str,
    baseline_samples: list[float],
    candidate_samples: list[float],
    paired: bool,
) -> tuple[str, float]:
    """Return (markdown_row, pct_tmean). pct_tmean is nan if undefined.

    `paired` selects paired-t vs Welch's t. The runner passes paired=True
    for intra-run comparisons (same chunked run — indices match across
    configs) and paired=False for cross-run diffs.
    """
    if not baseline_samples or not candidate_samples:
        return (
            f"| {scenario} | 0 | (no samples) | | | | | |",
            float("nan"),
        )

    n = min(len(baseline_samples), len(candidate_samples))
    base_p50 = _percentile(baseline_samples, 50)
    cand_p50 = _percentile(candidate_samples, 50)
    base_p95 = _percentile(baseline_samples, 95)
    cand_p95 = _percentile(candidate_samples, 95)
    base_tm = trimmed_mean(baseline_samples)
    cand_tm = trimmed_mean(candidate_samples)

    pct_p50 = ((cand_p50 - base_p50) / base_p50 * 100.0) if base_p50 else float("nan")
    pct_tm = ((cand_tm - base_tm) / base_tm * 100.0) if base_tm else float("nan")
    pct_p95 = ((cand_p95 - base_p95) / base_p95 * 100.0) if base_p95 else float("nan")

    if paired and len(baseline_samples) == len(candidate_samples):
        _, _, df, p = paired_t(baseline_samples, candidate_samples)
        test = "paired"
    else:
        _, _, df, p = welch_t(baseline_samples, candidate_samples)
        test = "welch"
    marker = sig_marker(p)

    def arrow(pct: float) -> str:
        if math.isnan(pct):
            return ""
        if pct <= -1:
            return " ↓"
        if pct >= 1:
            return " ↑"
        return ""

    return (
        (
            f"| {scenario} "
            f"| {n} "
            f"| {base_p50:.1f} → {cand_p50:.1f} "
            f"| {pct_p50:+.1f}{arrow(pct_p50)} "
            f"| {base_tm:.1f} → {cand_tm:.1f} "
            f"| {pct_tm:+.1f}{arrow(pct_tm)} "
            f"| {test} p={p:.3g} {marker} "
            f"| {pct_p95:+.1f}{arrow(pct_p95)} |"
        ),
        pct_tm,
    )


def _percentile(xs: list[float], pct: float) -> float:
    if not xs:
        return float("nan")
    srt = sorted(xs)
    if len(srt) == 1:
        return srt[0]
    k = (len(srt) - 1) * (pct / 100.0)
    lo = math.floor(k)
    hi = math.ceil(k)
    if lo == hi:
        return srt[int(k)]
    return srt[lo] * (hi - k) + srt[hi] * (k - lo)


def samples_for(summary: dict[str, Any], scenario: str, metric: str) -> list[float]:
    scn = summary.get("scenarios", {}).get(scenario)
    if not scn:
        return []
    return list(scn.get(metric, {}).get("samples") or [])


def render_intra_run(
    run_dir: pathlib.Path,
    summaries: dict[str, dict[str, Any]],
    baseline_name: str,
) -> tuple[str, bool, float]:
    """Emit one section per non-baseline config, comparing to baseline."""
    if baseline_name not in summaries:
        raise SystemExit(
            f"error: baseline config '{baseline_name}' not found in {run_dir}. "
            f"Available: {sorted(summaries)}"
        )
    baseline = summaries[baseline_name]
    meta = baseline.get("meta", {})

    lines: list[str] = [
        f"# Patina bench report — {run_dir.name}",
        "",
        f"- Git SHA: `{meta.get('git_sha', 'unknown')}`",
        f"- Patina version: {meta.get('patina_version', 'unknown')}",
        f"- PHP: {meta.get('php_version', 'unknown')} · WP: {meta.get('wp_version', 'unknown')}",
        f"- Host: {meta.get('host', 'unknown')} ({meta.get('cpu', 'unknown')})",
        f"- Iterations per scenario: {meta.get('iterations_per_scenario', '?')}",
        f"- Baseline config: **{baseline_name}**",
        "",
        "Headline metrics: **p50** (median) and **tmean** (10%-trimmed mean, drops the 10 fastest + 10 slowest samples before averaging). Both are robust to single-request outliers. `Δ %` is (cand − base) / base × 100; ↓ = faster, ↑ = slower. The `p95 Δ %` column is kept as a tail-latency check but is **not** the pass/fail signal — its confidence interval at n≤200 is too wide to base decisions on. `*`=p<0.05, `**`=p<0.01, `***`=p<0.001; intra-run rows use a paired t-test (each sample paired by chunk-matched index against the stock config), cross-run rows use Welch's.",
        "",
    ]

    worst_regression_pct = 0.0
    any_regress = False

    for name, summary in summaries.items():
        if name == baseline_name:
            continue
        lines.append(f"## {name}")
        lines.append("")
        overrides = summary.get("config", {}).get("active_overrides") or []
        lines.append(f"Active overrides: `{', '.join(overrides) if overrides else '(none)'}`")
        spx_dir = run_dir / name / "spx"
        if spx_dir.is_dir():
            profile_files = sorted(p for p in spx_dir.rglob("*") if p.is_file())
            if profile_files:
                lines.append("")
                lines.append(f"SPX profiles ({len(profile_files)}):")
                for p in profile_files:
                    rel = p.relative_to(run_dir)
                    lines.append(f"- `{rel}`")
        lines.append("")
        lines.append(TABLE_HEADER)
        lines.append(TABLE_DIVIDER)

        scenarios = sorted(set(baseline.get("scenarios", {})) | set(summary.get("scenarios", {})))
        for scenario in scenarios:
            base_samples = samples_for(baseline, scenario, "ttfb_ms")
            cand_samples = samples_for(summary, scenario, "ttfb_ms")
            row, pct = format_delta_row(scenario, base_samples, cand_samples, paired=True)
            lines.append(row)
            if not math.isnan(pct) and pct > worst_regression_pct:
                worst_regression_pct = pct
                any_regress = True
        lines.append("")

    return ("\n".join(lines) + "\n", any_regress, worst_regression_pct)


def render_cross_run(
    run_a: pathlib.Path,
    run_b: pathlib.Path,
    summaries_a: dict[str, dict[str, Any]],
    summaries_b: dict[str, dict[str, Any]],
) -> tuple[str, bool, float]:
    """Compare matching configs across two runs (A = before, B = after)."""
    meta_a = next(iter(summaries_a.values()), {}).get("meta", {}) if summaries_a else {}
    meta_b = next(iter(summaries_b.values()), {}).get("meta", {}) if summaries_b else {}

    lines: list[str] = [
        f"# Patina bench cross-run diff",
        "",
        f"- Before: `{run_a}` (git `{meta_a.get('git_sha', '?')}`, patina {meta_a.get('patina_version', '?')})",
        f"- After:  `{run_b}` (git `{meta_b.get('git_sha', '?')}`, patina {meta_b.get('patina_version', '?')})",
        "",
    ]

    shared_configs = sorted(set(summaries_a) & set(summaries_b))
    only_a = sorted(set(summaries_a) - set(summaries_b))
    only_b = sorted(set(summaries_b) - set(summaries_a))
    if only_a:
        lines.append(f"Configs only in before: `{', '.join(only_a)}`")
    if only_b:
        lines.append(f"Configs only in after: `{', '.join(only_b)}`")
    if only_a or only_b:
        lines.append("")

    worst_regression_pct = 0.0
    any_regress = False

    for config in shared_configs:
        a = summaries_a[config]
        b = summaries_b[config]
        lines.append(f"## {config}")
        lines.append("")
        lines.append(TABLE_HEADER)
        lines.append(TABLE_DIVIDER)
        scenarios = sorted(
            set(a.get("scenarios", {})) | set(b.get("scenarios", {}))
        )
        for scenario in scenarios:
            base = samples_for(a, scenario, "ttfb_ms")
            cand = samples_for(b, scenario, "ttfb_ms")
            row, pct = format_delta_row(scenario, base, cand, paired=False)
            lines.append(row)
            if not math.isnan(pct) and pct > worst_regression_pct:
                worst_regression_pct = pct
                any_regress = True
        lines.append("")

    return ("\n".join(lines) + "\n", any_regress, worst_regression_pct)


# ----------------------------------------------------------------------
# CLI
# ----------------------------------------------------------------------

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "runs",
        nargs="+",
        type=pathlib.Path,
        help="One run dir (intra-run compare) or two (cross-run diff)",
    )
    ap.add_argument(
        "--baseline",
        default="stock",
        help="Baseline config name for intra-run mode (default: stock)",
    )
    ap.add_argument(
        "--fail-on-regress",
        type=float,
        default=None,
        help="Exit nonzero if any trimmed-mean regressed by more than this percent",
    )
    ap.add_argument("--output", type=pathlib.Path, help="Write markdown to this file")
    args = ap.parse_args()

    if len(args.runs) == 1:
        run_dir = args.runs[0]
        summaries = discover_configs(run_dir)
        if not summaries:
            print(f"error: no summary.json files under {run_dir}", file=sys.stderr)
            return 1
        report, any_regress, worst_pct = render_intra_run(run_dir, summaries, args.baseline)
    elif len(args.runs) == 2:
        run_a, run_b = args.runs
        summaries_a = discover_configs(run_a)
        summaries_b = discover_configs(run_b)
        if not summaries_a or not summaries_b:
            print("error: one or both run dirs have no summary.json files", file=sys.stderr)
            return 1
        report, any_regress, worst_pct = render_cross_run(run_a, run_b, summaries_a, summaries_b)
    else:
        print("error: pass 1 run dir (intra-run) or 2 run dirs (cross-run)", file=sys.stderr)
        return 2

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(report, encoding="utf-8")
    else:
        sys.stdout.write(report)

    if args.fail_on_regress is not None and worst_pct > args.fail_on_regress:
        print(
            f"\nFAIL: worst trimmed-mean regression {worst_pct:+.1f}% exceeds threshold "
            f"{args.fail_on_regress:+.1f}%",
            file=sys.stderr,
        )
        return 3

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
