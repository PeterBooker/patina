---
name: benchmark
description: Benchmark a change to prove it is faster. Runs Criterion and PHP benchmarks before and after, compares results, and rejects regressions.
disable-model-invocation: true
argument-hint: "[function_name or 'all']"
---

# Benchmark: $ARGUMENTS

Every performance change must be proven with numbers. "Should be faster" is not evidence.

## 1. Baseline (before your change)

Stash or commit your changes, then run on the CURRENT code:

```bash
# Rust micro-benchmarks (reports MB/s and ns/iter)
cargo bench -p patina-bench -- $ARGUMENTS 2>&1 | tee /tmp/bench-before.txt

# PHP benchmarks (reports Rust vs PHP speedup)
make bench 2>&1 | tee /tmp/php-bench-before.txt
```

## 2. Apply your change and re-benchmark

```bash
cargo bench -p patina-bench -- $ARGUMENTS 2>&1 | tee /tmp/bench-after.txt
make bench 2>&1 | tee /tmp/php-bench-after.txt
```

## 3. Compare

Criterion prints change percentages automatically on the second run. Look for:
- **Green** (`-X.XX%`): faster. Good.
- **Red** (`+X.XX%`): slower. Reject unless there's a correctness reason.
- **`No change`**: within noise. Not worth the complexity.

For PHP benchmarks, compare the speedup columns side by side.

## 4. Rules

- A change that is not measurably faster on realistic inputs is not an improvement — revert it.
- Always benchmark the **fast path** (clean input, no work needed) AND the **slow path** (every byte needs processing). Optimizing one at the expense of the other is a regression.
- Use realistic input sizes: tiny (~20B), medium (~500B), large (~10KB). WordPress content is mostly medium.
- `make bench-jit` shows whether the improvement holds when PHP has JIT enabled — JIT narrows the gap on simple functions.
- If Criterion reports high variance (wide confidence interval), results are unreliable. Close other programs, re-run.
- Never trust a single run. Criterion runs hundreds of iterations automatically — let it.

## 5. What to report

State the function, what changed, and the measured impact:

```
esc_html (medium input, 500B):
  Before: 1.20 µs/iter, 420 MB/s
  After:  0.85 µs/iter, 590 MB/s
  Change: -29% (Criterion: p < 0.05)
```
