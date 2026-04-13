# Patina

[![CI](https://github.com/PeterBooker/patina/actions/workflows/ci.yml/badge.svg)](https://github.com/PeterBooker/patina/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

An **EXPERIMENTAL** PHP extension written in Rust that replaces WordPress core functions with optimized native implementations. WordPress, themes, and plugins continue to function identically — hot-path string processing runs at near-native speed.

## Current Status

| Function | Override Mechanism | Speedup vs PHP |
|---|---|---|
| `esc_html()` | Direct swap | 1.5–1.9× |
| `esc_attr()` | Direct swap | 1.5–1.9× |
| `wp_kses()` *(and all wrappers)* | PHP user-function shim | 2.9–6.9× |
| `parse_blocks()` | PHP user-function shim | ~44 ms/request saved on block-heavy posts |
| `wp_sanitize_redirect()` | Pluggable replacement | 1.2–1.6× |
| `wp_validate_redirect()` | Pluggable replacement | — |

The `wp_kses` override catches every wrapper that calls it internally — `wp_kses_post`, `wp_kses_data`, `wp_filter_post_kses`, `wp_filter_kses`, `wp_filter_nohtml_kses`, `wp_kses_post_deep` — including the save pipeline (`content_save_pre` → `wp_filter_post_kses` → `wp_kses`). Filter compatibility is preserved: `pre_kses`, `wp_kses_allowed_html`, `kses_allowed_protocols`, and `wp_kses_uri_attributes` are all honored.

`parse_blocks` runs the Gutenberg block grammar in Rust and returns the exact nested-array shape WordPress produces (`blockName`, `attrs`, `innerBlocks`, `innerHTML`, `innerContent`). The `block_parser_class` filter is still respected — when a plugin swaps in a custom parser, the shim falls through to the stock PHP path.

## Requirements

- Linux (x86_64 or aarch64)
- PHP 8.1, 8.2, 8.3, or 8.4
- WordPress 6.6+

## Quick Install

```bash
# Auto-detect PHP version and architecture, download from GitHub Releases
curl -sSL https://raw.githubusercontent.com/PeterBooker/patina/main/install.sh | bash

# Or manually:
PHP_VERSION=$(php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;")
ARCH=$(uname -m)
wget https://github.com/PeterBooker/patina/releases/latest/download/patina-php${PHP_VERSION}-linux-${ARCH}.so

# Install
sudo cp patina-*.so $(php -r "echo ini_get('extension_dir');")/patina.so
echo "extension=patina.so" | sudo tee $(php --ini | grep 'Scan for' | cut -d: -f2 | tr -d ' ')/99-patina.ini
sudo systemctl restart php${PHP_VERSION}-fpm
```

## Verify

```bash
php -m | grep patina-ext
php -r "echo patina_version();"     # 0.1.0
php -r "echo patina_esc_html('<script>alert(1)</script>');"
# &lt;script&gt;alert(1)&lt;/script&gt;
```

## How It Works

Patina uses three different mechanisms to replace WordPress core functions, chosen per-function based on how PHP lets itself be intercepted.

### 1. Pluggable replacement

WordPress's pluggable functions (`wp_sanitize_redirect`, `wp_validate_redirect`, etc.) are defined inside `if (!function_exists(...))` blocks. The extension registers these under their original WP names at PHP startup, so by the time `pluggable.php` runs, `function_exists()` is already true and WordPress skips its PHP definition. No bridge, no configuration.

### 2. PHP user-function shim (non-pluggable functions)

For non-pluggable targets — `esc_html`, `esc_attr`, `wp_kses`, and friends — patina defines a **PHP user-function shim** at activation time via `ext_php_rs::php_eval::execute`, then swaps the WordPress function-table slot to point at the shim. Each shim forwards to an internal Rust function that holds the real logic:

```php
function __patina_wp_kses_shim__($content, $allowed_html, $allowed_protocols = array()) {
    return patina_wp_kses_internal((string) $content, $allowed_html, $allowed_protocols);
}
```

The shim layer serves two purposes at once:

1. **User→user dispatch stays valid for pre-compiled callers.** PHP 8.3's Zend compiler specializes call-site opcodes (`INIT_FCALL` / `DO_UCALL`) based on the target function's type at the moment the caller is compiled. When `wp-includes/kses.php` parses `wp_kses_post`, `wp_kses` is a user function — so the bytecode bakes in user-function call-frame semantics. A direct swap to an ext-php-rs internal function crashes those pre-compiled opcodes in `execute_ex`. Because our shim *is* a real PHP user function, that dispatch stays valid. The shim's own body was compiled *after* `patina_wp_kses_internal` was registered, so its own `INIT_FCALL` uses internal-dispatch semantics cleanly.

2. **`(string)` cast handles PHP's loose typing.** Stock PHP's `esc_html()` / `htmlspecialchars()` / `wp_kses()` are untyped and internally coerce any scalar to a string via `zval_get_string()`. ext-php-rs's `&str` parameter extractor is strict and rejects non-string zvals, throwing `Invalid value given for argument`. Moving the coercion into the PHP shim with a `(string) $x` cast reproduces stock WordPress behavior for every scalar type (int, float, bool, null), while keeping the Rust internal function strictly typed.

The single `wp_kses` shim transparently catches every wrapper that calls `wp_kses` internally — `wp_kses_post`, `wp_kses_data`, `wp_filter_post_kses` (the save pipeline), `wp_filter_kses`, `wp_filter_nohtml_kses`, `wp_kses_post_deep` — without any per-wrapper configuration. Overhead is one extra PHP frame per call, measured at ~300–1000 ns depending on input size, invisible next to Rust speedups of 4–7×.

### Filter compatibility

Every `wp_kses`-family override still honors the WordPress filter API: `pre_kses`, `wp_kses_allowed_html`, `kses_allowed_protocols`, and `wp_kses_uri_attributes` are all invoked from Rust via `apply_filters()` / `has_filter()` round-trips, so plugins that customize the allowed tag list, protocol allowlist, or URI attribute list continue to work unchanged.

## Architecture

```
patina-core   (pure Rust logic, no PHP dependency)
    ↓
patina-ext    (thin PHP extension wrapper via ext-php-rs)
    ↓
PHP / WordPress
```

All algorithms live in `patina-core` — testable with `cargo test`, benchmarkable with `cargo bench`, fuzzable with `cargo fuzz`. The PHP extension layer (`patina-ext`) is a thin wrapper: `#[php_function]` annotations, `catch_unwind` safety, and argument conversion.

See [docs/PROJECT_STRUCTURE.md](docs/PROJECT_STRUCTURE.md) for the full layout.

## Development

Only Docker is required. No local Rust or PHP needed.

```bash
git clone https://github.com/PeterBooker/patina.git
cd patina
make test       # Run all Rust + PHP tests
make bench      # Run PHP benchmarks
make check      # Full CI check (test + clippy + fmt + PHPUnit)
make shell      # Open a shell in the dev container
```

Build for a specific PHP version:
```bash
PHP_VERSION=8.1 make build
```

See `make help` for all targets. See [CONTRIBUTING.md](CONTRIBUTING.md) for local toolchain setup.

## Testing

```bash
make test             # All tests (Rust + PHP unit)
make test-rust        # Rust tests only
make test-php         # PHP unit tests only (no WordPress)
make test-integration # PHP integration tests (full WordPress stack, filter/hook coverage)
make bench            # PHP benchmarks (Rust vs PHP)
make bench-wp         # WP-backed benchmarks (kses family)
make bench-jit        # PHP benchmarks with JIT enabled
make bench-rust       # Criterion micro-benchmarks
make bench-http       # HTTP-level bench (k6 vs the profiling stack, TTFB per scenario)
```

`make bench-http` drives the profiling stack with [k6](https://k6.io/) over real HTTP requests, tracking `http_req_waiting` (TTFB) and `http_req_duration` per scenario. Raw k6 JSON is written under `/tmp/patina-bench/<timestamp>/k6-output.json`. Override the sample count with `ITERATIONS=200 make bench-http` (default is 100 post-warmup samples per scenario, plus 5 discarded warmup iterations).

The integration suite runs inside the profiling stack's php-fpm container — WordPress is fully loaded, the bridge mu-plugin is installed, and tests can register real filters via `add_filter()` to verify that plugin/theme customization still works against overridden functions. Use it whenever you add or modify an override that interacts with WordPress filters (`pre_kses`, `wp_kses_allowed_html`, `kses_allowed_protocols`, etc.).

## Rollback

Remove the extension — WordPress works exactly as before:

```bash
sudo rm $(php --ini | grep 'Scan for' | cut -d: -f2 | tr -d ' ')/99-patina.ini
sudo systemctl restart php*-fpm
```

No data migration, no database changes, nothing to undo.

## Roadmap

- **Next**: high-value targets using the mechanisms above — `esc_url`, `wpautop`, `sanitize_title`, `make_clickable`
- **Future**: PECL packaging, Composer distribution, OS packages

## License

MIT
