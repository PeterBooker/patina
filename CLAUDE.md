# Patina

Rust PHP extension replacing WordPress core functions with native implementations. Built with ext-php-rs 0.15.

## Architecture

- `patina-core` — Pure Rust logic. No PHP types. All algorithms live here.
- `patina-ext` — PHP cdylib wrapper. Thin `#[php_function]` + `catch_unwind`.
- `patina-bench` — Criterion benchmarks. `fuzz/` — cargo-fuzz targets.
- `php/tests/` — unit PHPUnit tests (no WordPress loaded). Run via the dev container.
- `php/tests-integration/` — WordPress-backed integration tests. Run inside the profiling stack; bootstrap requires `wp-load.php` so real `add_filter()`/`apply_filters()` calls work. Use these to verify filter/hook compatibility for overridden functions.
- `php/bridge/patina-bridge.php` — mu-plugin that calls `patina_activate()` at WP boot.
- `profiling/` — Docker WordPress stack with SPX + k6.

## Commands

All via Docker — no local Rust or PHP needed.

```
make test              # All Rust + PHP unit tests
make test-integration  # Integration tests against a real WordPress (profiling stack)
make check             # test + clippy + fmt + PHPUnit
make build             # Release .so
make bench             # PHP benchmarks (Rust vs PHP)
make bench-wp          # WP-backed benchmarks (kses + friends)
make bench-jit         # With JIT enabled
make verify            # Load extension, print functions
make shell             # Dev container shell
```

Target PHP version: `PHP_VERSION=8.1 make build`

`test-integration` spins up (or reuses) the profiling stack, drops the built `.so` into php-fpm, copies the bridge mu-plugin into `wp-content/mu-plugins/`, and runs `php/tests-integration/phpunit.xml`. Use it whenever you add or modify an override that interacts with WordPress filters/hooks — unit tests can't exercise those code paths because the extension test container doesn't load WordPress.

## Key Constraints

- `panic = "unwind"` (NOT abort) — required for `catch_unwind`
- All `#[php_function]` in `patina-ext/src/lib.rs` with explicit `wrap_function!()` registration
- Non-pluggable functions need a `_filtered` variant that calls `apply_filters()`
- Match WordPress output byte-for-byte — validate with fixtures generated from WP

## Function Override Mechanics

Replacing WordPress core functions in the Zend function table is subtle. **Read this before adding a new override** — the naive approach crashes.

### Two override paths

Pick based on the WordPress function's PHP signature:

**1. Direct swap** (`OVERRIDES` table in `patina-ext/src/lib.rs`) — for functions whose signature can be matched byte-for-byte by a Rust `&str` parameter list. `patina_activate()` looks up the target by name in the function table and overwrites its `value.ptr` with the Rust function's `zend_function*`.

- Required: the Rust `#[php_function]`'s arg count and types must match the WordPress function exactly. Examples: `esc_html(string $text)`, `esc_attr(string $text)`, `wp_sanitize_redirect(string $location)` — all 1-arg string-in/string-out.

**2. Shim override** (`SHIM_OVERRIDES` table + embedded PHP source like `KSES_SHIM_PHP`) — for functions with multi-arg, optional, or mixed-type signatures (e.g. `wp_kses($content, $allowed_html, $allowed_protocols = array())`). `patina_activate()` uses `ext_php_rs::php_eval::execute` to define a PHP **user-function** shim whose signature mirrors the WordPress function byte-for-byte. The shim's body trampolines to a Rust internal function (e.g. `patina_wp_kses_internal`). Then the function table slot is swapped to point at the shim.

### Why two paths — compile-time opcode specialization

PHP 8.3's Zend compiler specializes `INIT_FCALL` / `DO_UCALL` opcodes based on the target function's type **at the moment the caller is compiled**. When `wp-includes/kses.php` parses `wp_kses_post`'s body, `wp_kses` is a user function, so the opcodes bake in user-function call-frame semantics. Swapping `wp_kses` to an ext-php-rs internal replacement makes those pre-compiled opcodes crash in `execute_ex` **before** the Rust handler is entered.

The shim approach dodges this: pre-compiled callers dispatch user→user (safe, the shim is a real user function); the shim itself dispatches user→internal (safe, because the shim's opcodes were compiled *after* the Rust function was registered, so its `INIT_FCALL` uses internal-dispatch semantics). Overhead is one extra PHP frame per call — ~1µs, invisible next to the Rust speedup.

### Rules

- **Never `zend_hash_str_update`** — it triggers the hash table destructor on the old value and frees the op_array. Use direct pointer write (`zval.value.ptr = new`) only.
- **Watch digits in Rust function names.** ext-php-rs inserts underscores at digit boundaries: `patina_foo_1arg` registers as PHP `patina_foo_1_arg`. If the replacement name in `OVERRIDES` / `SHIM_OVERRIDES` doesn't match the registered name, `zend_fetch_function_str` silently returns null and the swap is a no-op. Avoid digits, or verify with `get_extension_funcs("patina-ext")`.
- **Test swaps from freshly-compiled code**, never from the same file that defines the swap. PHP's `INIT_FCALL` cache slot fills on first call and pins to whatever was resolved — call the target from a `require`d file, an `eval`'d stub, or a separate entry script.
- **Shim body must NOT call its own WordPress name.** The shim for `wp_kses` trampolines to `patina_wp_kses_internal` (the Rust function), not back to `wp_kses` — otherwise it infinite-loops through the same swapped slot.
- **`ORIGINALS` tracking.** Every `swap_function` records the original `zend_function*` so `patina_deactivate()` can restore it. Direct pointer write only — never free.

### The reference implementation

- Direct swap: `esc_html`, `esc_attr` (see `OVERRIDES` in `patina-ext/src/lib.rs`).
- Shim: `wp_kses` via `__patina_wp_kses_shim__` → `patina_wp_kses_internal` (see `KSES_SHIM_PHP` + `SHIM_OVERRIDES`).

Both are activated by `patina_activate()` called from the bridge mu-plugin (`php/bridge/patina-bridge.php`) at WP mu-plugin load time — after core functions are defined, before plugins/themes run. When adding a new override, copy one of these two patterns; don't invent a third.
