# Patina

[![CI](https://github.com/PeterBooker/patina/actions/workflows/ci.yml/badge.svg)](https://github.com/PeterBooker/patina/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A PHP extension written in Rust that replaces WordPress core functions with optimized native implementations. WordPress, themes, and plugins continue to function identically — hot-path string processing runs at near-native speed.

## Current Status

**v0.1.0** — Proof of concept with 4 replaced functions:

| Function | Type | Speedup vs PHP | Speedup vs PHP+JIT |
|---|---|---|---|
| `esc_html()` | Non-pluggable* | 1.5-1.8x | 1.4-2.0x |
| `esc_attr()` | Non-pluggable* | 1.5-1.9x | 1.3-2.1x |
| `wp_sanitize_redirect()` | Pluggable | 1.2-1.6x | 1.0-1.9x |
| `wp_validate_redirect()` | Pluggable | — | — |

\* `esc_html` and `esc_attr` are registered under `patina_esc_html` / `patina_esc_attr` prefixed names. A bridge mu-plugin is needed to route WordPress calls to them (Phase 8-9 work).

Pluggable functions (`wp_sanitize_redirect`, `wp_validate_redirect`) replace WordPress's definitions directly — no bridge needed.

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

**Pluggable functions** (`wp_sanitize_redirect`, `wp_validate_redirect`): The extension registers these at PHP startup under their original WordPress names. When WordPress loads `pluggable.php` and checks `function_exists()`, it skips its PHP definition. Zero configuration needed.

**Non-pluggable functions** (`esc_html`, `esc_attr`, and future targets like `wp_kses`, `wpautop`): Registered under `patina_*` prefixed names. A bridge mu-plugin will route WordPress calls to the Rust implementations once the interception strategy is finalized.

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
make test           # All tests (Rust + PHP)
make test-rust      # Rust tests only (56 tests)
make test-php       # PHP tests only (179 tests, 2192 assertions)
make bench          # PHP benchmarks (Rust vs PHP)
make bench-jit      # PHP benchmarks with JIT enabled
make bench-rust     # Criterion micro-benchmarks
```

## Rollback

Remove the extension — WordPress works exactly as before:

```bash
sudo rm $(php --ini | grep 'Scan for' | cut -d: -f2 | tr -d ' ')/99-patina.ini
sudo systemctl restart php*-fpm
```

No data migration, no database changes, nothing to undo.

## Roadmap

- **Phase 8**: Non-pluggable function interception strategy (uopz / Zend function table manipulation)
- **Phase 9**: High-value targets — `esc_url`, `wp_kses` (8-15% of wall time), `wpautop`, `sanitize_title`, `make_clickable`, `parse_blocks`
- **Future**: PECL packaging, Composer distribution, OS packages

## License

MIT
