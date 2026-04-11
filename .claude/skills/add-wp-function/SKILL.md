---
name: add-wp-function
description: Add a WordPress function replacement. Full workflow from WP source to Rust implementation, testing, and registration.
disable-model-invocation: true
argument-hint: "[function_name]"
---

# Add: $ARGUMENTS

## 1. Study WordPress source

Start the profiling stack, find and read the source:
```bash
cd profiling && docker compose up -d
docker compose exec php-fpm php -d memory_limit=512M -r "
require '/var/www/html/wp-load.php';
\$ref = new ReflectionFunction('$ARGUMENTS');
echo \$ref->getFileName() . ':' . \$ref->getStartLine() . '-' . \$ref->getEndLine();
"
```

Document: operations in order, whether it calls `apply_filters` (and filter name), whether pluggable, dependencies, edge cases.

## 2. Generate fixtures

Create `php/fixture-generator/functions/$ARGUMENTS.php` following the pattern of existing definitions.
Generate: `docker compose -f profiling/docker-compose.yml exec php-fpm php -d memory_limit=512M /app/php/fixture-generator/generate.php --function=$ARGUMENTS --output=/app/fixtures/`

## 3. Implement in patina-core

Add to the appropriate module (`escaping/`, `kses/`, `formatting/`, `sanitize/`, `pluggable/`). Follow the patterns in existing implementations. Include a `matches_wordpress_fixtures` test.

## 4. Register in patina-ext

Add `#[php_function]` + `wrap_function!()` in `lib.rs`. Follow existing function patterns:
- Raw `patina_*` version (always)
- `_filtered` version with `apply_filters` callback (if WP function uses filters)
- Add filtered variant to `OVERRIDES` if non-pluggable

## 5. Add tests, fuzz target, benchmarks

- PHP test in `php/tests/` extending `FixtureTestCase`
- Fuzz target in `fuzz/fuzz_targets/` + `[[bin]]` in `fuzz/Cargo.toml`
- Criterion bench + PHP benchmark with reference implementation

## 6. Verify

```bash
make check
cargo +nightly fuzz run fuzz_$ARGUMENTS -- -max_total_time=30
```
