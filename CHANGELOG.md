# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.1.0] - 2026-04-11

### Added

- Initial release
- PHP extension (`patina-ext`) built with ext-php-rs 0.15 for PHP 8.1-8.4
- **Pluggable function replacements** (registered directly, no bridge needed):
  - `wp_sanitize_redirect()` — URL sanitization for redirects
  - `wp_validate_redirect()` — redirect URL validation with `apply_filters` callback
- **Escaping functions** (registered as `patina_*`, bridge needed for WordPress routing):
  - `patina_esc_html()` — HTML entity encoding without double-encoding
  - `patina_esc_attr()` — HTML attribute encoding
- Utility functions: `patina_version()`, `patina_loaded()`
- 56 Rust tests including WordPress fixture validation (156 fixtures from WP 6.9.4)
- 179 PHP tests with 2192 assertions
- 2 cargo-fuzz targets
- Criterion benchmarks for all functions
- PHP benchmark harness with JIT on/off comparison
- CI: build matrix (PHP 8.1-8.4), PHPUnit tests, clippy, rustfmt, fuzz testing
- Release workflow: 8 artifacts (4 PHP versions x 2 architectures)
- Docker-based profiling stack (WordPress + SPX + k6)
- Install script with auto-detection

### Performance

- `esc_html`: 1.3-2.0x speedup vs PHP (medium inputs)
- `esc_attr`: 1.3-2.1x speedup vs PHP
- `wp_sanitize_redirect`: 1.1-1.6x speedup vs PHP
