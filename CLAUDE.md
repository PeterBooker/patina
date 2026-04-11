# Patina

Rust PHP extension replacing WordPress core functions with native implementations. Built with ext-php-rs 0.15.

## Architecture

- `patina-core` — Pure Rust logic. No PHP types. All algorithms live here.
- `patina-ext` — PHP cdylib wrapper. Thin `#[php_function]` + `catch_unwind`.
- `patina-bench` — Criterion benchmarks. `fuzz/` — cargo-fuzz targets.
- `php/` — PHPUnit tests, benchmarks, fixture generator, bridge mu-plugin.
- `profiling/` — Docker WordPress stack with SPX + k6.

## Commands

All via Docker — no local Rust or PHP needed.

```
make test        # All Rust + PHP tests
make check       # test + clippy + fmt + PHPUnit
make build       # Release .so
make bench       # PHP benchmarks (Rust vs PHP)
make bench-jit   # With JIT enabled
make verify      # Load extension, print functions
make shell       # Dev container shell
```

Target PHP version: `PHP_VERSION=8.1 make build`

## Key Constraints

- `panic = "unwind"` (NOT abort) — required for `catch_unwind`
- All `#[php_function]` in `patina-ext/src/lib.rs` with explicit `wrap_function!()` registration
- Non-pluggable functions need a `_filtered` variant that calls `apply_filters()`
- Function table swap via `patina_activate()` only affects code compiled AFTER activation
- Match WordPress output byte-for-byte — validate with fixtures generated from WP
