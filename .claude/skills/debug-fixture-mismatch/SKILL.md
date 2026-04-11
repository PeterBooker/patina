---
name: debug-fixture-mismatch
description: Debug when a Rust implementation produces different output than the WordPress fixture. Identifies the exact behavioral difference and fixes it.
disable-model-invocation: true
argument-hint: "[function_name]"
---

# Debug Fixture Mismatch: $ARGUMENTS

A Rust test failed because our output doesn't match the WordPress fixture. Find the exact difference and fix it.

## 1. Identify the failing input

Run the Rust test to see the exact input, expected output, and actual output:
```bash
cargo test -p patina-core -- $ARGUMENTS --nocapture 2>&1 | grep "mismatch\|left\|right"
```

## 2. Test in WordPress

Start the profiling stack and test the specific input directly in WordPress:
```bash
cd profiling && docker compose up -d
docker compose exec php-fpm php -d memory_limit=512M -r "
require '/var/www/html/wp-load.php';
var_dump($ARGUMENTS('THE_FAILING_INPUT'));
"
```

## 3. Trace the WordPress implementation

Find and read the WordPress source. Test intermediate steps individually to find where behavior diverges:
```bash
docker compose exec php-fpm php -d memory_limit=512M -r "
require '/var/www/html/wp-load.php';
// Test intermediate functions called by $ARGUMENTS
// e.g., for esc_html: test _wp_specialchars, wp_kses_normalize_entities separately
"
```

Common divergence causes:
- **Entity normalization**: WP zero-pads decimal entities (`&#38;` → `&#038;`) via `wp_kses_normalize_entities`
- **Entity validation**: WP validates named entities against `$allowedentitynames` (253 entries)
- **Null handling**: `wp_kses_no_null` strips literal nulls and `\0` but NOT `%00`
- **htmlspecialchars behavior**: WP calls it with `double_encode=false` after entity normalization
- **Filter hooks**: WP may transform output via `apply_filters` before returning

## 4. Fix and regenerate

Fix the Rust implementation, then regenerate fixtures if the corpus needs updating:
```bash
docker compose -f profiling/docker-compose.yml exec php-fpm php -d memory_limit=512M \
    /app/php/fixture-generator/generate.php --function=$ARGUMENTS --output=/app/fixtures/
```

Verify: `cargo test -p patina-core -- $ARGUMENTS`
