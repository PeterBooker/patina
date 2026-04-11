# Patina: Detailed Implementation Plan

## Overview

Patina is a Rust PHP extension that replaces WordPress core functions with optimized native implementations. It targets PHP 8.1–8.4 on Linux (x86_64 and aarch64).

### Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| **Interception strategy** | Pluggable functions only (initial release) | Can register directly via the extension; non-pluggable strategy TBD |
| **Project name** | `patina` | Extension loads as `patina` in PHP; replaced functions keep their original WordPress names |
| **Block parser return** | Native PHP arrays (ZendHashTable) | Avoids JSON serialize/deserialize round-trip |
| **PHP versions** | 8.1, 8.2, 8.3, 8.4 | 8 build artifacts (× x86_64, aarch64) |
| **Error handling** | `catch_unwind` in all builds | Prevents panics from segfaulting PHP-FPM workers |
| **Panic mode** | `panic = "unwind"` (NOT `"abort"`) | Required for `catch_unwind` to work — see Technical Note below |
| **JIT benchmarks** | Yes, with and without | Quantify interaction between JIT and native replacements |
| **Shortcode parser** | Out of scope | Requires executing PHP callbacks per shortcode |
| **`sanitize_text_field`** | Out of scope | |
| **Observability/metrics** | Out of scope (initial release) | Avoids atomic counter overhead on hot paths |
| **Admin plugin UI** | Out of scope | mu-plugin only (needed later for non-pluggable functions) |
| **`cargo-deny`** | Out of scope (initial release) | |

### Critical Technical Note: `panic = "unwind"` vs `catch_unwind`

The original plan specified `panic = "abort"` in the release profile. This is **incompatible** with `catch_unwind` — when panic mode is `abort`, the process terminates immediately on any panic, and `catch_unwind` becomes a no-op.

Since the project decision is to use `catch_unwind` as a safety net in all builds (including release), we **must** use `panic = "unwind"` (the Rust default). This has a small code size cost (~5-10%) due to unwinding tables, but it's the only way to prevent a missed `unwrap()` or out-of-bounds access from killing an entire PHP-FPM worker pool.

### Scope Reality Check

**The pluggable-only constraint limits initial targets to ~5 functions, most of which are low-frequency.** WordPress's `pluggable.php` contains mostly auth, email, and session management functions — NOT the string processing hotspots identified in the original plan.

The high-value targets (`esc_html` at 500+ calls/request, `wp_kses` at 8-15% wall time, `wpautop`, etc.) are all defined unconditionally in `formatting.php` / `kses.php` with no `function_exists()` guards. Replacing them requires solving the non-pluggable interception strategy (Phase 8).

**Phase 0-7 are primarily a proof of concept** that validates the full pipeline (build, test, CI, benchmark). The real performance wins come in Phases 8-9.

### Viable Pluggable Function Candidates

| Function | Source | Profile | Notes |
|---|---|---|---|
| `wp_sanitize_redirect()` | `pluggable.php` | Regex-heavy URL sanitization | Pure string processing, no WP state. Best initial candidate. |
| `wp_validate_redirect()` | `pluggable.php` | URL validation | Calls `apply_filters` — tests Rust→PHP callback path. |
| `wp_parse_auth_cookie()` | `pluggable.php` | Simple string splitting | Accesses `$_COOKIE` superglobal + WP constants. Low value. |
| `wp_check_filetype_and_ext()` | `pluggable.php` | MIME/extension matching | May call `finfo` (C-level already). Profile first. |
| `wp_text_diff()` | `pluggable.php` | Text diffing | Rarely called in request path. Low priority. |

Functions like `wp_hash()`, `wp_check_password()`, `wp_generate_password()` are pluggable but already delegate to C-level PHP functions (`hash_hmac`, bcrypt). Rust provides no meaningful speedup.

---

## Crate Architecture

The pure Rust logic must be separated from the PHP extension wrapper so that benchmarks and fuzz targets can depend on it (a `cdylib` crate cannot be a dependency of other crates).

```
crates/
├── patina-core/            # Pure Rust logic (standard lib crate)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── compat.rs       # PHP behavioral compatibility helpers
│       ├── pluggable/      # Pluggable function replacements
│       │   ├── mod.rs
│       │   ├── sanitize_redirect.rs
│       │   └── validate_redirect.rs
│       ├── escapers/       # (Future: non-pluggable functions)
│       ├── sanitizers/     # (Future)
│       ├── parsers/        # (Future)
│       └── text/           # (Future)
│
├── patina-ext/             # PHP extension wrapper (cdylib)
│   ├── Cargo.toml          # Depends on patina-core + ext-php-rs
│   └── src/
│       ├── lib.rs          # #[php_function] wrappers, #[php_module]
│       └── panic_guard.rs  # catch_unwind wrapper
│
├── patina-bench/           # Criterion benchmarks
│   ├── Cargo.toml          # Depends on patina-core
│   └── benches/
│       └── sanitize_redirect.rs
│
└── patina-fuzz/            # Fuzz targets (cargo-fuzz)
    ├── Cargo.toml          # Depends on patina-core
    └── fuzz_targets/
        └── sanitize_redirect.rs
```

**Why this split matters:**
- `patina-core` is a normal `lib` crate — can be depended on by anything
- `patina-ext` is a `cdylib` — cannot be a dependency of other crates
- Benchmarks (`criterion`) and fuzz targets (`libfuzzer-sys`) both need to call the pure Rust functions directly
- The PHP wrapper layer in `patina-ext` is intentionally thin: just `#[php_function]` annotations, argument conversion, and `catch_unwind`

---

## Phase 0: Project Scaffold

**Goal:** Working Rust workspace that compiles to a `.so`, loads in PHP, and registers a test function.

**Duration:** ~2 days

**Depends on:** Nothing

### Step 0.1: Replace `.gitignore`

Replace the Go-oriented `.gitignore` with:

```gitignore
# Rust
/target/

# PHP
/vendor/
*.phpunit.result.cache

# Build artifacts
/dist/
*.dylib
*.dll

# Docker
docker-compose.override.yml

# Profiling output
/profiling/output/
*.cachegrind
*.spx

# Editor/IDE
.idea/
.vscode/
*.swp
*.swo
*~

# Environment
.env
.env.*
!.env.example

# OS
.DS_Store
Thumbs.db
```

Note: `Cargo.lock` is NOT ignored — it must be committed for reproducible builds of the extension binary. `*.so` is also not ignored since we don't want to accidentally exclude the build output discussion, but the `/dist/` directory (where release artifacts go) is ignored.

### Step 0.2: Initialize Cargo workspace

**File: `Cargo.toml` (workspace root)**

```toml
[workspace]
resolver = "2"
members = [
    "crates/patina-core",
    "crates/patina-ext",
    "crates/patina-bench",
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
# IMPORTANT: Do NOT set panic = "abort" — catch_unwind requires unwinding
# panic = "unwind" is the default and we rely on it
```

**File: `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

No cross-compilation targets pinned here — those are handled in CI via `--target` flags.

**File: `crates/patina-core/Cargo.toml`**

```toml
[package]
name = "patina-core"
version.workspace = true
edition.workspace = true

[dependencies]
# Added incrementally as functions are implemented:
# aho-corasick = "1"     # Multi-pattern matching (kses)
# regex = "1"             # DFA regex
# memchr = "2"            # SIMD byte searching
# url = "2"               # URL parsing (esc_url)

[dev-dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**File: `crates/patina-ext/Cargo.toml`**

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

Note on `[lib] name`: This produces `libpatina.so` on Linux. PHP expects `patina.so` (no `lib` prefix). The build/install process must rename or symlink. Verify whether `ext-php-rs` handles this automatically — if not, add a post-build step.

**File: `crates/patina-bench/Cargo.toml`**

```toml
[package]
name = "patina-bench"
version.workspace = true
edition.workspace = true

[[bench]]
name = "pluggable"
harness = false

[dependencies]
patina-core = { path = "../patina-core" }
criterion = "0.5"
```

### Step 0.3: Hello world extension

**File: `crates/patina-core/src/lib.rs`**

```rust
pub mod compat;
pub mod pluggable;
```

**File: `crates/patina-core/src/compat.rs`**

```rust
// PHP behavioral compatibility helpers
// (populated as functions are implemented)
```

**File: `crates/patina-core/src/pluggable/mod.rs`**

```rust
// Pluggable function replacements
// (modules added as functions are implemented)
```

**File: `crates/patina-ext/src/panic_guard.rs`**

```rust
use ext_php_rs::prelude::*;

/// Wraps a closure in catch_unwind, converting panics to PHP exceptions.
///
/// Every #[php_function] must route through this. A panic in a PHP extension
/// unwinds through C frames (undefined behavior) or aborts the process.
/// This converts panics to catchable PHP exceptions instead.
pub fn guarded<F, T>(func_name: &str, f: F) -> PhpResult<T>
where
    F: FnOnce() -> T + std::panic::UnwindSafe,
{
    std::panic::catch_unwind(f).map_err(|_| {
        PhpException::default(format!("patina: internal error in {func_name}").into())
    })
}
```

**File: `crates/patina-ext/src/lib.rs`**

```rust
use ext_php_rs::prelude::*;

mod panic_guard;

/// Returns the extension version. Used for health checks.
#[php_function]
pub fn patina_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Returns true. Simplest possible smoke test.
#[php_function]
pub fn patina_loaded() -> bool {
    true
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
}
```

### Step 0.4: Local build verification

```bash
cd crates/patina-ext
cargo build --release
```

Output: `target/release/libpatina.so`

```bash
# Verify it loads (may need to adjust the path based on your PHP setup)
php -d extension=./target/release/libpatina.so -r "var_dump(patina_loaded());"
# Expected: bool(true)

php -d extension=./target/release/libpatina.so -r "echo patina_version();"
# Expected: 0.1.0

php -d extension=./target/release/libpatina.so -m | grep patina
# Expected: patina
```

If `libpatina.so` doesn't load because PHP expects `patina.so`, rename it:
```bash
cp target/release/libpatina.so target/release/patina.so
php -d extension=./target/release/patina.so -r "echo patina_version();"
```

### Step 0.5: Docker build environment

**File: `docker/Dockerfile.build`**

```dockerfile
ARG PHP_VERSION=8.3
FROM php:${PHP_VERSION}-cli AS builder

RUN apt-get update && apt-get install -y \
    curl build-essential pkg-config libclang-dev

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /src
COPY . .

RUN cd crates/patina-ext && cargo build --release

# Verify it loads
RUN php -d extension=/src/target/release/libpatina.so -r "var_dump(patina_loaded());" \
    || php -d extension=/src/target/release/patina.so -r "var_dump(patina_loaded());"

# Output stage — extract just the .so
FROM scratch AS output
COPY --from=builder /src/target/release/libpatina.so /patina.so
```

### Step 0.6: Verify across PHP versions

Build and test against all 4 PHP versions:

```bash
for v in 8.1 8.2 8.3 8.4; do
    echo "=== PHP $v ==="
    docker build --build-arg PHP_VERSION=$v \
        -f docker/Dockerfile.build --target builder \
        -t patina-build-$v .
    docker run --rm patina-build-$v \
        php -d extension=/src/target/release/libpatina.so \
        -r "echo 'PHP ' . PHP_VERSION . ': ' . patina_version() . PHP_EOL;"
done
```

**Acceptance criteria:**
- Extension compiles against PHP 8.1, 8.2, 8.3, 8.4
- `patina_version()` returns `0.1.0` on all versions
- `patina_loaded()` returns `true` on all versions
- No warnings or errors on load

**Potential blocker:** `ext-php-rs 0.13` may not support PHP 8.4 yet. If it doesn't:
- Check for a newer version or pre-release branch
- Check the ext-php-rs issue tracker for PHP 8.4 support status
- Temporarily drop PHP 8.4 from the matrix and track the issue

---

## Phase 1: Profiling Infrastructure

**Goal:** Docker-based WordPress environment with SPX profiling, k6 load testing, and a procedure for identifying function hotspots. Produces a candidate report that confirms which functions are worth replacing.

**Duration:** ~1 week

**Depends on:** Nothing (can run in parallel with Phase 0)

### Step 1.1: Docker Compose profiling stack

**File: `profiling/docker-compose.yml`**

Services:
1. **nginx** — Reverse proxy to PHP-FPM, port 8080
2. **php-fpm** — PHP 8.3-FPM with SPX profiler and Xdebug installed
3. **mariadb** — MariaDB 11, database `wordpress`

**File: `profiling/Dockerfile.profiling`**

Based on `php:8.3-fpm`. Installs:
- WordPress PHP extensions: `gd`, `mysqli`, `zip`, `intl`, `mbstring`, `opcache`
- SPX profiler (build from source, v0.4.17+)
- Xdebug (PECL install, configured for profiling mode only — NOT debug mode)
- WP-CLI

SPX configuration:
```ini
extension=spx.so
spx.http_enabled=1
spx.http_key=dev
spx.http_ip_whitelist=*
```

Xdebug configuration (trigger-only profiling, zero overhead when not triggered):
```ini
xdebug.mode=profile
xdebug.start_with_request=trigger
xdebug.output_dir=/tmp/xdebug-profiles
xdebug.profiler_output_name=cachegrind.out.%R.%t
```

OPcache configuration (realistic production settings):
```ini
opcache.enable=1
opcache.memory_consumption=256
opcache.max_accelerated_files=20000
opcache.validate_timestamps=0
```

**File: `profiling/nginx.conf`**

Standard WordPress Nginx config: `try_files $uri $uri/ /index.php?$args;` with `fastcgi_pass php-fpm:9000`.

### Step 1.2: WordPress setup script

**File: `profiling/setup-wordpress.sh`**

Automated script that:
1. Waits for MariaDB to be healthy
2. Downloads WordPress via WP-CLI
3. Installs WordPress (`wp core install`)
4. Sets permalink structure to `/%postname%/`
5. Installs and activates the WordPress Importer plugin
6. Downloads and imports the WordPress Theme Unit Test Data (~600 posts of varied content types)
7. Optionally imports Gutenberg stress-test content (block-heavy pages with 50+ nested blocks)
8. Flushes rewrite rules
9. Warms OPcache with 3 requests to the homepage
10. Prints the SPX UI URL

### Step 1.3: k6 workload scripts

**File: `profiling/k6-workloads.js`**

Defines the standard workload set:

| Workload | URL Pattern | What it exercises |
|---|---|---|
| Homepage | `/` | `the_content`, template rendering, escaping |
| Single post (block content) | `/[discovered-post]/` | Block parser, `wpautop`, content filters |
| Category archive | `/category/uncategorized/` | Loop rendering, `esc_html`, `esc_url`, `esc_attr` in templates |
| Search | `/?s=lorem` | `sanitize_text_field`, search escaping |
| REST API | `/wp-json/wp/v2/posts` | `wp_kses_post`, REST field serialization |
| Admin dashboard | `/wp-admin/` | `esc_html`, `esc_attr`, menu rendering (requires auth cookie) |

Script auto-discovers post URLs from the REST API to avoid hardcoding slugs.

Two scenarios:
- **Profiling mode**: Low concurrency (1 VU), slow pace, SPX headers enabled
- **Load test mode**: 10 VUs, 60 seconds, measuring throughput and latency percentiles

### Step 1.4: JIT baseline profiling

Run the profiling suite twice:
1. With JIT disabled (`opcache.jit=0`)
2. With JIT enabled (`opcache.jit_buffer_size=128M`, `opcache.jit=1255`)

Compare function timings. Document which functions see significant JIT acceleration — these may be less valuable as Rust targets since JIT already speeds them up.

### Step 1.5: Profiling procedure

Documented steps:

```bash
# 1. Start the stack
cd profiling
docker compose up -d
./setup-wordpress.sh

# 2. SPX profiling (per-request flame graphs)
# Open browser: http://localhost:8080/?SPX_UI_URI=/&SPX_KEY=dev
# Navigate to pages — SPX records each request automatically

# 3. Xdebug cachegrind (for KCacheGrind/QCacheGrind analysis)
curl -b "XDEBUG_PROFILE=1" http://localhost:8080/sample-post/
# Files appear in profiling/output/

# 4. k6 aggregate load test
k6 run k6-workloads.js --out json=output/k6-results.json

# 5. Analyze
# Open cachegrind files in KCacheGrind/QCacheGrind
# Review SPX flame graphs in browser
# Produce the candidate report (Step 1.6)
```

### Step 1.6: Candidate report

**Deliverable:** A document (`profiling/CANDIDATE_REPORT.md`) that lists:

For every function consuming >0.5% of wall time OR called >50 times per request:
- Function name
- Cumulative wall time %
- Self time %
- Call count per request (averaged across workloads)
- Whether it's pluggable (`function_exists` guard in source)
- API surface (string→string, requires callbacks, requires complex objects)
- Score using the candidate scoring matrix (Section 3.5 of original plan)
- Recommendation: **implement now** (pluggable) / **implement after strategy** (non-pluggable, high value) / **skip** (low value or infeasible)

**Expected findings:**
- Pluggable functions will show minimal contribution to total wall time
- `esc_html`, `esc_attr`, `wp_kses`, `wpautop` will dominate the non-pluggable candidate list
- The report validates that Phase 8 (non-pluggable strategy) is the critical path for real performance impact

---

## Phase 2: Core Extension Architecture

**Goal:** Establish the module structure, error handling patterns, and foundational utilities that all function implementations will use.

**Duration:** ~3 days

**Depends on:** Phase 0

### Step 2.1: Panic guard pattern

Already scaffolded in Phase 0.5 (`panic_guard.rs`). Every `#[php_function]` in `patina-ext` MUST use this:

```rust
#[php_function]
pub fn wp_sanitize_redirect(location: &str) -> PhpResult<String> {
    panic_guard::guarded("wp_sanitize_redirect", || {
        patina_core::pluggable::sanitize_redirect::sanitize_redirect(location)
    })
}
```

No exceptions. Enforced by code review and a clippy lint or grep-based CI check that every `#[php_function]` contains a `guarded(` call.

### Step 2.2: PHP callback infrastructure

For pluggable functions that need to call back into WordPress (e.g., `apply_filters`), establish the calling convention:

**File: `crates/patina-ext/src/php_callback.rs`**

```rust
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

/// Call a PHP function by name. Used for calling apply_filters, etc.
/// Returns the result Zval or a PhpException.
pub fn call_php_func(name: &str, args: &mut [&mut Zval]) -> PhpResult<Zval> {
    let mut func = Zval::new();
    func.set_string(name, false)
        .map_err(|e| PhpException::default(format!("patina: failed to set function name '{name}': {e}").into()))?;
    func.try_call(args)
        .map_err(|e| PhpException::default(format!("patina: call to '{name}' failed: {e}").into()))
}
```

**Testing requirement:** Validate that calling `apply_filters` from Rust works correctly. This is a re-entrant call (PHP → Rust → PHP → possibly Rust again). Create a test that:
1. Registers a PHP filter callback via `add_filter()`
2. Calls the Rust function that internally calls `apply_filters`
3. Verifies the filter callback executed and the result is correct

This is a risk area — if re-entrant calls don't work cleanly in ext-php-rs, the approach needs revision. Test early.

### Step 2.3: String handling conventions

Document and enforce these rules (add to `ARCHITECTURE.md` in Phase 10, but establish now in code):

1. **Input**: `&str` for functions that only accept valid strings. This is safe because PHP strings passed to `#[php_function(name = &str)]` are validated by ext-php-rs.
2. **Binary input**: `&[u8]` for functions that must handle arbitrary byte sequences (e.g., if WordPress passes non-UTF-8 data). Use sparingly — most WordPress functions assume UTF-8.
3. **Output**: `String` (owned). ext-php-rs converts to `zend_string` — one allocation + memcpy. Acceptable cost.
4. **Never** accept `Vec<Zval>`, `HashMap`, or complex objects across the boundary. Use `&ZendHashTable` and index on demand if array access is needed (future, for non-pluggable functions).

### Step 2.4: Function registration pattern

When the extension registers a function with the same name as a WordPress pluggable function (e.g., `wp_sanitize_redirect`), the registration happens at PHP module init (MINIT) — before any PHP script executes. When WordPress loads `pluggable.php` and checks `function_exists('wp_sanitize_redirect')`, it returns `true`, and WordPress's PHP definition is skipped.

This means:
- **No mu-plugin is needed** for pluggable function replacement
- The function signature (parameters, defaults, return type) must match WordPress's exactly
- If the extension is not loaded, WordPress defines its own version — graceful degradation

**Parameter matching requirement:** WordPress's `wp_sanitize_redirect` has this signature:
```php
function wp_sanitize_redirect( $location ) { ... }
```

The Rust registration must match:
```rust
#[php_function]
pub fn wp_sanitize_redirect(location: &str) -> PhpResult<String> { ... }
```

If WordPress has optional parameters with defaults, the Rust function must declare them with `#[php_function]` default attributes. Verify ext-php-rs syntax for optional parameters.

---

## Phase 3: Testing Infrastructure

**Goal:** Three-layer test framework (Rust unit tests, PHP extension tests, WordPress full suite) plus fuzz testing.

**Duration:** ~1 week

**Depends on:** Phase 0

### Step 3.1: PHP fixture generator

**File: `php/fixture-generator/generate-fixtures.php`**

A PHP script that:
1. Loads WordPress (requires a running WordPress installation — use the profiling Docker stack or test Docker stack)
2. For a given function name, runs it against a comprehensive corpus of inputs
3. Outputs JSON: `[{"input": [...args], "output": <result>}, ...]`

**Corpus composition per function:**
- Empty string
- Single character (each of: `<`, `>`, `&`, `"`, `'`, ` `, `\0`, `\n`, `\t`)
- ASCII-only text (various lengths: 10, 100, 1000, 10000 chars)
- Multibyte UTF-8: CJK characters, emoji, combining characters, RTL text
- HTML entities: `&amp;`, `&#039;`, `&lt;`, `&#x41;`, invalid entities
- HTML tags: `<script>`, `<div class="foo">`, nested tags, unclosed tags
- Control characters and null bytes
- URL-specific: protocols, ports, query strings, fragments, IDN domains
- Inputs extracted from the WordPress Theme Unit Test content (real-world data)
- Edge cases from WordPress's own PHPUnit tests for the function (extract from `tests/phpunit/tests/`)
- 1MB+ strings (stress test)
- Random byte sequences that happen to be valid UTF-8

**Usage:**
```bash
# From the profiling or test Docker stack:
docker compose exec php-fpm php /app/php/fixture-generator/generate-fixtures.php \
    --function=wp_sanitize_redirect \
    --output=/app/crates/patina-core/tests/fixtures/wp_sanitize_redirect.json
```

**Output directory:** `crates/patina-core/tests/fixtures/`

These fixtures are committed to the repository. They are regenerated when:
- A new WordPress version changes function behavior
- New edge cases are discovered
- The corpus is expanded

### Step 3.2: Rust fixture-based tests

Each function module in `patina-core` includes tests that load fixtures:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Fixture {
        input: Vec<String>,
        output: String,
    }

    #[test]
    fn test_against_wordpress_fixtures() {
        let data = include_str!("../../tests/fixtures/wp_sanitize_redirect.json");
        let fixtures: Vec<Fixture> = serde_json::from_str(data).unwrap();
        assert!(!fixtures.is_empty(), "No fixtures loaded");

        for (i, f) in fixtures.iter().enumerate() {
            let result = sanitize_redirect(&f.input[0]);
            assert_eq!(
                result, f.output,
                "Fixture {i} failed.\n  Input:    {:?}\n  Expected: {:?}\n  Got:      {:?}",
                &f.input[0], &f.output, &result
            );
        }
    }
}
```

Run: `cargo test -p patina-core`

### Step 3.3: PHPUnit test environment

**File: `docker/docker-compose.test.yml`**

Services:
- **php-cli**: PHP CLI with the patina extension installed + PHPUnit
- **mariadb**: Test database

**File: `php/composer.json`**

```json
{
    "require-dev": {
        "phpunit/phpunit": "^10.0",
        "yoast/phpunit-polyfills": "^2.0"
    }
}
```

**File: `php/phpunit.xml`**

```xml
<phpunit bootstrap="tests/bootstrap.php">
    <testsuites>
        <testsuite name="patina">
            <directory>tests</directory>
        </testsuite>
    </testsuites>
</phpunit>
```

**File: `php/tests/bootstrap.php`**

```php
<?php
if (!extension_loaded('patina')) {
    die("FATAL: patina extension not loaded. Cannot run tests.\n");
}

// For tests that need WordPress:
$wp_tests_dir = getenv('WP_TESTS_DIR') ?: '/tmp/wordpress-tests-lib';
if (file_exists($wp_tests_dir . '/includes/bootstrap.php')) {
    require_once $wp_tests_dir . '/includes/bootstrap.php';
}
```

### Step 3.4: PHP extension test pattern

**Important subtlety:** Since the extension replaces the pluggable function, we can't compare "core output vs Rust output" at runtime — the core version doesn't exist when the extension is loaded. Instead, we compare against **pre-generated fixtures** (Step 3.1) that were produced from WordPress WITHOUT the extension.

```php
class PatinaSanitizeRedirectTest extends WP_UnitTestCase {

    /**
     * @dataProvider fixtureProvider
     */
    public function test_matches_wordpress_output(string $input, string $expected): void {
        // wp_sanitize_redirect() is now the Rust version (pluggable replacement)
        $this->assertSame($expected, wp_sanitize_redirect($input));
    }

    public static function fixtureProvider(): array {
        $fixtures = json_decode(
            file_get_contents(__DIR__ . '/fixtures/wp_sanitize_redirect.json'),
            true
        );
        $cases = [];
        foreach ($fixtures as $i => $f) {
            $cases["fixture_{$i}"] = [$f['input'][0], $f['output']];
        }
        return $cases;
    }

    public function test_fuzz_no_crash(): void {
        for ($i = 0; $i < 10000; $i++) {
            $input = random_bytes(random_int(0, 5000));
            $result = wp_sanitize_redirect($input);
            $this->assertIsString($result, "Non-string return for random input $i");
        }
    }

    public function test_empty_string(): void {
        $this->assertSame('', wp_sanitize_redirect(''));
    }

    public function test_stress_large_input(): void {
        $input = 'http://example.com/' . str_repeat('a', 1_000_000);
        $result = wp_sanitize_redirect($input);
        $this->assertIsString($result);
        $this->assertGreaterThan(0, strlen($result));
    }
}
```

### Step 3.5: WordPress core test suite integration

Run the FULL WordPress PHPUnit test suite with the extension loaded. Since we're replacing pluggable functions, all of WordPress's existing tests that call these functions will now exercise the Rust implementation.

**Process (automated in CI, runnable locally via Docker):**

1. Build the extension for the target PHP version
2. Install it in the PHP environment (`extension=patina.so` in php.ini)
3. Check out the WordPress develop repository (Git mirror: `https://github.com/WordPress/wordpress-develop.git`)
4. Configure `wp-tests-config.php` with test database credentials
5. Run: `vendor/bin/phpunit --testsuite default`
6. **Zero failures** = correctness validated

Specific test classes to watch (will directly exercise our replacements):
- `Tests_Functions_SanitizeRedirect` (if it exists)
- Any test that calls `wp_sanitize_redirect()` or `wp_validate_redirect()`
- Search the test suite: `grep -r "wp_sanitize_redirect\|wp_validate_redirect" tests/phpunit/`

### Step 3.6: Fuzz testing

**File: `crates/patina-fuzz/Cargo.toml`**

```toml
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

# cargo-fuzz requires this
[workspace]
members = ["."]
```

**File: `crates/patina-fuzz/fuzz_targets/sanitize_redirect.rs`**

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let result = patina_core::pluggable::sanitize_redirect::sanitize_redirect(s);
        // Invariants:
        // - Must return a string (can't panic)
        // - Result must not contain null bytes
        assert!(!result.contains('\0'));
    }
});
```

Run locally: `cd crates/patina-fuzz && cargo +nightly fuzz run sanitize_redirect -- -max_total_time=300`

Run in CI nightly (Phase 6): 5 minutes per target.

---

## Phase 4: First Function — `wp_sanitize_redirect`

**Goal:** End-to-end proof of concept. One pluggable function fully replaced, all test layers passing, benchmarked.

**Duration:** ~1 week

**Depends on:** Phases 0, 2, 3

### Step 4.1: Study WordPress implementation

Read `wp-includes/pluggable.php`, function `wp_sanitize_redirect()`. Document every operation in order:

1. **Replace spaces with `%20`**: `str_replace(' ', '%20', $location)`

2. **Percent-encode multibyte UTF-8 characters**: A regex matches valid multibyte UTF-8 byte sequences (2-byte through 4-byte) and passes them to `_wp_sanitize_utf8_in_redirect()`, which calls `urlencode()` on each matched group. This makes the URL ASCII-safe while preserving the encoded characters.

   The regex (from WordPress source):
   ```
   /(
       (?: [\xC2-\xDF][\x80-\xBF]
       |   \xE0[\xA0-\xBF][\x80-\xBF]
       |   [\xE1-\xEC][\x80-\xBF]{2}
       |   \xED[\x80-\x9F][\x80-\xBF]
       |   [\xEE-\xEF][\x80-\xBF]{2}
       |   \xF0[\x90-\xBF][\x80-\xBF]{2}
       |   [\xF1-\xF3][\x80-\xBF]{3}
       |   \xF4[\x80-\x8F][\x80-\xBF]{2}
   ){1,40}
   )/x
   ```

   In Rust: iterate over the string's chars. For each char that is multi-byte (>= U+0080), percent-encode its UTF-8 bytes. This is simpler than the regex approach — Rust knows UTF-8 char boundaries natively.

3. **Strip disallowed characters**: `preg_replace('|[^a-z0-9-~+_.?#=&;,/:%!*\[\]()@]|i', '', $location)` — remove any character NOT in the URL-safe allowlist.

   In Rust: iterate over bytes, retain only those matching the character class. Use a 256-byte lookup table for O(1) per-byte classification.

4. **Strip null bytes**: Calls `wp_kses_no_null($location)` which removes null bytes and optionally handles `\x00` sequences. Need to reimplement this inline.

   In Rust: `location.replace('\0', "")` plus handling of the `%00` and `\0` string patterns per `wp_kses_no_null`'s exact behavior.

5. **Prepend protocol if missing**: If the result doesn't start with a known protocol, prepend nothing (WordPress just returns the sanitized string as-is without a protocol check in newer versions — verify against current WordPress source).

**Edge cases to verify:**
- Empty string input → empty string output
- Already-encoded characters (`%20`, `%C3%A9`) → what happens? Double-encoding?
- Very long URLs (100KB+) → must not stack overflow
- All-multibyte input (e.g., full Chinese URL)
- Null bytes in various positions
- Input that's already a clean URL → should pass through unchanged

### Step 4.2: Generate fixtures

Run the fixture generator (Step 3.1) against WordPress WITHOUT the extension installed:

```bash
cd profiling
docker compose up -d
# Ensure WordPress is installed (setup-wordpress.sh)

docker compose exec php-fpm php /app/php/fixture-generator/generate-fixtures.php \
    --function=wp_sanitize_redirect \
    --output=/app/crates/patina-core/tests/fixtures/wp_sanitize_redirect.json
```

Manually inspect the fixtures for sanity. Add any edge cases discovered in Step 4.1 that the corpus didn't cover.

### Step 4.3: Implement in Rust

**File: `crates/patina-core/src/pluggable/sanitize_redirect.rs`**

Implementation approach:
- Single pass through the string where possible
- Use a `String` builder, pre-allocated to input length (output is typically similar size)
- Byte-level lookup table for the allowlist check (256-entry `[bool; 256]`)
- Native Rust char iteration for multibyte detection (no regex needed)
- Inline `wp_kses_no_null` behavior

```rust
/// Sanitizes a URL for use in a redirect.
///
/// Replaces the WordPress pluggable function `wp_sanitize_redirect()`.
/// Behavior is byte-identical to the PHP version.
pub fn sanitize_redirect(location: &str) -> String {
    // Step 1: Replace spaces with %20
    // Step 2: Percent-encode multibyte UTF-8 characters
    // Step 3: Strip disallowed characters
    // Step 4: Strip null bytes (wp_kses_no_null)
    // (Details of implementation)
    todo!()
}
```

Add `memchr = "2"` to `patina-core`'s dependencies if needed for fast byte scanning.

### Step 4.4: Register as PHP function

**File: `crates/patina-ext/src/lib.rs`** — add:

```rust
#[php_function]
pub fn wp_sanitize_redirect(location: &str) -> PhpResult<String> {
    panic_guard::guarded("wp_sanitize_redirect", || {
        patina_core::pluggable::sanitize_redirect::sanitize_redirect(location)
    })
}
```

When PHP loads the extension, `wp_sanitize_redirect` is registered. When WordPress's `pluggable.php` runs `if ( ! function_exists( 'wp_sanitize_redirect' ) )`, it evaluates to `false`, and WordPress's PHP version is skipped.

### Step 4.5: Run all test layers

1. **Rust unit tests**: `cargo test -p patina-core` — all fixture assertions pass
2. **PHP extension tests**: `cd php && vendor/bin/phpunit` — all fixture comparisons pass, fuzz test passes
3. **WordPress core test suite**: `vendor/bin/phpunit --testsuite default` — zero regressions
4. **Fuzz testing**: `cargo +nightly fuzz run sanitize_redirect -- -max_total_time=300` — zero crashes

### Step 4.6: Benchmark

**Criterion benchmark** (`crates/patina-bench/benches/pluggable.rs`):

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_sanitize_redirect(c: &mut Criterion) {
    let inputs = vec![
        ("simple_url", "http://example.com/page"),
        ("unicode_url", "http://example.com/日本語/ページ?q=テスト"),
        ("dirty_url", "http://example.com/<script>alert(1)</script>?foo=bar&baz=qux"),
        ("long_url", &format!("http://example.com/{}", "a".repeat(10000))),
        ("empty", ""),
    ];

    let mut group = c.benchmark_group("wp_sanitize_redirect");
    for (name, input) in &inputs {
        group.bench_with_input(
            BenchmarkId::new("rust", name),
            input,
            |b, i| b.iter(|| patina_core::pluggable::sanitize_redirect::sanitize_redirect(i)),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_sanitize_redirect);
criterion_main!(benches);
```

Run: `cargo bench -p patina-bench`

**PHP comparison benchmark** (`php/benchmarks/bench-sanitize-redirect.php`):

Since the Rust function replaces the pluggable one, we need a reference copy of the original PHP implementation for comparison. Copy WordPress's `wp_sanitize_redirect()` and its helper `_wp_sanitize_utf8_in_redirect()` into the benchmark script as `reference_wp_sanitize_redirect()`.

```php
// Reference: WordPress's original implementation (copied verbatim)
function reference_wp_sanitize_redirect($location) { /* ... WordPress source ... */ }
// Also need: reference__wp_sanitize_utf8_in_redirect()
// Also need: reference_wp_kses_no_null()

$bench = new PatinaBenchmark(iterations: 100_000);
$inputs = [
    'simple' => 'http://example.com/page',
    'unicode' => 'http://example.com/日本語/ページ',
    'dirty' => 'http://example.com/<script>alert(1)</script>?foo=bar',
    'long' => 'http://example.com/' . str_repeat('a', 10000),
];

foreach ($inputs as $name => $input) {
    $bench->run(
        "wp_sanitize_redirect ($name)",
        fn() => reference_wp_sanitize_redirect($input),
        fn() => wp_sanitize_redirect($input),  // Rust version via extension
    );
}
$bench->report();
```

**Acceptance criteria:**
- All tests green across all layers
- Criterion benchmark numbers recorded
- PHP benchmark shows speedup factor (even if modest)
- The entire pipeline works end-to-end

---

## Phase 5: Additional Pluggable Functions

**Goal:** Implement remaining worthwhile pluggable functions. Validate the PHP callback infrastructure.

**Duration:** ~1-2 weeks

**Depends on:** Phase 4

### Step 5.1: `wp_validate_redirect($location, $fallback_url = '')`

**Why this matters:** This function calls `apply_filters('allowed_redirect_hosts', ...)`, making it the first test of the Rust→PHP callback path (Step 2.2). If this works, it validates that future non-pluggable functions can call `apply_filters` from Rust.

**WordPress source analysis:**
1. Calls `wp_sanitize_redirect(trim($location, ...))` — our Rust version from Phase 4
2. Parses the URL with `parse_url()` — need to either reimplement or call PHP's `parse_url`
3. Compares host against `$_SERVER['HTTP_HOST']` and a list from `apply_filters('allowed_redirect_hosts', ...)`
4. Returns `$location` if valid, `$fallback_url` if not

**Implementation approach:**
- Implement URL host extraction in Rust (use the `url` crate, but handle WordPress-specific edge cases)
- Call back to PHP for `apply_filters` via `php_callback.rs`
- Handle the `$_SERVER` superglobal access from Rust (ext-php-rs can access PHP superglobals)

**Signature matching:**
```rust
#[php_function]
pub fn wp_validate_redirect(location: &str, fallback_url: Option<&str>) -> PhpResult<String> {
    // fallback_url defaults to '' in WordPress
    let fallback = fallback_url.unwrap_or("");
    panic_guard::guarded("wp_validate_redirect", || {
        // Implementation that calls back to PHP for apply_filters
        todo!()
    })
}
```

Verify ext-php-rs syntax for optional parameters with defaults. May need:
```rust
#[php_function(defaults(fallback_url = ""))]
pub fn wp_validate_redirect(location: &str, fallback_url: &str) -> PhpResult<String> { ... }
```

Follow the same test/fixture/benchmark cycle as Phase 4.

### Step 5.2: `wp_check_filetype_and_ext($file, $filename, $mimes = null)`

Only implement if profiling (Phase 1) shows it as a hotspot. Otherwise skip — it's rarely called in the request hot path.

### Step 5.3: Other pluggable functions

Implement any remaining pluggable functions that profiling identified as significant. Follow the Phase 4 pattern for each:

1. Study WordPress source
2. Generate fixtures
3. Implement in Rust
4. Register
5. Test (3 layers)
6. Fuzz
7. Benchmark

### Phase 5 exit criteria

- At least 2 pluggable functions fully replaced and tested
- PHP callback infrastructure (Rust→PHP `apply_filters`) validated
- Full WordPress test suite green with all replacements active
- Benchmark data collected

---

## Phase 6: CI/CD Pipeline

**Goal:** Automated build, test, and release pipeline for all target platforms and PHP versions.

**Duration:** ~1 week

**Depends on:** Phase 4 (needs a working extension to test CI against)

### Step 6.1: Build matrix

**File: `.github/workflows/build.yml`**

**Matrix:** PHP {8.1, 8.2, 8.3, 8.4} × Arch {x86_64, aarch64} = 8 artifacts

**x86_64 builds:** Native on `ubuntu-latest`, using the Docker build approach from Phase 0.5.

**aarch64 builds:** Two options:
- **Option A (preferred):** Use GitHub's ARM runners (`runs-on: ubuntu-24.04-arm`) if available
- **Option B:** Use Docker `buildx` with QEMU emulation on x86_64 runners (slower but always available)

```yaml
strategy:
  matrix:
    php: ["8.1", "8.2", "8.3", "8.4"]
    arch:
      - { runner: ubuntu-latest, target: x86_64-unknown-linux-gnu, label: x86_64 }
      - { runner: ubuntu-24.04-arm, target: aarch64-unknown-linux-gnu, label: aarch64 }
```

Each job:
1. Checkout code
2. Build inside `php:$VERSION-cli` container (or install PHP headers natively)
3. `cargo build --release`
4. Verify: `php -d extension=... -r "echo patina_version();"`
5. Upload artifact: `patina-php$VERSION-linux-$ARCH.so`
6. Log SHA256 checksum

### Step 6.2: Test matrix

**File: `.github/workflows/test.yml`**

**Job 1: Rust tests**
```yaml
- run: cargo test --workspace
- run: cargo clippy --workspace -- -D warnings
- run: cargo fmt --check
```

**Job 2: PHP extension tests** (per PHP version)
```yaml
strategy:
  matrix:
    php: ["8.1", "8.2", "8.3", "8.4"]
steps:
  - Build extension for this PHP version
  - Install extension
  - cd php && composer install
  - vendor/bin/phpunit
```

**Job 3: WordPress core test suite** (per PHP version × WP version)
```yaml
strategy:
  matrix:
    php: ["8.1", "8.2", "8.3", "8.4"]
    wp: ["6.6", "6.7", "latest"]
services:
  mariadb: ...
steps:
  - Build and install extension
  - Checkout wordpress-develop
  - Configure wp-tests-config.php
  - Run: vendor/bin/phpunit --testsuite default
  - Assert zero failures
```

**Job 4: Fuzz testing** (nightly only, via cron trigger)
```yaml
on:
  schedule:
    - cron: '0 3 * * *'  # 3am UTC daily
steps:
  - cargo +nightly fuzz run sanitize_redirect -- -max_total_time=300
  # Repeat for each fuzz target
```

### Step 6.3: Benchmark CI

**File: `.github/workflows/benchmark.yml`**

Runs on push to `main` only.

1. Rust benchmarks: `cargo bench -p patina-bench -- --output-format bencher`
2. Store results: `benchmark-action/github-action-benchmark@v1`
3. Alert on >20% regression
4. Publish benchmark history to GitHub Pages (optional)

### Step 6.4: Release automation

**File: `.github/workflows/release.yml`**

Triggered on Git tag push (`v*`):

1. Run the full build matrix (8 artifacts)
2. Run all tests (must pass)
3. Collect artifacts, rename to final names:
   - `patina-php8.1-linux-x86_64.so`
   - `patina-php8.1-linux-aarch64.so`
   - `patina-php8.2-linux-x86_64.so`
   - ... (8 total)
4. Generate `SHA256SUMS` file
5. Create GitHub Release with all artifacts, checksums, and changelog

---

## Phase 7: Benchmarking Suite

**Goal:** Comprehensive benchmarks at all three layers (Rust, PHP, full-stack), with and without JIT.

**Duration:** ~1 week

**Depends on:** Phases 4-5 (need implemented functions), Phase 1 (need profiling infrastructure)

### Step 7.1: Criterion benchmarks

One benchmark file per function in `crates/patina-bench/benches/`. Each tests multiple input sizes (tiny, medium, large, huge) with real-world content.

### Step 7.2: PHP extension benchmarks

**File: `php/benchmarks/bench-runner.php`**

A harness that:
1. Copies of the original WordPress PHP implementations as reference functions
2. Runs both the reference and the Rust version (via extension) on identical inputs
3. Reports per-function: input size, PHP time (ms), Rust time (ms), speedup multiplier
4. Outputs both human-readable table and JSON for CI consumption

### Step 7.3: Full-stack k6 benchmarks

Using the profiling Docker stack from Phase 1:

**Run 1:** Baseline (no extension)
**Run 2:** With extension
**Run 3:** Baseline + JIT
**Run 4:** With extension + JIT

For each run, measure:
- p50, p95, p99 latency per workload
- Requests per second
- Error rate

**Toggle mechanism:** The extension is either installed or not in the PHP-FPM container. Rebuild the container between runs. (No mu-plugin toggle needed for pluggable functions — if the extension is loaded, the functions are replaced.)

### Step 7.4: Performance report

Produce a summary with:
- Per-function speedup table (Rust vs PHP, with and without JIT)
- Full-stack latency impact (likely modest for pluggable functions only)
- JIT interaction analysis: does JIT make Rust less valuable for certain functions?
- Projection: estimated full-stack impact once non-pluggable functions are also replaced (using profiling data from Phase 1)
- Recommendation for Phase 8 priority

---

## Phase 8: Non-Pluggable Function Strategy Research

**Goal:** Decide and prototype the interception approach for non-pluggable WordPress functions (esc_html, wp_kses, wpautop, etc.).

**Duration:** ~1-2 weeks

**Depends on:** Nothing (can start anytime, but findings inform Phase 9)

**This is the critical gate for the project's real value.**

### Step 8.1: Evaluate approaches

Research each approach on: correctness, performance overhead, deployment complexity, PHP version compatibility, and maintenance burden.

| Approach | Mechanism | Pros | Cons |
|---|---|---|---|
| **uopz** | `uopz_set_return($func, $callback, true)` | Per-function replacement, clean API | Extra PECL dependency. May not be available on managed hosting. |
| **runkit7** | `runkit7_function_rename()` + new definition | Full replacement | Less maintained. PHP 8.4 support uncertain. |
| **Zend function table manipulation** | In extension's RINIT, rename original function and register Rust version under the original name | No extra PHP deps. Transparent. Used by APM extensions (Datadog, New Relic). | Complex C/Rust code touching Zend internals. May break across PHP minor versions. Not officially supported by ext-php-rs. |
| **`auto_prepend_file`** | A PHP file loaded before WordPress redefines functions via uopz/runkit | Can work without extension changes | Requires php.ini modification. Adds another moving part. |
| **Custom Zend extension** | Register as a Zend extension (not just PHP extension) to hook into function compilation/execution | Deepest integration. Can intercept at opcode level. | Significant complexity. ext-php-rs doesn't support Zend extensions — may need raw FFI. |

### Step 8.2: Prototype top 2 approaches

Build minimal prototypes:

**Prototype A: uopz-based**
- Install `uopz` in the test Docker stack
- Write a mu-plugin that uses `uopz_set_return('esc_html', fn($text) => patina_esc_html($text), true)`
- Measure overhead: how much time does the uopz dispatch layer add?
- Test: does the WordPress test suite still pass?

**Prototype B: Zend function table manipulation**
- In the extension's `RINIT` (request initialization), iterate the Zend function table
- Rename `esc_html` → `__wp_original_esc_html`
- Register the Rust version as `esc_html`
- In `RSHUTDOWN`, reverse the rename (or don't — it's per-request state)
- This requires raw FFI to Zend internals — ext-php-rs may not expose the function table

### Step 8.3: Decision

Select the approach based on prototyping results. Key criteria:
1. Does it work correctly across PHP 8.1-8.4?
2. What's the per-call overhead of the interception layer?
3. How complex is the deployment (extra PECL deps, php.ini changes)?
4. How maintainable is it across PHP version upgrades?

Update the project plan and proceed to Phase 9.

---

## Phase 9: High-Value Function Implementations

**Goal:** Implement the non-pluggable functions that deliver the real performance wins.

**Duration:** ~3-4 weeks

**Depends on:** Phase 8 (must have interception strategy decided)

### Implementation order (by estimated impact)

#### Tier 1 — Highest impact, implement first

**9.1: `esc_html($text)`**
- Called 100-500+ times per request
- Simple algorithm: encode `<`, `>`, `&`, `"`, `'` — BUT must NOT double-encode existing valid HTML entities (`&amp;` stays `&amp;`, not `&amp;amp;`)
- The entity-detection is the tricky part: must recognize `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#039;`, `&#xNN;`, `&#NNN;` patterns and skip them
- WordPress filter: `esc_html` filter on output — must still fire via the bridge
- Share implementation with `esc_attr` (identical algorithm, different filter hook)

**9.2: `esc_attr($text)`**
- Same implementation as `esc_html()` — same `_wp_specialchars()` call
- Different filter hook: `esc_attr` instead of `esc_html`
- Implement as: shared `specialchars()` in Rust, two thin wrappers with different PHP function names

**9.3: `wp_check_invalid_utf8($string, $strip = false)`**
- Called 200+ times per request (building block for other functions)
- Trivial in Rust: `std::str::from_utf8(bytes)` — if valid, return as-is; if invalid and `$strip`, remove invalid bytes; if invalid and `!$strip`, return empty string
- Must match WordPress's exact behavior for the `$strip` parameter and what "invalid" means in WordPress's context (uses `mb_check_encoding` or `preg_match('//u')`)

**9.4: `wp_kses_post($data)` / `wp_kses($content, $allowed_html, $allowed_protocols)`**
- 8-15% of wall time — the single highest-impact target
- Complex: full HTML tag parser, attribute allowlist, protocol validation, entity normalization
- `wp_kses_post` is the common case (always uses `$allowedposttags`)
- Strategy: cache the `$allowedposttags` spec at request init (mu-plugin calls `patina_kses_init(json_encode($allowedposttags))`)
- Use `aho-corasick` for multi-pattern tag matching
- This is the most complex implementation in the project — allocate 1-2 weeks

#### Tier 2 — Moderate impact

**9.5: `esc_url($url, $protocols = null, $_context = 'display')`**
- URL validation and sanitization
- Use the `url` crate for parsing, then apply WordPress-specific rules (protocol allowlist, entity encoding for display context)
- Filter hook: `clean_url`

**9.6: `wpautop($text, $br = true)`**
- Auto-paragraphing: converts double newlines to `<p>` tags, optionally converts single newlines to `<br>`
- Complex regex chain with 20+ years of edge case accumulation
- Pure string→string, no WordPress state
- No filter hook — requires direct replacement (Phase 8 strategy)
- Test fixtures are critical — the edge cases are numerous and subtle

**9.7: `sanitize_title($title, $fallback_title = '', $context = 'save')` / `sanitize_title_with_dashes($title, $raw_title, $context)`**
- Title→slug conversion
- Filter hook: `sanitize_title`
- Moderate complexity: accent removal, special character handling, dash conversion

#### Tier 3 — Lower impact

**9.8: `make_clickable($text)`**
- Auto-links URLs and email addresses in text
- Regex-heavy, moderate frequency
- No filter hook — direct replacement

**9.9: `sanitize_file_name($filename)`**
- File name sanitization
- Filter hook: `sanitize_file_name`
- Simple but useful

**9.10: `parse_blocks($content)` (Gutenberg block parser)**
- 3-8% of wall time on block-content pages
- Full PEG/recursive descent parser for the Gutenberg block grammar
- WordPress's PHP implementation is a state machine
- Must return native PHP arrays (`ZendHashTable`) per project decision — NOT JSON
- Complex: block comments, nested blocks, block attributes (JSON in HTML comments)
- The return structure is deeply nested: `[{blockName, attrs: {}, innerBlocks: [...], innerHTML, innerContent: [...]}, ...]`

### Implementation pattern (same for every function)

Each function follows this 10-step process:

1. **Read** the WordPress PHP source thoroughly. Document every operation and edge case.
2. **Search** WordPress's PHPUnit tests for the function. Note every test case and edge case tested.
3. **Generate fixtures** from WordPress (without extension) using the fixture generator.
4. **Implement** in `patina-core/src/` (pure Rust, no PHP deps).
5. **Rust unit tests** against fixtures: `cargo test -p patina-core`.
6. **Register** `#[php_function]` wrapper in `patina-ext/src/lib.rs` with `catch_unwind`.
7. **Hook** into PHP via the Phase 8 interception mechanism (mu-plugin or Zend manipulation).
8. **PHP extension tests** against fixtures.
9. **WordPress core test suite** — zero regressions.
10. **Benchmark** (Criterion + PHP harness).

### Bridge mu-plugin (now needed)

For non-pluggable functions, a mu-plugin is required to set up the interception mechanism. The exact implementation depends on Phase 8's decision.

**File: `php/mu-plugin/patina-bridge.php`**

```php
<?php
/**
 * Plugin Name: Patina Bridge
 * Description: Routes WordPress core function calls to Patina native implementations.
 * Version: 0.1.0
 */

// Bail if extension not loaded — WordPress works normally without it
if (!extension_loaded('patina')) {
    return;
}

// Kill switch: set PATINA_DISABLE constant or env var to bypass
if (getenv('PATINA_DISABLE') || (defined('PATINA_DISABLE') && PATINA_DISABLE)) {
    return;
}

// The interception mechanism (populated based on Phase 8 decision)
// Example with uopz:
//   uopz_set_return('esc_html', function($text) {
//       $result = patina_esc_html_raw($text);
//       return apply_filters('esc_html', $result, $text);
//   }, true);
//
// Note: the bridge must still call apply_filters on the result
// so that other plugins' filter hooks continue to work.
```

**Critical bridge pattern for filter-bearing functions:**

```php
// For esc_html (which has an 'esc_html' filter):
uopz_set_return('esc_html', function($text) {
    // Rust does the computation
    $safe_text = patina_esc_html_raw($text);
    // PHP still applies the filter chain (other plugins may hook this)
    return apply_filters('esc_html', $safe_text, $text);
}, true);
```

The Rust extension registers `patina_esc_html_raw` (the pure computation). The bridge calls it and then passes the result through `apply_filters` so other plugins' hooks still fire.

For functions WITHOUT a filter hook (e.g., `wpautop`, `make_clickable`):

```php
uopz_set_return('wpautop', function($text, $br = true) {
    return patina_wpautop($text, $br);
}, true);
```

---

## Phase 10: Release & Documentation

**Goal:** First public release with documentation and install tooling.

**Duration:** ~1 week

**Depends on:** Phases 6, 7, 9

### Step 10.1: Documentation

- `docs/ARCHITECTURE.md` — Crate structure, data flow, string handling conventions, panic guard pattern
- `docs/DEPLOYMENT.md` — Step-by-step installation for various hosting setups (VPS, Docker, managed)
- `docs/ADDING-A-FUNCTION.md` — The 10-step contributor guide from Phase 9
- `docs/PROFILING.md` — How to run the profiling stack and interpret results
- `docs/BENCHMARKS.md` — How to run benchmarks, interpret results, and compare JIT interaction

### Step 10.2: Install script

**File: `install.sh`**

```bash
#!/bin/bash
set -euo pipefail

PHP_VERSION=$(php -r "echo PHP_MAJOR_VERSION.'.'.PHP_MINOR_VERSION;")
ARCH=$(uname -m)  # x86_64 or aarch64
VERSION="${1:-latest}"

echo "Installing Patina for PHP $PHP_VERSION on $ARCH..."

# Download
ARTIFACT="patina-php${PHP_VERSION}-linux-${ARCH}.so"
URL="https://github.com/<org>/patina/releases/${VERSION}/download/${ARTIFACT}"
curl -fSL "$URL" -o /tmp/patina.so

# Verify checksum
curl -fSL "${URL%.so}.sha256" -o /tmp/patina.sha256
cd /tmp && sha256sum -c patina.sha256

# Install
EXT_DIR=$(php -r "echo ini_get('extension_dir');")
cp /tmp/patina.so "$EXT_DIR/patina.so"

# Enable
PHP_INI_DIR=$(php -r "echo PHP_CONFIG_FILE_SCAN_DIR;")
echo "extension=patina.so" > "$PHP_INI_DIR/50-patina.ini"

# Verify
php -r "echo 'Patina ' . patina_version() . ' installed successfully.' . PHP_EOL;"
```

### Step 10.3: First release

**Tag:** `v0.1.0`

**Release contents:**
- 8 `.so` artifacts (PHP 8.1-8.4 × x86_64/aarch64)
- `patina-bridge.php` mu-plugin (if non-pluggable functions are implemented)
- `SHA256SUMS`
- `install.sh`
- Changelog:
  - Functions replaced
  - Benchmark summary
  - Known limitations
  - Supported PHP versions and architectures

---

## Timeline Summary

| Phase | Duration | Deliverable | Parallelizable with |
|---|---|---|---|
| **0: Scaffold** | 2 days | Extension loads in PHP 8.1-8.4 | — |
| **1: Profiling** | 1 week | Docker stack, candidate report | Phase 0 |
| **2: Architecture** | 3 days | Module structure, patterns | Phase 1 |
| **3: Testing** | 1 week | Three-layer test framework | Phase 1, 2 |
| **4: First function** | 1 week | `wp_sanitize_redirect` end-to-end | — |
| **5: More pluggable** | 1-2 weeks | `wp_validate_redirect` + others | — |
| **6: CI/CD** | 1 week | Build/test/release automation | Phase 5 |
| **7: Benchmarks** | 1 week | Full benchmark suite, JIT comparison | Phase 6 |
| **8: Strategy research** | 1-2 weeks | Non-pluggable interception decision | Any phase |
| **9: High-value functions** | 3-4 weeks | esc_html, wp_kses, wpautop, etc. | — |
| **10: Release** | 1 week | v0.1.0 with docs and artifacts | — |
| **Total** | **~13-15 weeks** | |

## Dependency Graph

```
Phase 0 ──┬──→ Phase 2 ──→ Phase 4 ──→ Phase 5 ──→ Phase 7
           │                   ↑                       ↑
           ├──→ Phase 3 ───────┘                       │
           │                                           │
           └──→ Phase 6 ──────────────────────────────→┤
                                                       │
Phase 1 ──────────────────────────────────────────────→┤
                                                       │
Phase 8 ──→ Phase 9 ──────────────────────────────────→┤
                                                       │
                                                  Phase 10

Parallelization:
- Phases 0, 1 can run simultaneously (profiling doesn't need the extension)
- Phase 8 (research) can start anytime — it's investigation, not code
- Phase 6 (CI) can start once Phase 4 produces a working extension
- Phases 1, 2, 3 can largely overlap after Phase 0
```

## Key Dependencies

| Crate / Tool | Version | Purpose |
|---|---|---|
| `ext-php-rs` | 0.13.x | PHP Zend API bindings for Rust |
| `aho-corasick` | 1.x | Multi-pattern matching (kses tag allowlist) |
| `regex` | 1.x | DFA regex (PCRE-compatible patterns) |
| `memchr` | 2.x | SIMD-accelerated byte searching |
| `url` | 2.x | URL parsing (for esc_url) |
| `criterion` | 0.5.x | Rust micro-benchmarks |
| `serde` + `serde_json` | 1.x | Test fixture loading (dev-dependency only) |
| `libfuzzer-sys` | 0.4.x | Fuzz testing |
| SPX | 0.4.17+ | PHP profiler |
| k6 | latest | HTTP load testing |
| Docker + Compose | latest | Build, profiling, and test environments |
