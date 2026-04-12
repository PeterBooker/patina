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
| `wp_sanitize_redirect()` | Pluggable replacement | 1.2–1.6× |
| `wp_validate_redirect()` | Pluggable replacement | — |

The `wp_kses` override catches every wrapper that calls it internally — `wp_kses_post`, `wp_kses_data`, `wp_filter_post_kses`, `wp_filter_kses`, `wp_filter_nohtml_kses`, `wp_kses_post_deep` — including the save pipeline (`content_save_pre` → `wp_filter_post_kses` → `wp_kses`). Filter compatibility is preserved: `pre_kses`, `wp_kses_allowed_html`, `kses_allowed_protocols`, and `wp_kses_uri_attributes` are all honored.

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

### 2. Direct Zend function table swap

For non-pluggable functions whose signature can be matched exactly by a simple Rust parameter list (e.g. `esc_html(string $text)`), the bridge mu-plugin calls `patina_activate()` after WordPress core loads. That walks an override table and overwrites each target's `zend_function*` pointer in the Zend function table with the Rust implementation. Every subsequent call to `esc_html()` from PHP code dispatches directly to Rust — no PHP wrapper overhead.

### 3. PHP user-function shim

For functions with multi-arg, optional, or mixed-type signatures — `wp_kses($content, $allowed_html, $allowed_protocols = array())` is the canonical example — a direct swap **crashes**. PHP 8.3's Zend compiler specializes call-site opcodes (`INIT_FCALL` / `DO_UCALL`) based on the target function's type at the moment the caller is compiled. When `wp-includes/kses.php` parses `wp_kses_post`, `wp_kses` is a user function, so the bytecode bakes in user-function call-frame semantics. Swapping `wp_kses` to an ext-php-rs internal replacement makes those pre-compiled opcodes crash in `execute_ex` before the Rust handler is even entered.

Patina solves this by defining a **PHP user-function shim** at activation time via `ext_php_rs::php_eval::execute`:

```php
function __patina_wp_kses_shim__($content, $allowed_html, $allowed_protocols = array()) {
    return patina_wp_kses_internal($content, $allowed_html, $allowed_protocols);
}
```

The function table slot for `wp_kses` is then swapped to point at the shim. Pre-compiled callers like `wp_kses_post` dispatch user→user (safe — the shim is a real user function); the shim itself dispatches user→internal (safe — the shim's own body was compiled *after* `patina_wp_kses_internal` was registered, so its `INIT_FCALL` uses internal-dispatch semantics). Overhead is one extra PHP frame per call, measured at under 1µs — negligible next to the Rust speedup.

This shim mechanism is what enables `wp_kses` and every function that calls it (`wp_kses_post`, `wp_filter_post_kses`, `wp_kses_data`, …) to route through Rust without any PHP-side wrapper code. The single `wp_kses` override catches the entire family, including the save pipeline.

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
```

The integration suite runs inside the profiling stack's php-fpm container — WordPress is fully loaded, the bridge mu-plugin is installed, and tests can register real filters via `add_filter()` to verify that plugin/theme customization still works against overridden functions. Use it whenever you add or modify an override that interacts with WordPress filters (`pre_kses`, `wp_kses_allowed_html`, `kses_allowed_protocols`, etc.).

## Rollback

Remove the extension — WordPress works exactly as before:

```bash
sudo rm $(php --ini | grep 'Scan for' | cut -d: -f2 | tr -d ' ')/99-patina.ini
sudo systemctl restart php*-fpm
```

No data migration, no database changes, nothing to undo.

## Roadmap

- **Next**: high-value targets using the mechanisms above — `esc_url`, `wpautop`, `sanitize_title`, `make_clickable`, `parse_blocks`
- **Future**: PECL packaging, Composer distribution, OS packages

## License

MIT
