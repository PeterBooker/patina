# Patina: Project & Code Structure

## Design Principles

1. **Find anything in 5 seconds.** Given a WordPress function name, you should be able to locate its Rust implementation, PHP wrapper, test fixtures, benchmarks, and fuzz target without searching.

2. **One function, one file.** Each WordPress function gets its own source file. Related functions that share an implementation (e.g., `esc_html` and `esc_attr` both call `_wp_specialchars`) live in sibling files within a shared module, with the common code factored into its own file.

3. **The cdylib boundary is a thin wrapper.** `patina-ext` contains only PHP glue: `#[php_function]` signatures, `catch_unwind`, and argument conversion. All logic lives in `patina-core`. If you can't test something with `cargo test -p patina-core`, it's in the wrong crate.

4. **Mirror structure across layers.** The module grouping in `patina-core` is repeated in `patina-ext`, PHP tests, and benchmarks. The `escaping/` group in Rust maps to `escaping.rs` in the extension, `Escaping/` in PHP tests, and `escaping.rs` in benchmarks.

5. **Fixtures are the source of truth for correctness.** JSON fixtures generated from WordPress are committed to the repo and consumed by both Rust and PHP tests. They are generated once per WordPress version, not per build.

---

## Top-Level Layout

```
patina/
│
├── Cargo.toml                     # Workspace definition
├── rust-toolchain.toml            # Rust channel + components
├── .gitignore
├── LICENSE
├── README.md
│
├── crates/                        # All Rust code
│   ├── patina-core/               #   Pure Rust logic (lib crate)
│   ├── patina-ext/                #   PHP extension (cdylib)
│   ├── patina-bench/              #   Criterion benchmarks
│   └── patina-fuzz/               #   Fuzz targets
│
├── fixtures/                      # JSON test fixtures (shared by Rust + PHP)
│   └── baselines/                 #   Committed HTTP-bench baselines (Phase 6 output)
│
├── php/                           # All PHP code
│   ├── bridge/                    #   mu-plugin for non-pluggable interception
│   ├── fixture-generator/         #   Script to generate fixtures from WordPress
│   ├── tests/                     #   PHPUnit tests
│   ├── tests-integration/         #   PHPUnit tests run against a real WP boot
│   └── benchmarks/                #   PHP-level benchmarks
│
├── profiling/                     # WordPress profiling environment + HTTP bench stack
│   └── benchmark-content/         #   Seed corpus for realistic HTTP benches
│
├── scripts/                       # Bench runner, aggregator, comparator, SPX helper
│
├── docker/                        # Build and test Dockerfiles
│
├── .github/workflows/             # CI/CD
│
└── docs/                          # Project documentation
    ├── BENCHMARKS.md               #   End-to-end HTTP bench results + analysis
    ├── BENCHMARK_PLAN.md           #   The six-phase bench plan (status tracker)
    └── PROJECT_STRUCTURE.md        #   This file
```

### Why these top-level divisions?

| Directory | Concern | Changes when... |
|---|---|---|
| `crates/` | Rust implementation | A function is added/modified |
| `fixtures/` | Correctness contract + committed bench baselines | WordPress changes behavior, a new function is added, or a new HTTP baseline is taken |
| `php/` | PHP integration layer | Bridge logic changes, new PHP tests, new benchmarks |
| `profiling/` | Performance measurement infra + bench seed corpus | Workloads change, content corpus grows, or tools are updated |
| `scripts/` | Bench runner + comparison tooling | Bench harness evolves (rare) |
| `docker/` | Build/test environments | PHP versions or build deps change |
| `.github/` | CI automation | Pipeline logic changes |
| `docs/` | Human knowledge | Decisions or processes change |

These concerns are independent. A change to the profiling stack shouldn't touch `crates/`. Adding a Rust function shouldn't require changes in `profiling/`. The bridge PHP code and the test PHP code serve different purposes and live in different directories.

---

## Fixtures: The Shared Correctness Layer

Fixtures live at the repo root because they sit between the Rust and PHP worlds — neither owns them.

```
fixtures/
├── README.md                      # Format spec, generation instructions
├── esc_html.json
├── esc_attr.json
├── esc_url.json
├── wp_kses_post.json
├── wpautop.json
├── sanitize_title.json
├── sanitize_file_name.json
├── make_clickable.json
├── wp_check_invalid_utf8.json
├── wp_sanitize_redirect.json
├── wp_validate_redirect.json
└── parse_blocks.json
```

**Format** (every file follows this schema):

```json
[
  {
    "input": ["<script>alert(1)</script>"],
    "output": "&lt;script&gt;alert(1)&lt;/script&gt;"
  }
]
```

- `input` is always an array (positional arguments to the function)
- `output` is the return value
- For functions with boolean/optional arguments, the array includes them: `["text", true]`

**How they're consumed:**

| Consumer | Access method |
|---|---|
| Rust tests (`patina-core`) | `std::fs::read_to_string` via helper using `CARGO_MANIFEST_DIR` + `../../fixtures/` |
| PHP tests | `json_decode(file_get_contents(__DIR__ . '/../../fixtures/...'))` |
| CI | Both paths work because fixtures are in the checkout |

**Generation:**

```bash
# From the profiling Docker stack (WordPress must be running WITHOUT the extension)
docker compose -f profiling/docker-compose.yml exec php-fpm \
    php /app/php/fixture-generator/generate.php --all
```

This writes every `<function>.json` into `fixtures/`. Fixtures are committed and only regenerated when targeting a new WordPress version.

---

## Rust Crates

### `patina-core` — Pure Logic

This crate contains every algorithm. No PHP types, no ext-php-rs, no FFI. Everything here is testable with `cargo test` and benchmarkable with `cargo bench`.

```
crates/patina-core/
├── Cargo.toml
└── src/
    ├── lib.rs                     # Re-exports all public modules
    │
    ├── util/                      # Shared building blocks
    │   ├── mod.rs
    │   ├── entities.rs            # HTML entity detection & preservation
    │   ├── null_bytes.rs          # wp_kses_no_null: null byte / %00 stripping
    │   └── byte_class.rs          # Const lookup tables for character classification
    │
    ├── escaping/                  # esc_html, esc_attr, esc_url
    │   ├── mod.rs                 # Re-exports public functions
    │   ├── specialchars.rs        # _wp_specialchars: core entity-encoding logic
    │   ├── html.rs                # esc_html() — calls specialchars
    │   ├── attr.rs                # esc_attr() — calls specialchars
    │   └── url.rs                 # esc_url() — URL validation + encoding
    │
    ├── kses/                      # wp_kses HTML sanitization family
    │   ├── mod.rs                 # wp_kses(), wp_kses_post(), wp_kses_data()
    │   ├── allowed_html.rs        # AllowedHtmlSpec: compiled tag/attr allowlist
    │   ├── tag_parser.rs          # HTML tag tokenizer + attribute parser
    │   ├── protocols.rs           # Protocol allowlist validation
    │   └── normalize.rs           # Entity normalization (wp_kses_normalize_entities)
    │
    ├── formatting/                # Text transformation functions
    │   ├── mod.rs
    │   ├── autop.rs               # wpautop() — double-newline to <p> conversion
    │   ├── clickable.rs           # make_clickable() — auto-linking URLs
    │   └── utf8.rs                # wp_check_invalid_utf8()
    │
    ├── sanitize/                  # Input sanitization
    │   ├── mod.rs
    │   ├── title.rs               # sanitize_title(), sanitize_title_with_dashes()
    │   └── filename.rs            # sanitize_file_name()
    │
    ├── blocks/                    # Gutenberg block parser
    │   ├── mod.rs                 # parse_blocks() entry point
    │   ├── grammar.rs             # Block grammar state machine / parser
    │   └── types.rs               # Block, BlockAttributes structs
    │
    ├── pluggable/                 # WordPress pluggable.php functions
    │   ├── mod.rs
    │   ├── sanitize_redirect.rs   # wp_sanitize_redirect()
    │   └── validate_redirect.rs   # wp_validate_redirect()
    │
    └── test_support.rs            # Fixture loading helper (cfg(test) only)
```

#### Module grouping rationale

The grouping follows **what the function does**, not where WordPress defines it:

| Module | Purpose | Contains |
|---|---|---|
| `escaping/` | Make unsafe strings safe for a specific output context | `esc_html`, `esc_attr`, `esc_url` |
| `kses/` | Strip disallowed HTML tags and attributes | `wp_kses`, `wp_kses_post`, `wp_kses_data` |
| `formatting/` | Transform text structure or content | `wpautop`, `make_clickable`, `wp_check_invalid_utf8` |
| `sanitize/` | Clean user input for storage | `sanitize_title`, `sanitize_file_name` |
| `blocks/` | Parse Gutenberg block markup | `parse_blocks` |
| `pluggable/` | Functions from `pluggable.php` | `wp_sanitize_redirect`, `wp_validate_redirect` |
| `util/` | Shared primitives used by multiple modules | Entity handling, null stripping, char tables |

The `pluggable/` module exists as a separate group because these functions have a **different integration mechanism** (registered directly by the extension, no bridge needed) — that architectural distinction is worth encoding in the directory structure.

#### The `util/` module in detail

These are small, focused utilities that multiple function modules depend on. They are NOT WordPress functions themselves — they're implementation details.

**`entities.rs`** — HTML entity detection:
```rust
/// Returns true if `s` at position `pos` starts a valid HTML entity
/// (e.g., &amp; &#039; &#x41;). Used by esc_html/esc_attr to avoid
/// double-encoding, and by kses for entity normalization.
pub fn is_valid_entity_at(s: &str, pos: usize) -> bool { ... }

/// Returns the byte length of the entity starting at `pos`, or 0 if
/// not a valid entity.
pub fn entity_len_at(s: &str, pos: usize) -> usize { ... }
```

**`null_bytes.rs`** — WordPress's `wp_kses_no_null()`:
```rust
/// Strips null bytes and \0 escape sequences from a string.
/// Reimplements wp_kses_no_null() which is called by wp_kses and
/// wp_sanitize_redirect.
pub fn strip_null_bytes(s: &str) -> String { ... }
```

**`byte_class.rs`** — Const lookup tables:
```rust
/// 256-entry boolean lookup table for URL-safe characters.
/// Used by wp_sanitize_redirect and esc_url.
pub const URL_SAFE: [bool; 256] = { ... };
```

#### `lib.rs` — public API

```rust
pub mod util;
pub mod escaping;
pub mod kses;
pub mod formatting;
pub mod sanitize;
pub mod blocks;
pub mod pluggable;

#[cfg(test)]
mod test_support;
```

Each module's `mod.rs` re-exports the public function(s) so callers can write:
```rust
use patina_core::escaping::esc_html;
use patina_core::pluggable::sanitize_redirect;
```

#### `test_support.rs` — Fixture loading

```rust
#[cfg(test)]
use std::path::{Path, PathBuf};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Fixture {
    pub input: Vec<serde_json::Value>,
    pub output: serde_json::Value,
}

/// Returns the path to the repo-root `fixtures/` directory.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..") // crates/
        .join("..") // repo root
        .join("fixtures")
}

/// Load fixtures for a given function name.
pub fn load_fixtures(function: &str) -> Vec<Fixture> {
    let path = fixtures_dir().join(format!("{function}.json"));
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture not found: {}: {e}", path.display()));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("fixture parse error: {}: {e}", path.display()))
}
```

Each function module uses this in tests:
```rust
#[cfg(test)]
mod tests {
    use crate::test_support::load_fixtures;
    use super::*;

    #[test]
    fn matches_wordpress_output() {
        for (i, f) in load_fixtures("esc_html").iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let expected = f.output.as_str().unwrap();
            assert_eq!(esc_html(input), expected, "fixture {i}: {input:?}");
        }
    }
}
```

#### Cargo.toml

```toml
[package]
name = "patina-core"
version.workspace = true
edition.workspace = true

[dependencies]
memchr = "2"            # SIMD byte searching (used across most modules)

# Added per-module as needed:
# aho-corasick = "1"    # kses: multi-pattern tag matching
# regex = "1"           # formatting: complex pattern matching
# url = "2"             # escaping/url: URL parsing

[dev-dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Dependencies grow incrementally. Only `memchr` is needed from the start (it's useful everywhere). Other crates are added when their module is implemented.

---

### `patina-ext` — PHP Extension

Thin wrapper. All `#[php_function]` definitions live in `lib.rs` because ext-php-rs 0.15 generates private `_internal_*` types that must be visible to `wrap_function!()` in the `#[php_module]` block. Splitting functions into submodules creates visibility issues.

This is fine in practice — each wrapper is ~3 lines, and at full buildout (~15 functions) `lib.rs` is ~120 lines with clear section comments.

```
crates/patina-ext/
├── Cargo.toml
└── src/
    ├── lib.rs                 # ALL #[php_function] wrappers + #[php_module] registration
    ├── panic_guard.rs         # catch_unwind → PhpException conversion
    └── php_callback.rs        # Calling PHP functions (apply_filters) from Rust
```

#### `lib.rs` structure

```rust
use ext_php_rs::prelude::*;

mod panic_guard;
pub mod php_callback;

// -- Info functions --
#[php_function]
pub fn patina_version() -> &'static str { env!("CARGO_PKG_VERSION") }

// -- Escaping (patina_ prefix, bridge routes from WP name) --
#[php_function]
pub fn patina_esc_html(text: &str) -> PhpResult<String> {
    panic_guard::guarded("patina_esc_html", || patina_core::escaping::esc_html(text))
}

// -- Pluggable (original WP name, replaces pluggable.php definition) --
#[php_function]
pub fn wp_sanitize_redirect(location: &str) -> PhpResult<String> {
    panic_guard::guarded("wp_sanitize_redirect", || {
        patina_core::pluggable::sanitize_redirect(location)
    })
}

// -- Module registration (every function must be listed here) --
#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(patina_version))
        .function(wrap_function!(patina_esc_html))
        .function(wrap_function!(wp_sanitize_redirect))
}
```

**ext-php-rs 0.15 requires explicit registration.** The `#[php_function]` macro generates a private `_internal_*` struct; `wrap_function!()` in the `#[php_module]` block references it. Both must be in the same module scope (i.e., `lib.rs`). Functions not listed in `get_module()` are silently not registered.

#### Function naming in PHP

Two naming strategies coexist, reflecting the two interception mechanisms:

| Mechanism | PHP function name | Example |
|---|---|---|
| **Pluggable replacement** | Original WordPress name | `wp_sanitize_redirect` |
| **Bridge-assisted** (non-pluggable) | `patina_` prefix | `patina_esc_html` |

Pluggable functions register under the original name so `function_exists()` prevents WordPress from defining its PHP version. Non-pluggable functions register under a prefixed name; the bridge mu-plugin handles the routing from the original name to the prefixed one.

#### `parse_blocks` — ZendHashTable construction (future)

The block parser is a special case because it returns complex nested arrays, not strings. This is the one place `patina-ext` has meaningful logic beyond trivial wrapping:

```rust
use ext_php_rs::types::ZendHashTable;

#[php_function]
pub fn patina_parse_blocks(content: &str) -> PhpResult<ZendHashTable> {
    panic_guard::guarded("patina_parse_blocks", || {
        let blocks = patina_core::blocks::parse_blocks(content);
        // Convert Vec<Block> → ZendHashTable (recursive)
        blocks_to_zend_hash_table(&blocks)
    })
}

fn blocks_to_zend_hash_table(blocks: &[patina_core::blocks::Block]) -> ZendHashTable {
    // Build the nested PHP array structure
    // This is the only place with non-trivial PHP-type construction
    todo!()
}
```

#### Cargo.toml

```toml
[package]
name = "patina-ext"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["cdylib"]
name = "patina"

[dependencies]
patina-core = { path = "../patina-core" }
ext-php-rs = "0.13"
```

No other dependencies. The extension crate is deliberately minimal.

---

### `patina-bench` — Benchmarks

```
crates/patina-bench/
├── Cargo.toml
├── benches/
│   ├── escaping.rs                # esc_html, esc_attr, esc_url benchmarks
│   ├── kses.rs                    # wp_kses_post benchmarks
│   ├── formatting.rs              # wpautop, make_clickable benchmarks
│   ├── sanitize.rs                # sanitize_title, sanitize_file_name
│   ├── blocks.rs                  # parse_blocks benchmarks
│   └── pluggable.rs               # wp_sanitize_redirect, wp_validate_redirect
└── data/                          # Real-world HTML content for benchmarks
    ├── short-paragraph.html       # ~100 bytes — typical esc_html input
    ├── medium-post.html           # ~5KB — average blog post
    ├── gutenberg-heavy.html       # ~50KB — block-heavy page
    └── woocommerce-product.html   # ~200KB — complex product page
```

One bench file per `patina-core` module. Bench data is committed real-world HTML (extracted from the profiling WordPress instance).

```toml
# Cargo.toml
[package]
name = "patina-bench"
version.workspace = true
edition.workspace = true

# One [[bench]] entry per file
[[bench]]
name = "escaping"
harness = false

[[bench]]
name = "kses"
harness = false

[[bench]]
name = "formatting"
harness = false

[[bench]]
name = "sanitize"
harness = false

[[bench]]
name = "blocks"
harness = false

[[bench]]
name = "pluggable"
harness = false

[dependencies]
patina-core = { path = "../patina-core" }
criterion = "0.5"
```

---

### `patina-fuzz` — Fuzz Targets

```
crates/patina-fuzz/
├── Cargo.toml
└── fuzz_targets/
    ├── esc_html.rs
    ├── esc_attr.rs
    ├── esc_url.rs
    ├── wp_kses_post.rs
    ├── wpautop.rs
    ├── sanitize_title.rs
    ├── sanitize_redirect.rs
    └── parse_blocks.rs
```

One fuzz target per high-value function. Each target:
1. Accepts arbitrary bytes
2. Converts to UTF-8 (skip if invalid)
3. Calls the function
4. Asserts invariants (no panics, output properties hold)

```toml
# Cargo.toml — cargo-fuzz requires this exact structure
[package]
name = "patina-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
patina-core = { path = "../patina-core" }

# cargo-fuzz requires a local workspace
[workspace]
members = ["."]
```

---

## PHP Code

### `php/bridge/` — mu-plugin

```
php/bridge/
└── patina-bridge.php
```

Single file. Only needed once the non-pluggable interception strategy (Phase 8) is decided. For the initial pluggable-only release, this file is either absent or a no-op.

Structure when populated:

```php
<?php
/**
 * Plugin Name: Patina Bridge
 * Description: Routes WordPress core functions to Patina native implementations.
 */

if (!extension_loaded('patina')) {
    return; // Graceful degradation — WordPress works normally
}

if (getenv('PATINA_DISABLE') || (defined('PATINA_DISABLE') && PATINA_DISABLE)) {
    return; // Kill switch
}

// --- Escaping ---
// (Mechanism depends on Phase 8 decision — uopz, Zend table, etc.)

// --- KSES ---
// Initialize the allowed HTML spec once, on 'init'
add_action('init', function() {
    if (function_exists('patina_kses_init')) {
        patina_kses_init(json_encode($GLOBALS['allowedposttags']));
    }
}, 1);

// --- Formatting ---
// ...
```

The bridge is organized by the same module groups as the Rust code: escaping, kses, formatting, etc. Each section is clearly commented.

### `php/fixture-generator/` — Generates fixtures from WordPress

```
php/fixture-generator/
├── generate.php                   # Main entry point
├── corpus/                        # Test input collections
│   ├── strings.php                # General string inputs (empty, ASCII, multibyte, etc.)
│   ├── html.php                   # HTML-specific inputs (tags, entities, nesting)
│   ├── urls.php                   # URL inputs (protocols, IDN, query strings)
│   └── content.php                # Real WordPress content (extracted from theme unit test)
└── functions/                     # Per-function fixture definitions
    ├── esc_html.php               # How to call esc_html + which corpus subsets to use
    ├── wp_kses_post.php
    ├── wp_sanitize_redirect.php
    └── ...
```

**`generate.php`** usage:
```bash
php generate.php --function=esc_html         # One function
php generate.php --all                        # All functions
php generate.php --all --output=../../fixtures/  # Custom output dir
```

Each file in `functions/` defines:
1. The PHP function to call
2. Which corpus subsets are relevant
3. Any function-specific edge cases beyond the standard corpus

### `php/tests/` — PHPUnit Extension Tests

```
php/tests/
├── bootstrap.php                  # Verifies extension loaded, bootstraps WP test lib
├── phpunit.xml
│
├── Escaping/
│   ├── EscHtmlTest.php
│   ├── EscAttrTest.php
│   └── EscUrlTest.php
├── Kses/
│   └── KsesPostTest.php
├── Formatting/
│   ├── AutopTest.php
│   ├── ClickableTest.php
│   └── Utf8Test.php
├── Sanitize/
│   ├── TitleTest.php
│   └── FilenameTest.php
├── Blocks/
│   └── ParseBlocksTest.php
├── Pluggable/
│   ├── SanitizeRedirectTest.php
│   └── ValidateRedirectTest.php
└── Smoke/
    └── ExtensionLoadTest.php      # Verifies extension loads, version(), loaded()
```

Directory structure mirrors `patina-core` modules. Every test class:
1. Loads fixtures from `fixtures/`
2. Runs the function (now implemented in Rust, either via pluggable replacement or bridge)
3. Asserts output matches fixtures
4. Includes a fuzz-style "random bytes must not crash" test

### `php/benchmarks/` — PHP-Level Benchmarks

```
php/benchmarks/
├── Runner.php                     # Benchmark harness (timing, comparison, reporting)
├── reference/                     # Verbatim copies of original WordPress PHP implementations
│   ├── escaping.php               # esc_html, esc_attr, esc_url (original PHP)
│   ├── kses.php                   # wp_kses_post (original PHP + dependencies)
│   ├── formatting.php             # wpautop, make_clickable (original PHP)
│   ├── sanitize.php               # sanitize_title, sanitize_file_name (original PHP)
│   └── pluggable.php              # wp_sanitize_redirect, wp_validate_redirect (original PHP)
├── suites/
│   ├── escaping.php               # Benchmarks for esc_html/attr/url
│   ├── kses.php
│   ├── formatting.php
│   ├── sanitize.php
│   └── pluggable.php
└── run.php                        # Entry point: runs all suites, prints report
```

The `reference/` directory contains the original WordPress PHP implementations, copied verbatim with renamed function names (e.g., `reference_esc_html`). This allows comparing Rust vs PHP in the same process even when the extension replaces the original.

---

## Infrastructure

### `docker/` — Build and Test Environments

```
docker/
├── Dockerfile.build               # Multi-stage: builds .so for a given PHP version
└── docker-compose.test.yml        # PHP CLI + MariaDB for running PHP tests
```

`Dockerfile.build` accepts `PHP_VERSION` as a build arg. It installs Rust, compiles the extension, and verifies it loads. Used by CI and local development.

`docker-compose.test.yml` provides:
- PHP CLI with the extension installed
- MariaDB for WordPress test suite
- Volumes mounting the repo for test access

### `profiling/` — WordPress Profiling Environment

```
profiling/
├── docker-compose.yml             # Nginx + PHP-FPM + SPX + k6 + MariaDB
├── Dockerfile.profiling           # PHP-FPM with SPX, WP-CLI, and k6
├── nginx.conf                     # WordPress Nginx config
├── setup-wordpress.sh             # Automated WP install + bench seed
├── seed-benchmark-content.sh      # Phase 2 seed: TT25, theme-test-data WXR,
│                                  # block-tier corpus, stable bench slugs
├── k6-workloads.js                # k6 HTTP bench scenarios (TTFB per URL)
└── benchmark-content/             # Seed fixtures + WXR pin file
    ├── WXR_PIN.env                #   Pinned upstream theme-test-data ref
    ├── posts-short.html           #   ~500 B block tier
    ├── posts-medium.html          #   ~3 KB block tier
    ├── posts-long.html            #   ~8 KB block tier (deep nesting)
    └── classic-post.html          #   Pre-Gutenberg HTML — wpautop zone
```

`docker compose up && ./setup-wordpress.sh` gives a fully seeded
WordPress install ready for HTTP benches. The seed script fetches the
upstream theme-test-data WXR pinned to a SHA, imports it, and layers
the block-tier corpus and stable bench slugs on top so `scripts/bench-runner.sh`
can target them deterministically.

### `scripts/` — Bench runner and comparison tooling

```
scripts/
├── bench-runner.sh                # Full matrix: builds .so, iterates configs,
│                                  # restarts php-fpm, runs k6, aggregates
├── bench-aggregate.py             # k6 NDJSON → Patina summary schema
├── bench-compare.py               # Intra-run decomposition or cross-run diff
│                                  # with Welch's t-test + markdown report
└── spx-ui.sh                      # Restore captured SPX profiles into the
                                   # container and print the UI URL
```

One entry point per phase-4/5 deliverable. The runner writes per-run
output to `/tmp/patina-bench/<ts>/` by default; `make bench-baseline
NAME=<name>` redirects it to `fixtures/baselines/<name>/` so results
can be committed.

### `fixtures/baselines/` — Committed HTTP-bench baselines

```
fixtures/baselines/
└── phase6-initial/                # First baseline (2026-04-13)
    ├── manifest.json              #   Run metadata (SHA, host, versions)
    ├── report.md                  #   bench-compare markdown output
    └── <config>/summary.json      #   Aggregated stats + raw samples per config
```

Each baseline directory is a committable slice of a full `bench-full`
run — just the per-config `summary.json`s (which contain raw samples),
plus the manifest and the rendered report. Raw k6 NDJSON output stays
under `/tmp/patina-bench/` because it's an order of magnitude larger
without adding analytical value.

### `.github/workflows/` — CI/CD

```
.github/workflows/
├── ci.yml                         # Runs on every push/PR
│                                  #   - cargo test, clippy, fmt
│                                  #   - Build for all PHP versions (x86_64 only)
│                                  #   - PHP extension tests
│                                  #   - WordPress core test suite
├── build-all.yml                  # Runs on main push
│                                  #   - Full 8-artifact build matrix
│                                  #   - Cross-compilation for aarch64
├── benchmark.yml                  # Runs on main push
│                                  #   - Criterion benchmarks with regression detection
├── fuzz.yml                       # Nightly cron
│                                  #   - 5 min per fuzz target
└── release.yml                    # Triggered by tag push (v*)
                                   #   - Full build + test
                                   #   - GitHub Release with artifacts + checksums
```

Split by trigger and purpose. `ci.yml` runs on every PR and must be fast (skip aarch64 cross-compilation). `build-all.yml` on main does the full matrix. `release.yml` only on tags.

---

## Workspace Cargo Configuration

```toml
# Cargo.toml (workspace root)
[workspace]
resolver = "2"
members = [
    "crates/patina-core",
    "crates/patina-ext",
    "crates/patina-bench",
    # patina-fuzz is NOT a workspace member — cargo-fuzz manages its own workspace
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[profile.release]
lto = "fat"
codegen-units = 1
strip = "symbols"
opt-level = 3
# panic = "unwind" (default) — required for catch_unwind
```

Note: `patina-fuzz` declares its own `[workspace]` because `cargo-fuzz` requires it. It's excluded from the main workspace to avoid conflicts.

---

## Naming Conventions

| Thing | Convention | Example |
|---|---|---|
| Rust module (patina-core) | Snake case, WordPress function name when 1:1 | `sanitize_redirect.rs` |
| Rust module (group) | Snake case, descriptive noun | `escaping/`, `kses/`, `formatting/` |
| Rust public function | Snake case, matching WordPress name | `pub fn esc_html(text: &str) -> String` |
| PHP function (pluggable) | Original WordPress name | `wp_sanitize_redirect()` |
| PHP function (non-pluggable) | `patina_` prefix + descriptive | `patina_esc_html()` |
| PHP test class | PascalCase, mirrors module path | `Escaping/EscHtmlTest.php` |
| Fixture file | WordPress function name + `.json` | `esc_html.json` |
| Criterion bench | Module group name | `benches/escaping.rs` |
| Fuzz target | WordPress function name | `fuzz_targets/esc_html.rs` |
| Docker image | `patina-build-{php_version}` | `patina-build-8.3` |
| CI artifact | `patina-php{ver}-linux-{arch}.so` | `patina-php8.3-linux-x86_64.so` |

---

## Adding a New Function: Where Everything Goes

Example: adding `esc_url`.

| Step | File(s) | Action |
|---|---|---|
| 1. Generate fixtures | `php/fixture-generator/functions/esc_url.php` | Define inputs and call pattern |
| | `fixtures/esc_url.json` | Generated output |
| 2. Implement logic | `crates/patina-core/src/escaping/url.rs` | Pure Rust implementation |
| | `crates/patina-core/src/escaping/mod.rs` | Add `pub mod url;` and re-export |
| 3. Add tests | Bottom of `url.rs` (`#[cfg(test)]` block) | Fixture-based + hand-written edge case tests |
| 4. Register PHP function | `crates/patina-ext/src/lib.rs` | Add `#[php_function]` + `wrap_function!()` in `get_module` |
| 5. Add fuzz target | `crates/patina-fuzz/fuzz_targets/esc_url.rs` | Fuzz with invariant checks |
| 6. Add benchmark | `crates/patina-bench/benches/escaping.rs` | Add bench group for `esc_url` |
| 7. Add PHP test | `php/tests/Escaping/EscUrlTest.php` | Fixture comparison + fuzz |
| 8. Add PHP benchmark | `php/benchmarks/suites/escaping.php` | Add `esc_url` comparison |
| | `php/benchmarks/reference/escaping.php` | Copy original WP `esc_url()` |
| 9. Wire bridge (non-pluggable) | `php/bridge/patina-bridge.php` | Add interception hook |

Every step has a predictable location. No searching required.

---

## Dependency Flow

```
                        ┌──────────────┐
                        │  patina-core │  ← Pure Rust. No PHP. No FFI.
                        │  (lib crate) │     Testable, benchmarkable, fuzzable.
                        └──────┬───────┘
                               │
               ┌───────────────┼───────────────┐
               │               │               │
      ┌────────▼───────┐  ┌───▼────────┐  ┌───▼────────┐
      │  patina-ext    │  │ patina-    │  │ patina-    │
      │  (cdylib)      │  │ bench      │  │ fuzz       │
      │                │  │            │  │            │
      │  ext-php-rs    │  │ criterion  │  │ libfuzzer  │
      │  catch_unwind  │  │            │  │            │
      └────────┬───────┘  └────────────┘  └────────────┘
               │
               │  loads as
               ▼
      ┌─────────────────┐
      │  PHP (runtime)   │
      │                  │
      │  patina-bridge   │ ← Only needed for non-pluggable functions
      │  (mu-plugin)     │
      └─────────────────┘
               │
               ▼
      ┌─────────────────┐
      │  WordPress       │
      └─────────────────┘
```

Key property: `patina-core` has **zero awareness** of PHP. It takes Rust strings, returns Rust strings (or Rust structs for blocks). All PHP-specific concerns live in `patina-ext`.

---

## What Goes Where: Decision Guide

| Question | Answer |
|---|---|
| "Where does the algorithm go?" | `patina-core/src/<module>/<function>.rs` |
| "Where does the PHP glue go?" | `patina-ext/src/<module>.rs` |
| "Where does the shared utility go?" | `patina-core/src/util/` |
| "Where does the test data go?" | `fixtures/<function>.json` |
| "Where does the bench data go?" | `crates/patina-bench/data/` |
| "Where does a Docker config go?" | `docker/` for build/test, `profiling/` for profiling |
| "Where does CI config go?" | `.github/workflows/` |
| "Where does the WordPress integration go?" | `php/bridge/patina-bridge.php` |
| "Is this Rust code or PHP code?" | If it can be tested without PHP → Rust (`patina-core`). If it requires PHP runtime → PHP or `patina-ext`. |
