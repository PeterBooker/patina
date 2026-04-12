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

Replacing WordPress core functions in the Zend function table is subtle. **Read this before adding a new override** — the naive direct-swap approach has two independent failure modes that both bite.

### The policy: always use a PHP user-function shim

Every core function override goes through a PHP shim. There is no "direct swap" path anymore — we tried that and it broke in two different ways (see below). The shim approach is the only pattern we trust across every WordPress function signature.

**Adding a new override:**

1. Write a Rust internal function `patina_<name>_internal` with a clean typed signature. Use `&str` for string params, `&Zval` only for polymorphic params (like `$allowed_html` in `wp_kses` which is string-or-array).
2. Add a PHP shim definition to the `PATINA_SHIMS_PHP` string in `patina-ext/src/lib.rs`. The shim signature mirrors the WordPress function **byte-for-byte**, including optional args and default values. For string-typed params, cast with `(string) $param` before passing to the Rust internal.
3. Add `("wp_name", "__patina_<name>_shim__")` to `SHIM_OVERRIDES`.
4. Register the internal function in the module builder via `wrap_function!(patina_<name>_internal)`.
5. Add an integration test in `php/tests-integration/` that registers a real WordPress filter and verifies the Rust override honors it — don't ship an override without filter-compatibility coverage.

At activation time, `patina_activate()` evals `PATINA_SHIMS_PHP` once (defining all shims as PHP user functions), then swaps each entry in `SHIM_OVERRIDES` — the WordPress function-table slot gets its `zend_function*` overwritten to point at the shim.

### Why shim-only — two failure modes of direct swap

**Failure mode 1: compile-time opcode specialization (crashes `execute_ex`).**
PHP's Zend compiler specializes `INIT_FCALL` / `DO_UCALL` opcodes based on the target function's type **at the moment the caller is compiled**. When `wp-includes/kses.php` parses `wp_kses_post`'s body, `wp_kses` is a user function — so the opcodes bake in user-function call-frame semantics. Swapping `wp_kses` to an ext-php-rs internal replacement makes those pre-compiled opcodes crash in `execute_ex` *before* the Rust handler is even entered. 1-arg functions with no defaults (like `esc_html`) happen to escape this by accident, but it's not a property you can reason about safely per-function — it's easier to assume every multi-arg or defaulted signature will crash, and always shim. The shim, being a real PHP user function, keeps user→user dispatch valid for pre-compiled callers.

**Failure mode 2: strict parameter parsing (`Invalid value given for argument`).**
ext-php-rs's `&str` parameter extractor is strict — it rejects any zval that isn't already a string. Stock PHP's `esc_html()`, `wp_kses()`, etc. are untyped and accept any scalar because `htmlspecialchars()` / `zval_get_string()` internally coerce. An ext-php-rs internal function with `text: &str` throws `Invalid value given for argument` when WordPress passes it an int (e.g. `esc_attr($per_page)` from wp-admin screen options). **PHP does not coerce at the shim→internal call boundary either** — we verified this empirically. The fix: do the coercion in the PHP shim via `(string) $param`. That way the Rust function sees a proper string and its `&str` signature works cleanly.

**The shim + `(string)` cast solves both failure modes at once.** User→user dispatch stays valid for pre-compiled callers, and the PHP-level cast handles every scalar type (int, float, bool, null) before it ever reaches Rust.

### Pluggable functions are the one exception

`wp_sanitize_redirect` / `wp_validate_redirect` (and future pluggables) are registered at PHP MINIT under the WordPress function name. Because PHP compiles all user code *after* MINIT, pre-compiled callers never exist — failure mode 1 can't apply. They're registered directly as ext-php-rs internal functions with no shim.

But pluggables still hit failure mode 2 (loose typing), so they use `&Zval` for string params and call `coerce_to_string().unwrap_or_default()` in the function body. Optional args must use `Option<&Zval>` to match PHP's default-value semantics.

### Rules

- **Always shim non-pluggable overrides.** Don't write a "small optimization" direct swap. The 300–1000 ns shim overhead is noise at Rust speedups of 4–7×, and uniformity avoids the two failure modes above.
- **Never `zend_hash_str_update`** — it triggers the hash table destructor on the old value and frees the op_array. Use direct pointer write (`zval.value.ptr = new`) only.
- **Watch digits in Rust function names.** ext-php-rs inserts underscores at digit boundaries: `patina_foo_1arg` registers as PHP `patina_foo_1_arg`. If the replacement name in `SHIM_OVERRIDES` doesn't match the registered name, `zend_fetch_function_str` silently returns null and the swap is a no-op. Avoid digits in Rust function names, or verify with `get_extension_funcs("patina-ext")`.
- **Test shimmed overrides from freshly-compiled code.** Integration tests in `php/tests-integration/` are the right place — they boot WordPress fully and exercise the override through real wp-admin code paths. Never call the target function from the same file that defines the shim.
- **Shim body must NOT call its own WordPress name.** The shim for `wp_kses` trampolines to `patina_wp_kses_internal` (the Rust internal), not back to `wp_kses` — otherwise it infinite-loops through the same swapped slot.
- **`ORIGINALS` tracking.** Every `swap_function` records the original `zend_function*` so `patina_deactivate()` can restore it. Direct pointer write only — never free.

### The reference implementation

All three current overrides share the same pattern:

| WP function | Shim (user fn) | Rust internal | String coercion |
|---|---|---|---|
| `esc_html` | `__patina_esc_html_shim__` | `patina_esc_html_internal(text: &str)` | `(string) $text` in shim |
| `esc_attr` | `__patina_esc_attr_shim__` | `patina_esc_attr_internal(text: &str)` | `(string) $text` in shim |
| `wp_kses` | `__patina_wp_kses_shim__` | `patina_wp_kses_internal(content: &str, allowed_html: &Zval, allowed_protocols: &Zval)` | `(string) $content` in shim; `$allowed_html` / `$allowed_protocols` forwarded as-is |

See `SHIM_OVERRIDES` and `PATINA_SHIMS_PHP` in `patina-ext/src/lib.rs`. All three are activated by `patina_activate()` called from the bridge mu-plugin (`php/bridge/patina-bridge.php`) at WP mu-plugin load time — after core functions are defined, before plugins/themes run. When adding a new override, copy this pattern exactly.
